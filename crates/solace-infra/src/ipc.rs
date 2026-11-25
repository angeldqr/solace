use std::io::{self, BufRead, BufReader, Read, Write};
use std::thread;

use interprocess::local_socket::traits::ListenerExt as _;
use interprocess::local_socket::{GenericNamespaced, ListenerOptions, Stream, ToNsName};
use solace_core::orchestration::{Request, Response};

/// Requests are small JSON lines; anything bigger is not from the CLI.
const MAX_REQUEST: u64 = 1024 * 1024;

/// Serves the orchestration API, one thread per connection so a long `agent.wait` never blocks
/// other callers.
pub fn serve(
    name: &str,
    handle: impl Fn(Request) -> Response + Send + Sync + Clone + 'static,
) -> io::Result<()> {
    let options = ListenerOptions::new().name(name.to_ns_name::<GenericNamespaced>()?);
    #[cfg(windows)]
    let options = {
        use interprocess::os::windows::local_socket::ListenerOptionsExt as _;
        use interprocess::os::windows::security_descriptor::SecurityDescriptor;
        // Only this user (the pipe's owner) and SYSTEM may connect: the API types into shells.
        let sddl = widestring::U16CString::from_str("D:P(A;;GA;;;SY)(A;;GA;;;OW)")
            .map_err(io::Error::other)?;
        options.security_descriptor(SecurityDescriptor::deserialize(&sddl)?)
    };
    // shortcut: on Unix this is an abstract socket any local user can reach; switch to a 0600
    // socket file under XDG_RUNTIME_DIR before shipping Linux builds.
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
