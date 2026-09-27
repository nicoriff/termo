//! Running agents by themselves: on their schedule, or shortly after
//! something changes in the folder they watch. Only while Termo is open — a
//! run the app missed while it was closed happens once, when it's back.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Local, NaiveTime, TimeZone};
use notify::{EventKind, RecursiveMode, Watcher};

use super::runner::Runner;
use super::spec::{AgentSpec, Schedule};
use super::store::{self, RunTrigger};

/// How often the schedules are checked.
const TICK: Duration = Duration::from_secs(5);
/// A burst of changes (a download, a build, a git checkout) starts one run,
/// once things have been quiet this long.
const SETTLE: Duration = Duration::from_secs(5);
/// Changes right after a run are most likely the run's own doing.
const QUIET_AFTER_RUN_MS: i64 = 10_000;

fn parse_time(at: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(at.trim(), "%H:%M").ok()
}

/// When a schedule next falls due after `after`, in Unix milliseconds.
pub fn next_run(schedule: &Schedule, after_ms: i64) -> Option<i64> {
    match schedule {
        Schedule::Interval { minutes } => {
            let minutes = i64::from((*minutes).max(1));
            Some(after_ms + minutes * 60_000)
        }
        Schedule::Daily { at, weekdays } => {
            let time = parse_time(at)?;
            let after: DateTime<Local> = Local.timestamp_millis_opt(after_ms).single()?;
            // Today and the following week: the first slot on an allowed day.
            for offset in 0..8 {
                let day = after.date_naive() + chrono::Days::new(offset);
                let weekday = day.weekday().num_days_from_sunday() as u8;
                if !weekdays.is_empty() && !weekdays.contains(&weekday) {
                    continue;
                }
                let Some(slot) = Local.from_local_datetime(&day.and_time(time)).earliest() else {
                    continue;
                };
                if slot.timestamp_millis() > after_ms {
                    return Some(slot.timestamp_millis());
                }
            }
            None
        }
    }
}

/// When the agent runs next on its own: counted from its last real run
/// (tests from the wizard don't count), or from when it was last saved.
pub fn agent_next_run(root: &Path, spec: &AgentSpec) -> Option<i64> {
    if !spec.enabled {
        return None;
    }
    let schedule = spec.triggers.schedule.as_ref()?;
    let last_run = store::list_runs_in(root, &spec.id)
        .into_iter()
        .find(|run| run.trigger != RunTrigger::Test)
        .map(|run| run.started_at);
    let anchor = last_run.unwrap_or(0).max(spec.updated_at * 1000);
    next_run(schedule, anchor)
}

/// Whether a changed path should count: not the repository's own metadata,
/// not an editor's or a download's temporary file.
fn counts(path: &Path) -> bool {
    let ignored_dir = path
        .components()
        .any(|part| matches!(part.as_os_str().to_str(), Some(".git" | "node_modules" | "target" | ".termo-save")));
    let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
    let temporary = name.ends_with(".tmp")
        || name.ends_with(".crdownload")
        || name.ends_with(".part")
        || name.ends_with('~')
        || name.starts_with(".~");
    !ignored_dir && !temporary
}

struct Watched {
    path: PathBuf,
    _watcher: notify::RecommendedWatcher,
}

pub fn start(app: tauri::AppHandle, runner: Arc<Runner>) {
    std::thread::spawn(move || {
        let Ok(root) = store::root(&app) else { return };
        // agent id → when its folder last changed, while a run is pending.
        let changed: Arc<Mutex<HashMap<String, Instant>>> = Arc::default();
        let mut watchers: HashMap<String, Watched> = HashMap::new();
        loop {
            std::thread::sleep(TICK);
            let agents = store::list_in(&root);
            let now = store::now_millis();

            reconcile_watchers(&agents, &mut watchers, &changed);

            for spec in &agents {
                if !spec.enabled || runner.is_running(&spec.id) {
                    continue;
                }
                if agent_next_run(&root, spec).is_some_and(|due| due <= now) {
                    let _ = runner.start(&app, spec, RunTrigger::Schedule);
                    continue;
                }
                let settled = {
                    let mut pending = changed.lock().unwrap();
                    let ready = pending.get(&spec.id).is_some_and(|at| at.elapsed() >= SETTLE);
                    if ready {
                        pending.remove(&spec.id);
                    }
                    ready
                };
                if settled {
                    let ended_recently = store::list_runs_in(&root, &spec.id)
                        .first()
                        .and_then(|run| run.ended_at)
                        .is_some_and(|ended| now - ended < QUIET_AFTER_RUN_MS);
                    if !ended_recently {
                        let _ = runner.start(&app, spec, RunTrigger::Watch);
                    }
                }
            }
        }
    });
}

/// Starts watching the folders agents now watch, and stops the rest.
fn reconcile_watchers(
    agents: &[AgentSpec],
    watchers: &mut HashMap<String, Watched>,
    changed: &Arc<Mutex<HashMap<String, Instant>>>,
) {
    let wanted: HashMap<&str, PathBuf> = agents
        .iter()
        .filter(|spec| spec.enabled)
        .filter_map(|spec| {
            let path = spec.triggers.watch.as_ref()?.path.trim();
            (!path.is_empty()).then(|| (spec.id.as_str(), PathBuf::from(path)))
        })
        .collect();
    watchers.retain(|id, watched| wanted.get(id.as_str()) == Some(&watched.path));
    for (id, path) in wanted {
        if watchers.contains_key(id) || !path.is_dir() {
            continue;
        }
        let (agent_id, pending) = (id.to_string(), changed.clone());
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else { return };
            if !matches!(event.kind, EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_)) {
                return;
            }
            if event.paths.iter().any(|path| counts(path)) {
                pending.lock().unwrap().insert(agent_id.clone(), Instant::now());
            }
        });
        let Ok(mut watcher) = watcher else { continue };
        if watcher.watch(&path, RecursiveMode::Recursive).is_ok() {
            watchers.insert(id.to_string(), Watched { path, _watcher: watcher });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local_ms(y: i32, m: u32, d: u32, h: u32, min: u32) -> i64 {
        Local.with_ymd_and_hms(y, m, d, h, min, 0).earliest().unwrap().timestamp_millis()
    }

    #[test]
    fn intervals_count_from_the_anchor() {
        assert_eq!(next_run(&Schedule::Interval { minutes: 30 }, 1_000), Some(1_000 + 30 * 60_000));
        // Zero would mean "all the time".
        assert_eq!(next_run(&Schedule::Interval { minutes: 0 }, 0), Some(60_000));
    }

    #[test]
    fn daily_runs_later_today_or_tomorrow() {
        let schedule = Schedule::Daily { at: "09:00".into(), weekdays: vec![] };
        // 2026-09-28 is a Monday.
        assert_eq!(next_run(&schedule, local_ms(2026, 9, 28, 8, 0)), Some(local_ms(2026, 9, 28, 9, 0)));
        assert_eq!(next_run(&schedule, local_ms(2026, 9, 28, 9, 0)), Some(local_ms(2026, 9, 29, 9, 0)));
    }

    #[test]
    fn daily_skips_days_not_chosen() {
        // Weekdays only (Mon-Fri), asked on a Friday evening: next is Monday.
        let schedule = Schedule::Daily { at: "09:00".into(), weekdays: vec![1, 2, 3, 4, 5] };
        let friday_evening = local_ms(2026, 10, 2, 18, 0);
        assert_eq!(next_run(&schedule, friday_evening), Some(local_ms(2026, 10, 5, 9, 0)));
    }

    #[test]
    fn a_bad_time_never_runs() {
        assert_eq!(next_run(&Schedule::Daily { at: "9am".into(), weekdays: vec![] }, 0), None);
    }

    #[test]
    fn repository_metadata_and_temporary_files_dont_count() {
        assert!(counts(Path::new("C:\\dl\\report.pdf")));
        assert!(!counts(Path::new("C:\\repo\\.git\\index")));
        assert!(!counts(Path::new("C:\\dl\\movie.mp4.crdownload")));
        assert!(!counts(Path::new("/p/node_modules/x/y.js")));
    }
}
