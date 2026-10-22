//! The argv a harness starts with when nothing more specific says
//! otherwise (ADR 0142).
//!
//! Slot overrides and a mux's recorded argv outrank everything here and
//! are resolved by the slot resolver. Below them, in order: the
//! project's `[launch.<harness>]` profile, the user's, an `atelier exec`
//! wrapper when the working directory is inside an Atelier workspace
//! and `atelier` is on `PATH`, and the harness's own default.

use std::ffi::OsString;
use std::path::Path;

use crate::config::{LaunchConfig, ProfileOrigin};
use crate::discovery::harness::launch_argv_for;

/// Where a default argv came from.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DefaultSource {
    ProjectProfile,
    UserProfile,
    AtelierExec,
    HarnessDefault,
}

impl DefaultSource {
    /// A phrase for plan notes and detail panes.
    pub fn label(self) -> &'static str {
        match self {
            Self::ProjectProfile => "the project launch profile",
            Self::UserProfile => "the user launch profile",
            Self::AtelierExec => "`atelier exec` (the directory is in an Atelier workspace)",
            Self::HarnessDefault => "the harness default",
        }
    }
}

/// The launch profiles and whether `atelier` is available.
#[derive(Clone, Debug, Default)]
pub struct LaunchDefaults {
    profiles: LaunchConfig,
    atelier_on_path: bool,
}

impl LaunchDefaults {
    /// Profiles from config; `atelier` is looked up on this process's
    /// `PATH`.
    pub fn new(profiles: LaunchConfig) -> Self {
        Self {
            profiles,
            atelier_on_path: binary_on_path("atelier"),
        }
    }

    /// Fix whether `atelier` counts as installed (tests).
    #[must_use]
    pub fn with_atelier(mut self, on_path: bool) -> Self {
        self.atelier_on_path = on_path;
        self
    }

    /// The default argv for `harness` started in `cwd`, and its source.
    /// `None` for a harness Conspectus doesn't know and has no profile
    /// for.
    pub fn default_argv(
        &self,
        harness: &str,
        cwd: Option<&Path>,
    ) -> Option<(Vec<OsString>, DefaultSource)> {
        if let Some(profile) = self.profiles.profiles.get(harness) {
            let source = match profile.origin {
                ProfileOrigin::Project => DefaultSource::ProjectProfile,
                ProfileOrigin::User => DefaultSource::UserProfile,
            };
            return Some((profile.argv.iter().map(OsString::from).collect(), source));
        }
        let base = launch_argv_for(harness);
        if base.is_empty() {
            return None;
        }
        if self.atelier_on_path && cwd.is_some_and(crate::discovery::atelier::in_atelier_workspace)
        {
            let mut argv = vec![OsString::from("atelier"), OsString::from("exec")];
            argv.extend(base);
            return Some((argv, DefaultSource::AtelierExec));
        }
        Some((base, DefaultSource::HarnessDefault))
    }
}

fn binary_on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(name).is_file()))
}

#[cfg(test)]
#[path = "launch_default_tests.rs"]
mod tests;
