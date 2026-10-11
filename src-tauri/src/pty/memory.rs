use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Serialize;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// Measures a console's memory: its shell process plus everything running
/// under it (e.g. `claude`, `node`). One shared `System` so repeated polls
/// reuse the same process table instead of rebuilding it each time.
pub struct MemorySampler(Mutex<System>);

impl Default for MemorySampler {
    fn default() -> Self {
        Self(Mutex::new(System::new()))
    }
}

impl MemorySampler {
    /// Resident memory of `root` and all its descendants, in bytes, or
    /// `None` if `root` has already exited. Summed RSS slightly
    /// over-counts pages the processes share, which is fine for a status
    /// readout.
    pub fn tree_bytes(&self, root: u32) -> Option<u64> {
        let mut system = self.0.lock().unwrap();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory(),
        );
        let processes = system.processes();
        let root = Pid::from_u32(root);
        processes.get(&root)?;
        let tree = tree_pids(processes.iter().map(|(pid, process)| (*pid, process.parent())), root);
        Some(tree.iter().filter_map(|pid| processes.get(pid)).map(|p| p.memory()).sum())
    }

    /// CPU and memory for the diagnostics panel: the whole app, the machine,
    /// and each of `consoles` (id, shell pid). One refresh for all of them,
    /// from which every tree is worked out — asking per console would scan
    /// the process table once each. CPU is measured between two refreshes,
    /// so the first reading after startup is 0 and the ones after it need
    /// the polls to be a moment apart (the panel asks once a second).
    pub fn diagnostics(&self, consoles: &[(String, u32)]) -> Diagnostics {
        let mut system = self.0.lock().unwrap();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing().with_memory().with_cpu(),
        );
        system.refresh_cpu_usage();
        system.refresh_memory();
        let table: Vec<ProcessRow> = system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessRow {
                pid: *pid,
                parent: process.parent(),
                cpu: process.cpu_usage(),
                memory: process.memory(),
                started: process.start_time(),
                name: process.name().to_string_lossy().into_owned(),
            })
            .collect();
        Diagnostics {
            cores: system.cpus().len().max(1),
            system: SystemUsage {
                cpu: system.global_cpu_usage(),
                memory_used: system.used_memory(),
                memory_total: system.total_memory(),
            },
            app: usage_of_tree(&table, std::process::id()).unwrap_or_default(),
            consoles: consoles
                .iter()
                .filter_map(|(id, pid)| {
                    let tree = usage_of_tree(&table, *pid)?;
                    Some(ConsoleUsage { id: id.clone(), pid: *pid, usage: tree })
                })
                .collect(),
        }
    }

    /// Ends everything running under `root` (the shell itself is left to
    /// whoever closes the console). Children first, so a parent doesn't
    /// start another one on its way out. Returns how many it ended.
    pub fn kill_descendants(&self, root: u32) -> usize {
        let mut system = self.0.lock().unwrap();
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
        let processes = system.processes();
        let root = Pid::from_u32(root);
        let mut doomed = tree_pids(processes.iter().map(|(pid, process)| (*pid, process.parent())), root);
        doomed.remove(&root);
        let mut ended = 0;
        for pid in deepest_first(&doomed, processes.iter().map(|(pid, process)| (*pid, process.parent()))) {
            if processes.get(&pid).is_some_and(|process| process.kill()) {
                ended += 1;
            }
        }
        ended
    }

    /// Which of `roots` have something running under them — a command in
    /// progress, as opposed to a shell sitting at its prompt. Asked before
    /// quitting, so the app only interrupts when there's work to lose. One
    /// refresh for all of them: the process table is the expensive part.
    pub fn busy_roots(&self, roots: &[u32]) -> Vec<u32> {
        let mut system = self.0.lock().unwrap();
        system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
        busy_from_table(
            system.processes().iter().map(|(pid, process)| (*pid, process.parent())),
            roots,
        )
    }
}

/// What one process tree (a root and all its descendants) is using.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TreeUsage {
    /// Summed over the tree; 100 is one full core, so it can pass 100.
    pub cpu: f32,
    /// Resident bytes, summed (slightly over-counts shared pages).
    pub memory: u64,
    pub processes: u32,
    /// What the console is running right now: the newest process under the
    /// shell, or the shell itself when it sits at its prompt.
    pub running: String,
    /// Seconds since the root started.
    pub uptime_secs: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleUsage {
    pub id: String,
    pub pid: u32,
    #[serde(flatten)]
    pub usage: TreeUsage,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SystemUsage {
    /// 0–100 across all cores.
    pub cpu: f32,
    pub memory_used: u64,
    pub memory_total: u64,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub cores: usize,
    pub system: SystemUsage,
    /// Termo and everything under it, consoles included.
    pub app: TreeUsage,
    pub consoles: Vec<ConsoleUsage>,
}

struct ProcessRow {
    pid: Pid,
    parent: Option<Pid>,
    cpu: f32,
    memory: u64,
    /// Seconds since the epoch.
    started: u64,
    name: String,
}

/// `root`'s tree out of a process table, or `None` once it has exited.
fn usage_of_tree(table: &[ProcessRow], root: u32) -> Option<TreeUsage> {
    let root = Pid::from_u32(root);
    let root_row = table.iter().find(|row| row.pid == root)?;
    let tree = tree_pids(table.iter().map(|row| (row.pid, row.parent)), root);
    let rows: Vec<&ProcessRow> = table.iter().filter(|row| tree.contains(&row.pid)).collect();
    let newest_below = rows.iter().filter(|row| row.pid != root).max_by_key(|row| (row.started, row.pid));
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0);
    Some(TreeUsage {
        cpu: rows.iter().map(|row| row.cpu).sum(),
        memory: rows.iter().map(|row| row.memory).sum(),
        processes: rows.len() as u32,
        running: newest_below.unwrap_or(&root_row).name.clone(),
        uptime_secs: now.saturating_sub(root_row.started),
    })
}

/// `pids` ordered so that no process comes before one of its descendants.
fn deepest_first(pids: &HashSet<Pid>, parents: impl Iterator<Item = (Pid, Option<Pid>)>) -> Vec<Pid> {
    let parent_of: HashMap<Pid, Option<Pid>> = parents.collect();
    let depth = |pid: &Pid| {
        let mut depth = 0;
        let mut at = *pid;
        while let Some(Some(parent)) = parent_of.get(&at) {
            if !pids.contains(parent) {
                break;
            }
            depth += 1;
            at = *parent;
        }
        depth
    };
    let mut ordered: Vec<Pid> = pids.iter().copied().collect();
    ordered.sort_by_key(|pid| std::cmp::Reverse(depth(pid)));
    ordered
}

fn busy_from_table(parents: impl Iterator<Item = (Pid, Option<Pid>)>, roots: &[u32]) -> Vec<u32> {
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, parent) in parents {
        if let Some(parent) = parent {
            children.entry(parent).or_default().push(pid);
        }
    }
    roots
        .iter()
        .copied()
        .filter(|root| children.get(&Pid::from_u32(*root)).is_some_and(|kids| !kids.is_empty()))
        .collect()
}

fn tree_pids(parents: impl Iterator<Item = (Pid, Option<Pid>)>, root: Pid) -> HashSet<Pid> {
    let mut children: HashMap<Pid, Vec<Pid>> = HashMap::new();
    for (pid, parent) in parents {
        if let Some(parent) = parent {
            children.entry(parent).or_default().push(pid);
        }
    }
    let mut tree = HashSet::new();
    let mut stack = vec![root];
    while let Some(pid) = stack.pop() {
        if tree.insert(pid) {
            if let Some(kids) = children.get(&pid) {
                stack.extend(kids.iter().copied());
            }
        }
    }
    tree
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pid(n: u32) -> Pid {
        Pid::from_u32(n)
    }

    #[test]
    fn collects_all_descendants_but_not_unrelated_processes() {
        let table = vec![
            (pid(1), None),
            (pid(10), Some(pid(1))),
            (pid(11), Some(pid(10))),
            (pid(12), Some(pid(11))),
            (pid(20), Some(pid(1))),
        ];
        let tree = tree_pids(table.into_iter(), pid(10));
        assert_eq!(tree, HashSet::from([pid(10), pid(11), pid(12)]));
    }

    /// A shell with a command running under it is busy; one sitting at its
    /// prompt, with no children, is not.
    #[test]
    fn only_roots_with_something_under_them_are_busy() {
        let table = vec![
            (pid(1), None),
            (pid(10), Some(pid(1))),
            (pid(11), Some(pid(10))),
            (pid(20), Some(pid(1))),
        ];
        assert_eq!(busy_from_table(table.clone().into_iter(), &[10, 20]), vec![10]);
        assert!(busy_from_table(table.into_iter(), &[20]).is_empty());
    }

    fn row(pid: u32, parent: Option<u32>, cpu: f32, memory: u64, started: u64, name: &str) -> ProcessRow {
        ProcessRow {
            pid: Pid::from_u32(pid),
            parent: parent.map(Pid::from_u32),
            cpu,
            memory,
            started,
            name: name.to_string(),
        }
    }

    #[test]
    fn a_trees_usage_adds_up_its_processes_and_names_the_newest() {
        let table = vec![
            row(1, None, 50.0, 1000, 0, "launchd"),
            row(10, Some(1), 1.0, 100, 100, "zsh"),
            row(11, Some(10), 80.0, 300, 200, "cargo"),
            row(12, Some(11), 20.0, 50, 300, "rustc"),
            row(20, Some(1), 90.0, 999, 150, "other"),
        ];
        let usage = usage_of_tree(&table, 10).unwrap();
        assert_eq!(usage.processes, 3);
        assert_eq!(usage.memory, 450);
        assert_eq!(usage.cpu, 101.0);
        assert_eq!(usage.running, "rustc");
        assert!(usage_of_tree(&table, 999).is_none());
    }

    #[test]
    fn a_shell_at_its_prompt_is_what_it_runs() {
        let table = vec![row(10, Some(1), 0.0, 100, 100, "zsh")];
        assert_eq!(usage_of_tree(&table, 10).unwrap().running, "zsh");
    }

    #[test]
    fn descendants_are_ended_before_their_parents() {
        let table = vec![
            (pid(10), Some(pid(1))),
            (pid(11), Some(pid(10))),
            (pid(12), Some(pid(11))),
            (pid(13), Some(pid(10))),
        ];
        let doomed = HashSet::from([pid(11), pid(12), pid(13)]);
        let order = deepest_first(&doomed, table.into_iter());
        let at = |n: u32| order.iter().position(|p| *p == pid(n)).unwrap();
        assert!(at(12) < at(11));
    }

    #[test]
    #[cfg(unix)]
    fn counts_a_real_child_process() {
        let sampler = MemorySampler::default();
        let mut child = std::process::Command::new("sleep").arg("5").spawn().unwrap();

        let child_bytes = sampler.tree_bytes(child.id());
        let own_tree_bytes = sampler.tree_bytes(std::process::id());
        let _ = child.kill();
        let _ = child.wait();

        let child_bytes = child_bytes.expect("child is alive");
        assert!(child_bytes > 0);
        // This test process's own tree includes the sleeping child.
        assert!(own_tree_bytes.expect("own process is alive") > child_bytes);
        assert_eq!(sampler.tree_bytes(u32::MAX - 1), None);
    }

    #[test]
    #[cfg(unix)]
    fn diagnostics_sees_a_real_console_and_kill_descendants_ends_its_children() {
        let sampler = MemorySampler::default();
        let mut shell = std::process::Command::new("sh").args(["-c", "sleep 30 & wait"]).spawn().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));

        let report = sampler.diagnostics(&[("a".to_string(), shell.id())]);
        assert!(report.cores >= 1);
        assert!(report.system.memory_total > 0);
        assert!(report.app.processes >= 2);
        let console = &report.consoles[0];
        assert!(console.usage.processes >= 2, "{console:?}");
        assert!(console.usage.memory > 0);
        assert_eq!(console.usage.running, "sleep");

        assert!(sampler.kill_descendants(shell.id()) >= 1);
        let _ = shell.wait();
        let _ = shell.kill();
        assert!(sampler.diagnostics(&[("a".to_string(), shell.id())]).consoles.is_empty());
    }
}
