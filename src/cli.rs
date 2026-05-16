use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::str::FromStr;

use conspectus::config;
use conspectus::declared::DeclaredEndpoint;
use conspectus::model::RelationKind;

#[derive(Debug, Parser)]
#[command(name = "conspectus", version, about = "AI work graph status tool")]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command.unwrap_or(Command::Graph(GraphArgs::default())) {
            Command::Graph(args) => args.run(),
            Command::Session(args) => args.run(),
            Command::Declared(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Emit the current work graph.
    Graph(GraphArgs),
    /// Render the resolved session table.
    Session(SessionArgs),
    /// Inspect or author declared graph links.
    Declared(Box<DeclaredArgs>),
}

#[derive(Debug, Args)]
struct GraphArgs {
    #[arg(long, value_enum, default_value_t = OutputFormat::Json)]
    format: OutputFormat,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl Default for GraphArgs {
    fn default() -> Self {
        Self {
            format: OutputFormat::Json,
            scan_roots: Vec::new(),
        }
    }
}

impl GraphArgs {
    fn run(self) -> Result<()> {
        match self.format {
            OutputFormat::Json => {
                let snapshot = if self.scan_roots.is_empty() {
                    conspectus::discovery::discover_local_at_roots([std::env::current_dir()?])?
                } else {
                    conspectus::discovery::discover_local_at_roots(self.scan_roots)?
                };
                let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
                println!("{}", conspectus::output::render_graph_json(&snapshot)?);
            }
        }

        Ok(())
    }
}

#[derive(Debug, Args, Default)]
struct SessionArgs {
    /// Table projection to render. Defaults to the value loaded from
    /// `.conspectus.toml` / user config, falling back to `agent`.
    #[arg(long, value_enum)]
    projection: Option<ProjectionFlag>,
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl SessionArgs {
    fn run(self) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let loader = config::ConfigLoader::from_env();
        let outcome = loader.load_from(&cwd);
        for diagnostic in &outcome.diagnostics {
            eprintln!(
                "conspectus: warning: {}: {}",
                diagnostic.path.display(),
                diagnostic.message
            );
        }

        let projection = self
            .projection
            .map(ProjectionFlag::into_config)
            .unwrap_or(outcome.config.session.projection);

        let snapshot = if self.scan_roots.is_empty() {
            conspectus::discovery::discover_local_at_roots([cwd])?
        } else {
            conspectus::discovery::discover_local_at_roots(self.scan_roots)?
        };
        let snapshot = conspectus::resolve::resolve_snapshot(snapshot);
        let table = conspectus::output::table::render(&snapshot, projection);
        print!("{table}");
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredArgs {
    #[command(subcommand)]
    command: DeclaredCommand,
}

impl DeclaredArgs {
    fn run(self) -> Result<()> {
        match self.command {
            DeclaredCommand::List(args) => args.run(),
            DeclaredCommand::Create(args) => args.run(),
            DeclaredCommand::Remove(args) => args.run(),
            DeclaredCommand::Confirm(args) => args.run(),
            DeclaredCommand::Ignore(args) => args.run(),
            DeclaredCommand::Override(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum DeclaredCommand {
    /// List declared links from discovered config stores.
    List(DeclaredListArgs),
    /// Create a declared link.
    Create(Box<DeclaredCreateArgs>),
    /// Remove a declared link by id.
    Remove(DeclaredIdArgs),
    /// Confirm a discovered relationship as a declared link.
    Confirm(DeclaredIdArgs),
    /// Mark a declared link ignored.
    Ignore(DeclaredIgnoreArgs),
    /// Replace one declared link with another.
    Override(DeclaredOverrideArgs),
}

#[derive(Debug, Args)]
struct DeclaredListArgs {
    /// Limit the list to a declared-link store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local declared-link stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredListArgs {
    fn run(self) -> Result<()> {
        let _ = (self.store, self.scan_roots);
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredCreateArgs {
    /// Stable id for the declaration.
    #[arg(long)]
    id: String,
    /// Relationship kind, such as linked_to_mux or branch_has_forge_pr.
    #[arg(long, value_parser = parse_relation_kind)]
    relation: RelationKind,
    /// Source endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    source: DeclaredEndpointArg,
    /// Target endpoint as type:key=value,... using declared TOML field names.
    #[arg(long)]
    target: DeclaredEndpointArg,
    /// Human reason stored with the declaration.
    #[arg(long)]
    reason: Option<String>,
    /// Human label stored with the declaration.
    #[arg(long)]
    label: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl DeclaredCreateArgs {
    fn run(self) -> Result<()> {
        let _ = (
            self.id,
            self.relation,
            self.source.0,
            self.target.0,
            self.reason,
            self.label,
            self.store,
        );
        anyhow::bail!("declared create is not implemented yet")
    }
}

#[derive(Debug, Args)]
struct DeclaredIdArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl DeclaredIdArgs {
    fn run(self) -> Result<()> {
        let _ = (self.id, self.store);
        anyhow::bail!("declared mutation commands are not implemented yet")
    }
}

#[derive(Debug, Args)]
struct DeclaredIgnoreArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Reason the declaration should be ignored.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl DeclaredIgnoreArgs {
    fn run(self) -> Result<()> {
        let _ = (self.id, self.reason, self.store);
        anyhow::bail!("declared ignore is not implemented yet")
    }
}

#[derive(Debug, Args)]
struct DeclaredOverrideArgs {
    /// Declared-link id to replace.
    #[arg(long)]
    id: String,
    /// Replacement declared-link id.
    #[arg(long = "overridden-by")]
    overridden_by: String,
    /// Reason the old declaration was overridden.
    #[arg(long)]
    reason: Option<String>,
    /// Override automatic nearest-store selection.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
}

impl DeclaredOverrideArgs {
    fn run(self) -> Result<()> {
        let _ = (self.id, self.overridden_by, self.reason, self.store);
        anyhow::bail!("declared override is not implemented yet")
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum OutputFormat {
    Json,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum ProjectionFlag {
    Agent,
    Mux,
    Union,
}

impl ProjectionFlag {
    fn into_config(self) -> config::Projection {
        match self {
            Self::Agent => config::Projection::Agent,
            Self::Mux => config::Projection::Mux,
            Self::Union => config::Projection::Union,
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq, ValueEnum)]
enum DeclaredStoreFlag {
    All,
    Project,
    User,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeclaredEndpointArg(DeclaredEndpoint);

impl FromStr for DeclaredEndpointArg {
    type Err = String;

    fn from_str(raw: &str) -> std::result::Result<Self, Self::Err> {
        parse_endpoint(raw).map(Self)
    }
}

fn parse_relation_kind(raw: &str) -> std::result::Result<RelationKind, String> {
    match raw {
        "associated_with" => Ok(RelationKind::AssociatedWith),
        "belongs_to_repo" => Ok(RelationKind::BelongsToRepo),
        "checked_out_branch" => Ok(RelationKind::CheckedOutBranch),
        "workspace_contains_repo" => Ok(RelationKind::WorkspaceContainsRepo),
        "branch_has_forge_pr" => Ok(RelationKind::BranchHasForgePr),
        "linked_to_mux" => Ok(RelationKind::LinkedToMux),
        "rooted_in" => Ok(RelationKind::RootedIn),
        "forks_workspace" => Ok(RelationKind::ForksWorkspace),
        "forks_repo" => Ok(RelationKind::ForksRepo),
        "created_worktree" => Ok(RelationKind::CreatedWorktree),
        "referenced_worktree" => Ok(RelationKind::ReferencedWorktree),
        "parent_session" => Ok(RelationKind::ParentSession),
        "child_session" => Ok(RelationKind::ChildSession),
        "created_branch" => Ok(RelationKind::CreatedBranch),
        "associated_branch" => Ok(RelationKind::AssociatedBranch),
        "parent_fork" => Ok(RelationKind::ParentFork),
        "rooted_at_path" => Ok(RelationKind::RootedAtPath),
        _ => Err(format!(
            "invalid relation `{raw}`; expected a declared relation such as linked_to_mux"
        )),
    }
}

fn parse_endpoint(raw: &str) -> std::result::Result<DeclaredEndpoint, String> {
    let (kind, fields) = raw.split_once(':').ok_or_else(endpoint_syntax_error)?;
    let fields = parse_endpoint_fields(fields)?;
    match kind {
        "repo" => Ok(DeclaredEndpoint::Repo {
            common_dir: required_field(&fields, "common_dir")?,
        }),
        "worktree" => Ok(DeclaredEndpoint::Worktree {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            root: required_field(&fields, "root")?,
        }),
        "workspace" => Ok(DeclaredEndpoint::Workspace {
            root: required_field(&fields, "root")?,
        }),
        "agent_session" => Ok(DeclaredEndpoint::AgentSession {
            harness_key: required_field(&fields, "harness_key")?,
            state_scope: required_field(&fields, "state_scope")?,
            session_key: required_field(&fields, "session_key")?,
        }),
        "mux_session" => Ok(DeclaredEndpoint::MuxSession {
            native_id: required_field(&fields, "native_id")?,
        }),
        "branch" => Ok(DeclaredEndpoint::Branch {
            repo_common_dir: required_field(&fields, "repo_common_dir")?,
            refname: required_field(&fields, "refname")?,
        }),
        "fork" => Ok(DeclaredEndpoint::Fork {
            provider_source_key: required_field(&fields, "provider_source_key")?,
        }),
        "forge_pr" => Ok(DeclaredEndpoint::ForgePr {
            provider: required_field(&fields, "provider")?,
            host: required_field(&fields, "host")?,
            owner: required_field(&fields, "owner")?,
            repo: required_field(&fields, "repo")?,
            number: required_field(&fields, "number")?
                .parse()
                .map_err(|_| "endpoint field `number` must be an integer".to_string())?,
        }),
        _ => Err(endpoint_syntax_error()),
    }
}

fn parse_endpoint_fields(raw: &str) -> std::result::Result<BTreeMap<&str, &str>, String> {
    if raw.is_empty() {
        return Err(endpoint_syntax_error());
    }

    let mut fields = BTreeMap::new();
    for part in raw.split(',') {
        let (key, value) = part.split_once('=').ok_or_else(endpoint_syntax_error)?;
        if key.is_empty() || value.is_empty() {
            return Err(endpoint_syntax_error());
        }
        fields.insert(key, value);
    }
    Ok(fields)
}

fn required_field(fields: &BTreeMap<&str, &str>, key: &str) -> std::result::Result<String, String> {
    fields
        .get(key)
        .map(|value| (*value).to_string())
        .ok_or_else(|| format!("missing endpoint field `{key}`"))
}

fn endpoint_syntax_error() -> String {
    "invalid endpoint syntax; expected type:key=value,... using declared TOML field names"
        .to_string()
}
