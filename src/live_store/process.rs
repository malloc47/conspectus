//! Whether a recorded agent process still runs, from `/proc`.
//!
//! A pid is checked together with its start time, so a pid the kernel
//! handed to a new process does not pass for the recorded one. Where
//! `/proc` is absent the answer is [`Liveness::Unknown`], and the time
//! rules of [`LiveObservation::effective`] decide alone.

use std::fs;
use std::path::Path;

use crate::model::live::{LiveObservation, Liveness};

/// The start time of `pid` in clock ticks since boot (field 22 of
/// `/proc/<pid>/stat`), or `None` when there is no such process or no
/// `/proc`. Only ever compared for equality.
pub fn process_start_time(pid: i64) -> Option<u64> {
    start_time_in(Path::new("/proc"), pid)
}

/// Whether the process `observation` records still runs: `Unknown`
/// without a pid or without `/proc`, `Dead` when the pid is gone or now
/// belongs to a process that started at another time, else `Alive`.
pub fn process_liveness(observation: &LiveObservation) -> Liveness {
    liveness_in(Path::new("/proc"), observation)
}

pub(crate) fn liveness_in(proc_root: &Path, observation: &LiveObservation) -> Liveness {
    let Some(pid) = observation.pid.filter(|pid| *pid > 0) else {
        return Liveness::Unknown;
    };
    if !proc_root.is_dir() {
        return Liveness::Unknown;
    }
    if !proc_root.join(pid.to_string()).is_dir() {
        return Liveness::Dead;
    }
    Liveness::of_running_pid(observation.pid_start, start_time_in(proc_root, pid))
}

fn start_time_in(proc_root: &Path, pid: i64) -> Option<u64> {
    if pid <= 0 {
        return None;
    }
    let stat = fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    parse_start_time(&stat)
}

/// Field 22 of a `/proc/<pid>/stat` line. The command name in field 2
/// sits in parentheses and may hold spaces or parentheses itself, so the
/// fields after it are counted from the last `)`: field 3 is the first.
fn parse_start_time(stat: &str) -> Option<u64> {
    let after_command = stat.get(stat.rfind(')')? + 1..)?;
    after_command.split_whitespace().nth(19)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::live::{LiveSource, LiveState};

    fn stat_line(command: &str, start: u64) -> String {
        format!(
            "4242 ({command}) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 {start} 1000 10"
        )
    }

    fn observation(pid: Option<i64>, pid_start: Option<u64>) -> LiveObservation {
        let mut observation = LiveObservation::new(LiveState::Working, LiveSource::Hook, 0);
        observation.pid = pid;
        observation.pid_start = pid_start;
        observation
    }

    #[test]
    fn the_start_time_is_field_twenty_two_counted_past_the_command_name() {
        assert_eq!(parse_start_time(&stat_line("claude", 987654)), Some(987654));
        assert_eq!(
            parse_start_time(&stat_line("a (weird) name", 31337)),
            Some(31337)
        );
        assert_eq!(parse_start_time("4242 (short) S 1"), None);
    }

    #[test]
    fn liveness_compares_the_recorded_start_time_with_the_running_one() {
        let proc_root = tempfile::tempdir().unwrap();
        fs::create_dir(proc_root.path().join("4242")).unwrap();
        fs::write(proc_root.path().join("4242/stat"), stat_line("claude", 500)).unwrap();
        let root = proc_root.path();
        assert_eq!(
            liveness_in(root, &observation(Some(4242), Some(500))),
            Liveness::Alive
        );
        assert_eq!(
            liveness_in(root, &observation(Some(4242), None)),
            Liveness::Alive
        );
        assert_eq!(
            liveness_in(root, &observation(Some(4242), Some(499))),
            Liveness::Dead
        );
        assert_eq!(
            liveness_in(root, &observation(Some(77), Some(500))),
            Liveness::Dead
        );
        assert_eq!(
            liveness_in(root, &observation(None, None)),
            Liveness::Unknown
        );
        assert_eq!(
            liveness_in(&root.join("absent"), &observation(Some(4242), Some(500))),
            Liveness::Unknown
        );
    }

    #[test]
    fn this_process_is_alive_with_its_own_start_time() {
        if !Path::new("/proc/self").is_dir() {
            return;
        }
        let pid = i64::from(std::process::id());
        let start = process_start_time(pid);
        assert!(start.is_some());
        assert_eq!(
            process_liveness(&observation(Some(pid), start)),
            Liveness::Alive
        );
    }
}
