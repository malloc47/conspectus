//! Where a harness keeps its own files, and where discovery reads them
//! (ADR 0239).
//!
//! Claude Code and Codex each let the operator move their directory
//! with an environment variable (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`).
//! Discovery and `hook init|status|remove --scope user` resolve that
//! directory through the same [`HarnessDir`], so the hooks land where
//! the harness reads its settings and discovery reads where the harness
//! writes its sessions. A `CONSPECTUS_<HARNESS>_STATE` override moves
//! discovery alone: the harness never reads it, so a hook installed
//! there would never run.

use std::ffi::OsString;
use std::path::PathBuf;

/// One harness's directory rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarnessDir {
    /// The Conspectus variable that points discovery elsewhere.
    pub state_override: &'static str,
    /// The harness's own variable that moves its directory.
    pub variable: &'static str,
    /// The directory under `$HOME` without that variable.
    pub home_relative: &'static str,
}

impl HarnessDir {
    /// The harness's directory: what [`Self::variable`] names, else
    /// `$HOME/<home_relative>`. An empty value counts as unset, and
    /// `None` means neither is set.
    pub fn dir(&self, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
        match non_empty(&env, self.variable) {
            Some(dir) => Some(PathBuf::from(dir)),
            None => Some(PathBuf::from(non_empty(&env, "HOME")?).join(self.home_relative)),
        }
    }

    /// Discovery's root: [`Self::state_override`] when set, else
    /// [`Self::dir`].
    pub fn state_root(&self, env: impl Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
        match non_empty(&env, self.state_override) {
            Some(root) => Some(PathBuf::from(root)),
            None => self.dir(env),
        }
    }

    /// [`Self::dir`] from the process environment.
    pub fn dir_from_env(&self) -> Option<PathBuf> {
        self.dir(|key| std::env::var_os(key))
    }

    /// [`Self::state_root`] from the process environment.
    pub fn state_root_from_env(&self) -> Option<PathBuf> {
        self.state_root(|key| std::env::var_os(key))
    }

    /// What `--scope user` needs when [`Self::dir`] finds nothing.
    pub fn missing_message(&self) -> String {
        format!("HOME or {} is required for --scope user", self.variable)
    }
}

fn non_empty(env: &impl Fn(&str) -> Option<OsString>, key: &str) -> Option<OsString> {
    env(key).filter(|value| !value.is_empty())
}

/// An environment of `pairs` alone, for the resolvers' tests.
#[cfg(test)]
pub(crate) fn env_of(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
    let map: std::collections::HashMap<String, OsString> = pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), OsString::from(value)))
        .collect();
    move |key| map.get(key).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOOL: HarnessDir = HarnessDir {
        state_override: "CONSPECTUS_TOOL_STATE",
        variable: "TOOL_HOME",
        home_relative: ".tool",
    };

    #[test]
    fn the_harness_variable_moves_the_directory() {
        let env = env_of(&[("TOOL_HOME", "/elsewhere/tool"), ("HOME", "/home/user")]);
        assert_eq!(TOOL.dir(&env), Some(PathBuf::from("/elsewhere/tool")));
        assert_eq!(
            TOOL.state_root(&env),
            Some(PathBuf::from("/elsewhere/tool"))
        );
    }

    #[test]
    fn home_is_the_default() {
        let env = env_of(&[("HOME", "/home/user")]);
        assert_eq!(TOOL.dir(&env), Some(PathBuf::from("/home/user/.tool")));
        assert_eq!(
            TOOL.state_root(&env),
            Some(PathBuf::from("/home/user/.tool"))
        );
        assert_eq!(TOOL.dir(env_of(&[])), None);
        assert_eq!(TOOL.state_root(env_of(&[])), None);
    }

    #[test]
    fn the_override_moves_discovery_alone() {
        let env = env_of(&[
            ("CONSPECTUS_TOOL_STATE", "/fixture"),
            ("TOOL_HOME", "/elsewhere/tool"),
            ("HOME", "/home/user"),
        ]);
        assert_eq!(TOOL.state_root(&env), Some(PathBuf::from("/fixture")));
        assert_eq!(TOOL.dir(&env), Some(PathBuf::from("/elsewhere/tool")));
        let without_variable = env_of(&[("CONSPECTUS_TOOL_STATE", "/fixture"), ("HOME", "/h")]);
        assert_eq!(TOOL.dir(&without_variable), Some(PathBuf::from("/h/.tool")));
    }

    #[test]
    fn empty_values_count_as_unset() {
        let env = env_of(&[
            ("CONSPECTUS_TOOL_STATE", ""),
            ("TOOL_HOME", ""),
            ("HOME", "/home/user"),
        ]);
        assert_eq!(TOOL.dir(&env), Some(PathBuf::from("/home/user/.tool")));
        assert_eq!(
            TOOL.state_root(&env),
            Some(PathBuf::from("/home/user/.tool"))
        );
        assert_eq!(TOOL.dir(env_of(&[("HOME", "")])), None);
    }

    #[test]
    fn the_missing_message_names_the_variable() {
        assert_eq!(
            TOOL.missing_message(),
            "HOME or TOOL_HOME is required for --scope user"
        );
    }
}
