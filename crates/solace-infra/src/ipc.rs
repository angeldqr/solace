use std::io::{self, BufRead, BufReader, Read, Write};
use std::thread;

use interprocess::local_socket::traits::ListenerExt as _;
use interprocess::local_socket::{ListenerOptions, Stream};
use solace_core::orchestration::{Request, Response};

/// Requests are small JSON lines; anything bigger is not from the CLI.
const MAX_REQUEST: u64 = 1024 * 1024;

/// Where this app's API listens, given a random nonce: a pipe name on Windows, guarded by its ACL;
/// on Unix a socket file in a folder only this user may enter, since some systems ignore the
/// permissions of the socket itself.
pub fn endpoint(nonce: u64) -> String {
    #[cfg(windows)]
    {
        let user = std::env::var("USERNAME").unwrap_or_default();
        solace_core::orchestration::endpoint_name(&user, nonce)
    }
    #[cfg(unix)]
    {
        // XDG_RUNTIME_DIR is already private to the user; macOS's TMPDIR is per user too.
        let base = std::env::var_os("XDG_RUNTIME_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        // SAFETY: getuid has no preconditions and cannot fail.
        let uid = unsafe { libc::getuid() };
        // Random, so another user can't claim the folder first (in /tmp, without XDG_RUNTIME_DIR).
        let socket = base
            .join(format!("solace-{uid}-{nonce:016x}"))
            .join("api.sock");
        socket.to_string_lossy().into_owned()
    }
}

/// Serves the orchestration API, one thread per connection so a long `agent.wait` never blocks
/// other callers.
pub fn serve(
    name: &str,
    handle: impl Fn(Request) -> Response + Send + Sync + Clone + 'static,
) -> io::Result<()> {
    #[cfg(windows)]
    let options = {
        use interprocess::local_socket::{GenericNamespaced, ToNsName};
        use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
        let options = ListenerOptions::new().name(name.to_ns_name::<GenericNamespaced>()?);
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        // Only this user (the pipe's owner) and SYSTEM may connect: the API types into shells.
        let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;SY)(A;;GA;;;OW)")
            .map_err(io::Error::other)?;
        options.security_descriptor(SecurityDescriptor::deserialize(&sddl)?)
    };
    #[cfg(unix)]
    let options = {
        use interprocess::local_socket::{GenericFilePath, ToFsName};
        private_folder(std::path::Path::new(name))?;
        ListenerOptions::new().name(name.to_fs_name::<GenericFilePath>()?)
    };
    let listener = options.create_sync()?;
    thread::spawn(move || {
        for stream in listener.incoming().filter_map(Result::ok) {
            let handle = handle.clone();
            thread::spawn(move || {
                let _ = respond(stream, handle);
            });
        }
    });
    Ok(())
}

/// Creates the socket's folder for this user alone; one that already exists is refused, since
/// someone else may have made it.
#[cfg(unix)]
fn private_folder(socket: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    let dir = socket
        .parent()
        .ok_or_else(|| io::Error::other("the socket path has no folder"))?;
    std::fs::DirBuilder::new().mode(0o700).create(dir)?;
    let meta = std::fs::symlink_metadata(dir)?;
    // SAFETY: getuid has no preconditions and cannot fail.
    let uid = unsafe { libc::getuid() };
    if !meta.is_dir() || meta.uid() != uid || meta.permissions().mode() & 0o077 != 0 {
        return Err(io::Error::other(format!(
            "{} is not a folder private to this user",
            dir.display()
        )));
    }
    Ok(())
}

fn respond(stream: Stream, handle: impl Fn(Request) -> Response) -> io::Result<()> {
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.by_ref().take(MAX_REQUEST).read_line(&mut line)?;
    let response = match serde_json::from_str::<Request>(&line) {
        Ok(request) => handle(request),
        Err(err) => Err(format!("invalid request: {err}")),
    };
    let mut writer = &stream;
    let mut reply = serde_json::to_vec(&response)?;
    reply.push(b'\n');
    writer.write_all(&reply)?;
    writer.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use interprocess::local_socket::prelude::*;

    fn connect(name: &str) -> io::Result<Stream> {
        #[cfg(windows)]
        let name = {
            use interprocess::local_socket::{GenericNamespaced, ToNsName};
            name.to_ns_name::<GenericNamespaced>()?
        };
        #[cfg(unix)]
        let name = {
            use interprocess::local_socket::{GenericFilePath, ToFsName};
            name.to_fs_name::<GenericFilePath>()?
        };
        Stream::connect(name)
    }

    #[test]
    fn answers_a_request_on_its_endpoint() {
        let name = endpoint(u64::from(std::process::id()) << 20 | 0xabc);
        serve(&name, |_| Ok(serde_json::json!("pong"))).unwrap();
        let stream = connect(&name).unwrap();
        (&stream)
            .write_all(b"{\"method\":\"pane.list\"}\n")
            .unwrap();
        let mut reply = String::new();
        BufReader::new(&stream).read_line(&mut reply).unwrap();
        assert_eq!(reply.trim(), r#"{"Ok":"pong"}"#);
    }

    #[cfg(unix)]
    #[test]
    fn makes_a_new_private_socket_folder_and_refuses_an_existing_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("solace-private-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(private_folder(&dir.join("x.sock")).is_ok());
        let mode = std::fs::metadata(&dir).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        assert!(
            private_folder(&dir.join("x.sock")).is_err(),
            "made before us"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
