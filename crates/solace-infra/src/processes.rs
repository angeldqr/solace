use std::collections::HashMap;

use solace_core::agents::{self, AgentDef};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A cached process table, refreshed on demand, for finding agents under each pane's shell.
pub struct Processes {
    system: System,
}

impl Processes {
    pub fn new() -> Option<Self> {
        System::new().ok().map(|system| Processes { system })
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
        );
    }

    /// The known agent closest to `root` (the shell itself excluded). Closest wins so an agent's
    /// own helpers (an MCP server, a nested `codex exec`) never take over the pane's identity.
    pub fn find_agent(&self, root: u32) -> Option<&'static AgentDef> {
        let root = Pid::from_u32(root);
        let processes = self.system.processes();
        processes
            .iter()
            .filter(|&(&pid, _)| pid != root)
            .filter_map(|(&pid, process)| {
                let depth = depth_below(processes, pid, root)?;
                let exe = process.name().to_string_lossy();
                let args: Vec<String> = process
                    .cmd()
                    .iter()
                    .map(|a| a.to_string_lossy().into_owned())
                    .collect();
                Some((depth, agents::identify(&exe, &args)?))
            })
            .min_by_key(|(depth, _)| *depth)
            .map(|(_, agent)| agent)
    }
}

/// Generations between `pid` and its ancestor `root`, if it is one.
fn depth_below(processes: &HashMap<Pid, Process>, mut pid: Pid, root: Pid) -> Option<usize> {
    // Bounded: a recycled pid can make a parent chain loop.
    for depth in 1..=32 {
        let process = processes.get(&pid)?;
        let parent_pid = process.parent()?;
        // Windows keeps a dead parent's pid; a parent born after its child is a stranger.
        if processes
            .get(&parent_pid)
            .is_some_and(|parent| parent.start_time() > process.start_time())
        {
            return None;
        }
        if parent_pid == root {
            return Some(depth);
        }
        pid = parent_pid;
    }
    None
}
