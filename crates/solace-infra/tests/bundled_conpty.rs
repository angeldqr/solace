//! The ConPTY Solace ships, driven headless as `solace agent start` does: the shell must start
//! promptly with nobody attached to answer ConPTY's startup queries. Its own process, because
//! portable-pty picks the ConPTY once per process.
#![cfg(windows)]

use std::path::Path;
use std::time::{Duration, Instant};

use solace_infra::pty::{Launch, Pty, use_bundled_conpty};

#[test]
fn headless_pane_runs_promptly_on_the_bundled_conpty() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/conpty");
    if !dir.join("conpty.dll").is_file() {
        eprintln!("skipped: run node scripts/fetch-conpty.mjs first");
        return;
    }
    use_bundled_conpty(&dir).unwrap();
    let cwd = std::env::temp_dir();
    let launch = Launch {
        cwd: &cwd,
        env: &[],
        bin_dir: None,
        shell: None,
    };
    let started = Instant::now();
    let pty = Pty::spawn(&launch, 100, 30, None, |_| {}).unwrap();
    pty.write(b"echo solace-ready\r".to_vec());
    while !pty
        .snapshot()
        .rows
        .iter()
        .any(|row| row.trim_end() == "solace-ready")
    {
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "shell never ran the command"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let took = started.elapsed();
    pty.kill();
    assert!(
        took < Duration::from_millis(2500),
        "headless start took {took:?}"
    );
}
