use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

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
}
