//! End to end: a fake Claude Code (a node script under the real package path) runs in a pty,
//! and the process scan plus screen rules recognize it and read its state from the title.

use std::fs;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use solace_core::agents::AgentState;
use solace_infra::processes::Processes;
use solace_infra::pty::Pty;

#[test]
fn fake_claude_under_the_shell_is_identified_and_working() {
    if which::which("node").is_err() {
        eprintln!("skipped: node is not installed");
        return;
    }
    let dir = std::env::temp_dir().join(format!("solace-agent-test-{}", std::process::id()));
    let package = dir.join("node_modules/@anthropic-ai/claude-code");
    fs::create_dir_all(&package).unwrap();
    // Sets a Claude-style spinner title, then stays alive like a running agent.
    fs::write(
        package.join("cli.js"),
        "process.stdout.write('\\x1b]0;⠂ Refactoring\\x07ready\\n'); setTimeout(() => {}, 60000);",
    )
    .unwrap();

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let pty = Pty::spawn(
        &dir,
        &[],
        None,
        100,
        30,
        Some(Box::new(move |bytes| {
            let _ = tx.send(bytes);
        })),
        |_| {},
    )
    .unwrap();
    pty.write(b"\x1b[1;1R".to_vec());
    let script = package.join("cli.js");
    pty.write(format!("node \"{}\"\r", script.display()).into_bytes());

    let mut processes = Processes::new().unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let def = loop {
        while rx.try_recv().is_ok() {}
        processes.refresh();
        if let Some(def) = processes.find_agent(pty.pid().unwrap())
            && pty.snapshot().title.contains("Refactoring")
        {
            break def;
        }
        assert!(
            Instant::now() < deadline,
            "agent not found; title {:?}",
            pty.snapshot().title
        );
        std::thread::sleep(Duration::from_millis(200));
    };
    assert_eq!(def.id, "claude");
    let screen = pty.snapshot();
    assert_eq!(
        def.detect(&screen.title, &screen.rows),
        Some(AgentState::Working)
    );

    pty.kill();
    let _ = fs::remove_dir_all(&dir);
}
