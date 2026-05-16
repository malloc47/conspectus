//! Conspectus configuration loading.
//!
//! See ADR 0012 for the layout and precedence rules. The CLI typically
//! calls [`load_from_cwd`], which walks the current directory upward
//! looking for `.conspectus.toml` and merges that on top of the
//! user-level config. Tests usually construct a [`ConfigLoader`]
//! directly so they can inject paths and a fake `$HOME` boundary.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use serde::Deserialize;

/// Filename Conspectus looks for in project trees.
pub const PROJECT_CONFIG_FILENAME: &str = ".conspectus.toml";

/// Path under `$XDG_CONFIG_HOME` (or the platform equivalent) where
/// the user-level config lives.
pub const USER_CONFIG_RELATIVE: &str = "conspectus/config.toml";

/// Resolved configuration after project + user + defaults are merged.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Config {
    pub session: SessionConfig,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SessionConfig {
    pub projection: Projection,
}

/// Session-table projection. Matches the CLI `--projection` flag.
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub enum Projection {
    #[default]
    Agent,
    Mux,
    Union,
}

impl Projection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Agent => "agent",
            Self::Mux => "mux",
            Self::Union => "union",
        }
    }

    pub fn parse(raw: &str) -> Result<Self> {
        match raw {
            "agent" => Ok(Self::Agent),
            "mux" => Ok(Self::Mux),
            "union" => Ok(Self::Union),
            other => Err(anyhow!(
                "invalid session.projection value `{other}`; expected one of agent, mux, union"
            )),
        }
    }
}

/// Disk-shape of `.conspectus.toml` / user config. Kept private so
/// the merged [`Config`] is the only thing the rest of the crate sees.
#[derive(Clone, Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    session: Option<SessionFile>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct SessionFile {
    #[serde(default)]
    projection: Option<String>,
}

/// Result of a single load attempt.
#[derive(Clone, Debug, Default)]
pub struct LoadOutcome {
    pub config: Config,
    /// Files that produced parser or schema errors. The values are
    /// printed to stderr by the CLI but do not abort the run, per
    /// ADR 0012.
    pub diagnostics: Vec<ConfigDiagnostic>,
    pub project_path: Option<PathBuf>,
    pub user_path: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigDiagnostic {
    pub path: PathBuf,
    pub message: String,
}

/// Pluggable loader that takes explicit `cwd`, `home`, and
/// `xdg_config_home` so tests don't have to mutate process state.
#[derive(Clone, Debug, Default)]
pub struct ConfigLoader {
    home: Option<PathBuf>,
    xdg_config_home: Option<PathBuf>,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a loader populated from the process environment.
    pub fn from_env() -> Self {
        Self {
            home: env_path("HOME"),
            xdg_config_home: env_path("XDG_CONFIG_HOME"),
        }
    }

    pub fn with_home(mut self, home: impl Into<PathBuf>) -> Self {
        self.home = Some(home.into());
        self
    }

    pub fn with_xdg_config_home(mut self, xdg: impl Into<PathBuf>) -> Self {
        self.xdg_config_home = Some(xdg.into());
        self
    }

    /// Walk upward from `start` looking for [`PROJECT_CONFIG_FILENAME`],
    /// stopping at `$HOME` (when known) or the filesystem root.
    pub fn locate_project_config(&self, start: impl AsRef<Path>) -> Option<PathBuf> {
        let start = start.as_ref();
        let home = self.home.as_deref();
        let mut current = Some(start);
        while let Some(dir) = current {
            let candidate = dir.join(PROJECT_CONFIG_FILENAME);
            if candidate.is_file() {
                return Some(candidate);
            }
            if home.is_some_and(|home| dir == home) {
                break;
            }
            current = dir.parent();
        }
        None
    }

    /// Compute the user-level config path. Returns `None` only when no
    /// suitable base directory is known (no `$HOME`, no
    /// `$XDG_CONFIG_HOME`).
    pub fn user_config_path(&self) -> Option<PathBuf> {
        if let Some(xdg) = &self.xdg_config_home {
            return Some(xdg.join(USER_CONFIG_RELATIVE));
        }
        self.home
            .as_ref()
            .map(|home| home.join(".config").join(USER_CONFIG_RELATIVE))
    }

    /// Load and merge config from `cwd`. See ADR 0012 for precedence.
    pub fn load_from(&self, cwd: impl AsRef<Path>) -> LoadOutcome {
        let mut outcome = LoadOutcome::default();
        let mut config = Config::default();

        if let Some(path) = self.user_config_path()
            && path.is_file()
        {
            outcome.user_path = Some(path.clone());
            merge_from_file(&mut config, &path, &mut outcome.diagnostics);
        }

        if let Some(path) = self.locate_project_config(cwd) {
            outcome.project_path = Some(path.clone());
            merge_from_file(&mut config, &path, &mut outcome.diagnostics);
        }

        outcome.config = config;
        outcome
    }
}

/// Convenience wrapper: build a loader from the environment and load
/// from the current working directory.
pub fn load_from_cwd() -> Result<LoadOutcome> {
    let cwd = std::env::current_dir().context("failed to read current directory")?;
    Ok(ConfigLoader::from_env().load_from(cwd))
}

fn merge_from_file(config: &mut Config, path: &Path, diagnostics: &mut Vec<ConfigDiagnostic>) {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("failed to read config: {err}"),
            });
            return;
        }
    };

    let parsed: ConfigFile = match toml::from_str(&text) {
        Ok(parsed) => parsed,
        Err(err) => {
            diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            });
            return;
        }
    };

    if let Some(session) = parsed.session
        && let Some(raw) = session.projection
    {
        match Projection::parse(&raw) {
            Ok(projection) => config.session.projection = projection,
            Err(err) => diagnostics.push(ConfigDiagnostic {
                path: path.to_path_buf(),
                message: err.to_string(),
            }),
        }
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    std::env::var_os(key)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(path, contents).expect("write config file");
    }

    #[test]
    fn defaults_apply_when_no_config_files_exist() {
        let temp = TempDir::new().expect("temp dir");
        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(temp.path().join("xdg"));

        let outcome = loader.load_from(temp.path());

        assert_eq!(outcome.config.session.projection, Projection::Agent);
        assert!(outcome.diagnostics.is_empty());
        assert!(outcome.project_path.is_none());
        assert!(outcome.user_path.is_none());
    }

    #[test]
    fn project_config_overrides_default() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config.session.projection, Projection::Mux);
        assert_eq!(
            outcome.project_path.as_deref(),
            Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
        );
    }

    #[test]
    fn user_config_overrides_default_when_no_project_config() {
        let temp = TempDir::new().expect("temp dir");
        let xdg = temp.path().join("xdg");
        write_file(
            &xdg.join(USER_CONFIG_RELATIVE),
            "[session]\nprojection = \"union\"\n",
        );

        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);
        let outcome = loader.load_from(temp.path());

        assert_eq!(outcome.config.session.projection, Projection::Union);
        assert!(outcome.project_path.is_none());
        assert!(outcome.user_path.is_some());
    }

    #[test]
    fn project_config_wins_over_user_config() {
        let temp = TempDir::new().expect("temp dir");
        let xdg = temp.path().join("xdg");
        write_file(
            &xdg.join(USER_CONFIG_RELATIVE),
            "[session]\nprojection = \"mux\"\n",
        );
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project dir");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"agent\"\n",
        );

        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config.session.projection, Projection::Agent);
    }

    #[test]
    fn project_config_search_walks_up_to_home_boundary() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        let nested = project.join("nested").join("deep");
        fs::create_dir_all(&nested).expect("create nested");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&nested);

        assert_eq!(outcome.config.session.projection, Projection::Mux);
        assert_eq!(
            outcome.project_path.as_deref(),
            Some(project.join(PROJECT_CONFIG_FILENAME).as_path())
        );
    }

    #[test]
    fn project_config_search_stops_at_home() {
        let temp = TempDir::new().expect("temp dir");
        let home = temp.path().join("home");
        fs::create_dir_all(&home).expect("create home");
        // Place a file outside $HOME that would match if the walk
        // didn't stop. The loader must not pick it up.
        write_file(
            &temp.path().join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new().with_home(&home);
        let outcome = loader.load_from(&home);

        assert!(outcome.project_path.is_none());
        assert_eq!(outcome.config.session.projection, Projection::Agent);
    }

    #[test]
    fn invalid_projection_value_yields_diagnostic_and_default() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"ledger\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config.session.projection, Projection::Agent);
        assert_eq!(outcome.diagnostics.len(), 1);
        assert!(outcome.diagnostics[0].message.contains("invalid"));
    }

    #[test]
    fn malformed_toml_yields_diagnostic_and_default() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "this isn't toml = =",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config.session.projection, Projection::Agent);
        assert!(
            outcome
                .diagnostics
                .iter()
                .any(|d| d.message.starts_with("malformed TOML"))
        );
    }

    #[test]
    fn unknown_keys_are_silently_ignored() {
        let temp = TempDir::new().expect("temp dir");
        let project = temp.path().join("project");
        fs::create_dir(&project).expect("create project");
        write_file(
            &project.join(PROJECT_CONFIG_FILENAME),
            "[session]\nprojection = \"mux\"\nfuture_key = 1\n\n[unknown]\nx = \"y\"\n",
        );

        let loader = ConfigLoader::new().with_home(temp.path());
        let outcome = loader.load_from(&project);

        assert_eq!(outcome.config.session.projection, Projection::Mux);
        assert!(outcome.diagnostics.is_empty());
    }

    #[test]
    fn projection_round_trips_through_parse_and_as_str() {
        for variant in [Projection::Agent, Projection::Mux, Projection::Union] {
            assert_eq!(Projection::parse(variant.as_str()).unwrap(), variant);
        }
        assert!(Projection::parse("garbage").is_err());
    }

    #[test]
    fn xdg_config_home_overrides_home_fallback() {
        let temp = TempDir::new().expect("temp dir");
        let xdg = temp.path().join("xdg");
        write_file(
            &xdg.join(USER_CONFIG_RELATIVE),
            "[session]\nprojection = \"union\"\n",
        );
        // Also place a colliding file under $HOME/.config that
        // should be ignored when XDG_CONFIG_HOME is set.
        write_file(
            &temp.path().join(".config").join(USER_CONFIG_RELATIVE),
            "[session]\nprojection = \"mux\"\n",
        );

        let loader = ConfigLoader::new()
            .with_home(temp.path())
            .with_xdg_config_home(&xdg);
        let outcome = loader.load_from(temp.path());

        assert_eq!(outcome.config.session.projection, Projection::Union);
    }
}
