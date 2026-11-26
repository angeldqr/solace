use std::collections::HashMap;

use solace_core::agents::{self, AgentDef};
use sysinfo::{Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

/// A cached process table, refreshed on demand, for finding agents under each pane's shell.
pub struct Processes {
    system: System,
    wsl: crate::wsl::Wsl,
    /// Agents inside WSL by pane id, from the last `refresh_wsl`.
    wsl_agents: HashMap<u32, &'static AgentDef>,
}

impl Processes {
    pub fn new() -> Option<Self> {
        System::new().ok().map(|system| Processes {
            system,
            wsl: Default::default(),
            wsl_agents: HashMap::new(),
        })
    }

    pub fn refresh(&mut self) {
        self.system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_cmd(UpdateKind::OnlyIfNotSet),
        );
    }

    /// Looks inside WSL for panes (`(pane id, shell pid)`) with a WSL session under their shell.
    /// `pipe` is this Solace's SOLACE_PIPE, which tells its panes from any other's.
    pub fn refresh_wsl(&mut self, panes: &[(u32, u32)], pipe: &str) {
        let processes = self.system.processes();
        let mut sessions = Vec::new();
        for (&pid, process) in processes {
            if !process.name().eq_ignore_ascii_case("wsl.exe") {
                continue;
            }
            let Some(distro) = crate::wsl::session(&args(process)) else {
                continue;
            };
            for &(pane, root) in panes {
                let root = Pid::from_u32(root);
                if pid == root || depth_below(processes, pid, root).is_some() {
                    sessions.push((pane, distro.clone()));
                }
            }
        }
        self.wsl_agents = self.wsl.agents(&sessions, pipe);
    }

    /// The known agent closest to `root` (the shell itself excluded), else one found inside WSL
    /// for `pane`. Closest wins so an agent's own helpers (an MCP server, a nested
    /// `codex exec`) never take over the pane's identity.
    pub fn find_agent(&self, pane: u32, root: u32) -> Option<&'static AgentDef> {
        let root = Pid::from_u32(root);
        let processes = self.system.processes();
        processes
            .iter()
            .filter(|&(&pid, _)| pid != root)
            .filter_map(|(&pid, process)| {
                let depth = depth_below(processes, pid, root)?;
                let exe = process.name().to_string_lossy();
                Some((depth, agents::identify_process(&exe, &args(process))?))
            })
            .min_by_key(|(depth, _)| *depth)
            .map(|(_, agent)| agent)
            .or_else(|| self.wsl_agents.get(&pane).copied())
    }
}

fn args(process: &Process) -> Vec<String> {
    process
        .cmd()
        .iter()
        .map(|a| a.to_string_lossy().into_owned())
        .collect()
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
