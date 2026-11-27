//! Discord Rich Presence over Discord's local IPC (named pipe on Windows, unix socket elsewhere).
//! Only the agent's name and a count are ever sent: never folders, projects, branches or files.

use std::io::{self, Read, Write};
use std::sync::OnceLock;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};

/// The Discord application's public ID (Developer Portal). Empty disables the feature.
const DISCORD_APP_ID: &str = "1575879170863399013";

/// Discord rate-limits SET_ACTIVITY to roughly one per 15 seconds.
const MIN_INTERVAL: Duration = Duration::from_secs(15);
const RECONNECT_EVERY: Duration = Duration::from_secs(30);
/// Resent while an activity is set, so a dead pipe is noticed and Discord never expires it.
const REFRESH_EVERY: Duration = Duration::from_secs(120);
#[cfg(unix)]
const IO_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FIELD: usize = 128;
const MAX_FRAME: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceActivity {
    details: String,
    state: Option<String>,
    small_image: Option<String>,
    small_text: Option<String>,
}

enum Msg {
    Set(PresenceActivity),
    Clear,
}

#[tauri::command]
pub fn presence_available() -> bool {
    !DISCORD_APP_ID.is_empty()
}

#[tauri::command]
pub fn presence_update(activity: Option<PresenceActivity>) {
    if !presence_available() {
        return;
    }
    static TX: OnceLock<Sender<Msg>> = OnceLock::new();
    let tx = match (TX.get(), &activity) {
        (Some(tx), _) => tx,
        // Nothing to clear before the first update.
        (None, None) => return,
        (None, Some(_)) => TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel();
            // The first update, which is when Solace's agents start to matter.
            let start = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            std::thread::spawn(move || run(rx, start));
            tx
        }),
    };
    let _ = tx.send(activity.map_or(Msg::Clear, Msg::Set));
}

/// When the next SET_ACTIVITY may go out.
fn next_send_at(last_sent: Option<Instant>, now: Instant) -> Instant {
    last_sent.map_or(now, |last| (last + MIN_INTERVAL).max(now))
}

fn cap(s: &str) -> String {
    s.chars().take(MAX_FIELD).collect()
}

fn activity_json(a: &PresenceActivity, start: u64) -> Value {
    let mut assets = json!({ "large_image": "solace", "large_text": "Solace" });
    if let Some(image) = a.small_image.as_deref().filter(|s| !s.is_empty()) {
        assets["small_image"] = json!(cap(image));
        if let Some(text) = &a.small_text {
            assets["small_text"] = json!(cap(text));
        }
    }
    let mut activity = json!({
        "type": 0,
        "status_display_type": 2,
        "details": cap(&a.details),
        "timestamps": { "start": start },
        "assets": assets,
    });
    if let Some(state) = a.state.as_deref().filter(|s| !s.is_empty()) {
        activity["state"] = json!(cap(state));
    }
    activity
}

fn encode_frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(&op.to_le_bytes());
    frame.extend_from_slice(&(body.len() as u32).to_le_bytes());
    frame.extend_from_slice(&body);
    frame
}

fn read_frame<R: Read + ?Sized>(r: &mut R) -> io::Result<(u32, Vec<u8>)> {
    let mut header = [0u8; 8];
    r.read_exact(&mut header)?;
    let op = u32::from_le_bytes(header[..4].try_into().unwrap());
    let len = u32::from_le_bytes(header[4..].try_into().unwrap()) as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut body = vec![0u8; len];
    r.read_exact(&mut body)?;
    Ok((op, body))
}

trait Duplex: Read + Write + Send {}
impl<T: Read + Write + Send> Duplex for T {}

/// Sends a frame and returns the reply's op, answering pings on the way.
fn exchange(conn: &mut dyn Duplex, op: u32, payload: &Value) -> io::Result<u32> {
    conn.write_all(&encode_frame(op, payload))?;
    loop {
        let (reply, body) = read_frame(conn)?;
        match reply {
            3 => {
                let ping = serde_json::from_slice(&body).unwrap_or(Value::Null);
                conn.write_all(&encode_frame(4, &ping))?;
            }
            2 => return Err(io::Error::new(io::ErrorKind::ConnectionAborted, "closed")),
            _ => {
                let v: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
                if v["evt"] == "ERROR" {
                    #[cfg(debug_assertions)]
                    {
                        static SHOWN: std::sync::Once = std::sync::Once::new();
                        SHOWN.call_once(|| eprintln!("discord presence: {}", v["data"]["message"]));
                    }
                    return Err(io::Error::other("discord rejected the request"));
                }
                return Ok(reply);
            }
        }
    }
}

fn open_pipe(n: u8) -> io::Result<Box<dyn Duplex>> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION: the pipe server can't impersonate us.
        let open = || {
            std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .security_qos_flags(0x0010_0000 | 0x0001_0000)
                .open(format!(r"\\.\pipe\discord-ipc-{n}"))
        };
        let file = match open() {
            // ERROR_PIPE_BUSY: Discord is serving another client; try once more.
            Err(e) if e.raw_os_error() == Some(231) => {
                std::thread::sleep(Duration::from_millis(200));
                open()?
            }
            other => other?,
        };
        // shortcut: std's pipe File has no timeouts, so a hung Discord blocks this thread
        // (never the UI); upgrade to CancelSynchronousIo from a watchdog if that ever bites.
        Ok(Box::new(file))
    }
    #[cfg(unix)]
    {
        let dirs = ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"]
            .iter()
            .filter_map(|k| std::env::var(k).ok())
            .chain(["/tmp".to_string()]);
        let mut last = io::Error::from(io::ErrorKind::NotFound);
        for dir in dirs {
            let base = std::path::Path::new(&dir);
            // Flatpak and snap keep the socket in a subfolder.
            for sub in ["", "app/com.discordapp.Discord", "snap.discord"] {
                match std::os::unix::net::UnixStream::connect(
                    base.join(sub).join(format!("discord-ipc-{n}")),
                ) {
                    Ok(stream) => {
                        stream.set_read_timeout(Some(IO_TIMEOUT))?;
                        stream.set_write_timeout(Some(IO_TIMEOUT))?;
                        return Ok(Box::new(stream));
                    }
                    Err(err) => last = err,
                }
            }
        }
        Err(last)
    }
}

fn connect() -> io::Result<Box<dyn Duplex>> {
    let mut last = io::Error::from(io::ErrorKind::NotFound);
    for n in 0..10 {
        match open_pipe(n) {
            Ok(mut conn) => {
                match exchange(
                    &mut *conn,
                    0,
                    &json!({ "v": 1, "client_id": DISCORD_APP_ID }),
                ) {
                    Ok(_) => return Ok(conn),
                    Err(err) => last = err,
                }
            }
            Err(err) => last = err,
        }
    }
    Err(last)
}

fn set_activity(
    conn: &mut dyn Duplex,
    activity: Option<&PresenceActivity>,
    start: u64,
    nonce: u64,
) -> io::Result<()> {
    let payload = json!({
        "cmd": "SET_ACTIVITY",
        "args": {
            "pid": std::process::id(),
            "activity": activity.map(|a| activity_json(a, start)),
        },
        "nonce": nonce.to_string(),
    });
    exchange(conn, 1, &payload).map(|_| ())
}

fn run(rx: mpsc::Receiver<Msg>, start: u64) {
    let mut desired: Option<PresenceActivity> = None;
    let mut dirty = false;
    let mut conn: Option<Box<dyn Duplex>> = None;
    let mut last_sent: Option<Instant> = None;
    let mut retry_at = Instant::now();
    let mut nonce = 0u64;
    loop {
        let now = Instant::now();
        let mut due = next_send_at(last_sent, now);
        if conn.is_none() {
            due = due.max(retry_at);
        }
        let wait = if dirty {
            due.saturating_duration_since(now)
        } else if desired.is_some() {
            (last_sent.unwrap_or(now) + REFRESH_EVERY).saturating_duration_since(now)
        } else {
            Duration::from_secs(3600)
        };
        match rx.recv_timeout(wait) {
            Ok(Msg::Set(a)) => {
                desired = Some(a);
                dirty = true;
                continue;
            }
            Ok(Msg::Clear) => {
                desired = None;
                dirty = false;
                // Not throttled: leaving must not wait. Dropping the connection closes it.
                if let Some(mut c) = conn.take() {
                    nonce += 1;
                    let _ = set_activity(&mut *c, None, start, nonce);
                }
                continue;
            }
            Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => dirty |= desired.is_some(),
        }
        if !dirty {
            continue;
        }
        // A broken connection is replaced once straight away before waiting to retry.
        for attempt in 0..2 {
            if conn.is_none() {
                match connect() {
                    Ok(c) => conn = Some(c),
                    Err(_) => {
                        retry_at = Instant::now() + RECONNECT_EVERY;
                        break;
                    }
                }
            }
            nonce += 1;
            let c = conn.as_mut().expect("connected above");
            if set_activity(&mut **c, desired.as_ref(), start, nonce).is_ok() {
                dirty = false;
                last_sent = Some(Instant::now());
                break;
            }
            conn = None;
            if attempt == 1 {
                retry_at = Instant::now() + RECONNECT_EVERY;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> PresenceActivity {
        PresenceActivity {
            details: "Coding with Claude Code".into(),
            state: Some("3 agents working".into()),
            small_image: Some("claude".into()),
            small_text: Some("Claude Code".into()),
        }
    }

    #[test]
    fn frame_round_trips() {
        let frame = encode_frame(1, &json!({ "a": 1 }));
        assert_eq!(&frame[..4], &1u32.to_le_bytes());
        let (op, body) = read_frame(&mut &frame[..]).unwrap();
        assert_eq!(op, 1);
        assert_eq!(
            serde_json::from_slice::<Value>(&body).unwrap(),
            json!({ "a": 1 })
        );
    }

    #[test]
    fn rejects_oversized_and_truncated_frames() {
        let mut huge = Vec::new();
        huge.extend_from_slice(&1u32.to_le_bytes());
        huge.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(read_frame(&mut &huge[..]).is_err());
        assert!(read_frame(&mut &encode_frame(1, &json!({}))[..9]).is_err());
    }

    #[test]
    fn activity_has_exactly_the_expected_fields() {
        let v = activity_json(&sample(), 42);
        assert_eq!(
            v,
            json!({
                "type": 0,
                "status_display_type": 2,
                "details": "Coding with Claude Code",
                "state": "3 agents working",
                "timestamps": { "start": 42 },
                "assets": {
                    "large_image": "solace",
                    "large_text": "Solace",
                    "small_image": "claude",
                    "small_text": "Claude Code"
                }
            })
        );
    }

    #[test]
    fn optional_fields_are_omitted_and_long_text_is_capped() {
        let a = PresenceActivity {
            details: "x".repeat(300),
            state: None,
            small_image: None,
            small_text: Some("orphan".into()),
        };
        let v = activity_json(&a, 1);
        assert_eq!(v["details"].as_str().unwrap().chars().count(), MAX_FIELD);
        assert!(v.get("state").is_none());
        assert!(v["assets"].get("small_image").is_none());
        assert!(v["assets"].get("small_text").is_none());
    }

    #[test]
    fn throttle_waits_out_the_interval() {
        let now = Instant::now();
        assert_eq!(next_send_at(None, now), now);
        assert_eq!(next_send_at(Some(now - Duration::from_secs(20)), now), now);
        assert_eq!(
            next_send_at(Some(now - Duration::from_secs(5)), now),
            now + Duration::from_secs(10)
        );
    }

    #[test]
    fn set_activity_sends_a_command_frame_and_answers_pings() {
        // Reply script: a ping, then an ack.
        struct Fake {
            input: Vec<u8>,
            pos: usize,
            sent: Vec<u8>,
        }
        impl Read for Fake {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                let n = buf.len().min(self.input.len() - self.pos);
                buf[..n].copy_from_slice(&self.input[self.pos..self.pos + n]);
                self.pos += n;
                Ok(n)
            }
        }
        impl Write for Fake {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.sent.extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let mut input = encode_frame(3, &json!({ "p": 1 }));
        input.extend(encode_frame(1, &json!({ "evt": null })));
        let mut fake = Fake {
            input,
            pos: 0,
            sent: vec![],
        };
        set_activity(&mut fake, Some(&sample()), 7, 9).unwrap();
        let (op, body) = read_frame(&mut &fake.sent[..]).unwrap();
        assert_eq!(op, 1);
        let v: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(v["cmd"], "SET_ACTIVITY");
        assert_eq!(v["nonce"], "9");
        assert_eq!(v["args"]["activity"]["timestamps"]["start"], 7);
        // The pong follows the command.
        let rest = &fake.sent[8 + body.len()..];
        assert_eq!(&rest[..4], &4u32.to_le_bytes());
    }
}
