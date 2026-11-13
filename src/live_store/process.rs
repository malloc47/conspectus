//! Whether a recorded agent process still runs, from `/proc`.
//!
//! A pid is checked together with its start time, so a pid the kernel
//! handed to a new process does not pass for the recorded one. Where
//! `/proc` is absent the answer is [`Liveness::Unknown`], and the time
//! rules of [`LiveObservation::effective`] decide alone.
//!
//! The check goes through [`PidProbe`], so the store's garbage
//! collection, the graph binding, and tests that stand in a process
//! table of their own share one rule ([`PidProbe::liveness`]).

use std::fs;
use std::path::{Path, PathBuf};

use crate::model::live::{LiveObservation, Liveness};

/// What a probe found about one pid.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PidStatus {
    /// A process has the pid. `start` is its start time when the probe
    /// could read it.
    Running { start: Option<u64> },
    /// No process has the pid.
    Gone,
    /// The probe cannot tell, as on a platform without `/proc`.
    Unknown,
}

/// A process table to check recorded pids against.
pub trait PidProbe {
    /// What the table says about `pid` now.
    fn status(&self, pid: i64) -> PidStatus;

    /// Whether the process `observation` records still runs: `Unknown`
    /// without a pid or when the probe cannot tell, `Dead` when the pid
    /// is gone or now belongs to a process that started at another time
    /// ([`Liveness::of_running_pid`]), else `Alive`.
    fn liveness(&self, observation: &LiveObservation) -> Liveness {
        let Some(pid) = observation.pid.filter(|pid| *pid > 0) else {
            return Liveness::Unknown;
        };
        match self.status(pid) {
            PidStatus::Running { start } => Liveness::of_running_pid(observation.pid_start, start),
            PidStatus::Gone => Liveness::Dead,
            PidStatus::Unknown => Liveness::Unknown,
        }
    }
}

/// The process table under a `/proc` directory: one directory per pid,
/// with the start time in field 22 of its `stat`. It reads files only
/// and starts no process.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcProbe {
    root: Option<PathBuf>,
}

impl ProcProbe {
    /// This machine's process table: `/proc` on Linux. Elsewhere the
    /// probe answers [`PidStatus::Unknown`] for every pid.
    pub fn system() -> Self {
        Self {
            root: cfg!(target_os = "linux").then(|| PathBuf::from("/proc")),
        }
    }

    /// The table under `root`, laid out as `/proc` is.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
        }
    }
}

impl PidProbe for ProcProbe {
    /// A running process, the common case, costs one read of its `stat`;
    /// only a pid with no readable `stat` costs a stat of its directory
    /// and of the table's root, to tell a gone process from a table that
    /// is not there.
    fn status(&self, pid: i64) -> PidStatus {
        let Some(root) = self.root.as_deref() else {
            return PidStatus::Unknown;
        };
        if pid > 0 {
            let dir = root.join(pid.to_string());
            if let Ok(stat) = fs::read_to_string(dir.join("stat")) {
                return PidStatus::Running {
                    start: parse_start_time(&stat),
                };
            }
            if dir.is_dir() {
                return PidStatus::Running { start: None };
            }
        }
        if root.is_dir() {
            PidStatus::Gone
        } else {
            PidStatus::Unknown
        }
    }
}

/// The start time of `pid` in clock ticks since boot (field 22 of
/// `/proc/<pid>/stat`), or `None` when there is no such process or no
/// `/proc`. Only ever compared for equality.
pub fn process_start_time(pid: i64) -> Option<u64> {
    start_time_in(Path::new("/proc"), pid)
}

/// Whether the process `observation` records still runs, by this
/// machine's process table ([`ProcProbe::system`]).
pub fn process_liveness(observation: &LiveObservation) -> Liveness {
    ProcProbe::system().liveness(observation)
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
            ProcProbe::at(root).liveness(&observation(Some(4242), Some(500))),
            Liveness::Alive
        );
        assert_eq!(
            ProcProbe::at(root).liveness(&observation(Some(4242), None)),
            Liveness::Alive
        );
        assert_eq!(
            ProcProbe::at(root).liveness(&observation(Some(4242), Some(499))),
            Liveness::Dead
        );
        assert_eq!(
            ProcProbe::at(root).liveness(&observation(Some(77), Some(500))),
            Liveness::Dead
        );
        assert_eq!(
            ProcProbe::at(root).liveness(&observation(None, None)),
            Liveness::Unknown
        );
        assert_eq!(
            ProcProbe::at(root.join("absent")).liveness(&observation(Some(4242), Some(500))),
            Liveness::Unknown
        );
        fs::create_dir(root.join("5151")).unwrap();
        assert_eq!(
            ProcProbe::at(root).status(5151),
            PidStatus::Running { start: None },
            "a process whose stat cannot be read runs, start unknown"
        );
        assert_eq!(ProcProbe::at(root).status(0), PidStatus::Gone);
    }

    #[test]
    fn a_probe_that_cannot_tell_leaves_liveness_unknown() {
        struct Blind;
        impl PidProbe for Blind {
            fn status(&self, _: i64) -> PidStatus {
                PidStatus::Unknown
            }
        }
        assert_eq!(
            Blind.liveness(&observation(Some(4242), Some(500))),
            Liveness::Unknown
        );
        assert_eq!(ProcProbe::at("/no/such/proc").status(1), PidStatus::Unknown);
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
