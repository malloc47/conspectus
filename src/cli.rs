use anyhow::{Result, anyhow, bail};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use conspectus::config::{self, ConfigLoader, PROJECT_CONFIG_FILENAME};
use conspectus::declared::{
    DeclaredEndpoint, DeclaredLink, DeclaredLinkState, DeclaredStoreKind, DeclaredStoreSelection,
    declared_endpoint_from_node_id, load_declared_link_by_id, parse_declared_document,
    remove_declared_link, select_store_for_declaration, upsert_declared_link,
};
use conspectus::model::{GraphLink, GraphSnapshot, LinkEndpoint, Provenance, RelationKind};

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
            DeclaredCommand::Remove(args) => args.run_remove(),
            DeclaredCommand::Confirm(args) => args.run_confirm(),
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
        let loader = config::ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records = Vec::new();
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_declared_records(
                &mut records,
                DeclaredStoreFlag::User,
                Provenance::GlobalDeclared,
                path,
            );
        }
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_declared_records(
                    &mut records,
                    DeclaredStoreFlag::Project,
                    Provenance::LocalDeclared,
                    path,
                );
            }
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                declared_record_id(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    declared_record_id(right),
                ))
        });
        for record in records {
            match record.link {
                Ok(link) => println!(
                    "{}",
                    render_declared_record(&record.path, record.store, record.provenance, &link)
                ),
                Err(message) => eprintln!(
                    "conspectus: warning: {}: {}",
                    record.path.display(),
                    message
                ),
            }
        }
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
    /// Root used to discover project-local stores for nearest-store selection.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredCreateArgs {
    fn run(self) -> Result<()> {
        let link = DeclaredLink {
            id: self.id,
            relation: self.relation,
            state: DeclaredLinkState::Active,
            source: self.source.0,
            target: self.target.0,
            reason: self.reason,
            overridden_by: None,
            label: self.label,
        };

        let path = resolve_write_store(
            self.store,
            Some(&link.source),
            Some(&link.target),
            &self.scan_roots,
        )?;

        let outcome =
            upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            if outcome.link_count == 1 {
                "wrote"
            } else {
                "updated"
            }
        } else {
            "unchanged"
        };
        println!("{verb} declared link `{}` in {}", link.id, path.display());
        Ok(())
    }
}

#[derive(Debug, Args)]
struct DeclaredIdArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIdArgs {
    fn run_remove(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let mut removed_from = None;
        for path in &stores {
            if !path.is_file() {
                continue;
            }
            let outcome =
                remove_declared_link(path, &self.id).map_err(|err| anyhow!(err.to_string()))?;
            if outcome.changed {
                removed_from = Some(path.clone());
                break;
            }
        }

        match removed_from {
            Some(path) => {
                println!(
                    "removed declared link `{}` from {}",
                    self.id,
                    path.display()
                );
                Ok(())
            }
            None => {
                bail!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                );
            }
        }
    }

    fn run_confirm(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Active,
            None,
            self.store,
            &self.scan_roots,
        )
    }
}

/// Shared implementation for `declared confirm` and `declared ignore`.
///
/// Both commands take a candidate-link id from the current discovered
/// graph and produce a declared link whose source/target/relation
/// mirror the candidate. They only differ in the link state and the
/// optional reason string.
fn run_confirm_or_ignore(
    candidate_id: &str,
    state: DeclaredLinkState,
    reason: Option<String>,
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<()> {
    let snapshot = discover_for_store_selection(scan_roots)?;
    let candidate = find_candidate_by_id(&snapshot, candidate_id)?;
    let target_node = match &candidate.target {
        LinkEndpoint::Node { id } => id.clone(),
        LinkEndpoint::Unresolved { .. } => bail!(
            "candidate `{candidate_id}` targets an unresolved endpoint; declare it directly with \
             `conspectus declared create`"
        ),
    };

    let source_endpoint = declared_endpoint_from_node_id(&candidate.source);
    let target_endpoint = declared_endpoint_from_node_id(&target_node);

    let link = DeclaredLink {
        id: candidate_id.to_string(),
        relation: candidate.relation.clone(),
        state,
        source: source_endpoint.clone(),
        target: target_endpoint.clone(),
        reason,
        overridden_by: None,
        label: None,
    };

    let path = resolve_write_store(
        store,
        Some(&source_endpoint),
        Some(&target_endpoint),
        scan_roots,
    )?;

    let outcome =
        upsert_declared_link(&path, link.clone()).map_err(|err| anyhow!(err.to_string()))?;

    let verb = match (state, outcome.changed) {
        (DeclaredLinkState::Active, true) => "confirmed",
        (DeclaredLinkState::Ignored, true) => "ignored",
        (DeclaredLinkState::Overridden, true) => "overrode",
        (_, false) => "unchanged",
    };
    println!("{verb} declared link `{}` in {}", link.id, path.display());
    Ok(())
}

fn find_candidate_by_id<'a>(snapshot: &'a GraphSnapshot, id: &str) -> Result<&'a GraphLink> {
    snapshot
        .candidate_links
        .iter()
        .find(|link| link.id == id)
        .ok_or_else(|| anyhow!("no candidate link with id `{id}` was discovered"))
}

#[derive(Debug, Args)]
struct DeclaredIgnoreArgs {
    /// Declared-link id.
    #[arg(long)]
    id: String,
    /// Reason the declaration should be ignored.
    #[arg(long)]
    reason: Option<String>,
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredIgnoreArgs {
    fn run(self) -> Result<()> {
        run_confirm_or_ignore(
            &self.id,
            DeclaredLinkState::Ignored,
            self.reason,
            self.store,
            &self.scan_roots,
        )
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
    /// Restrict the operation to one store.
    #[arg(long, value_enum)]
    store: Option<DeclaredStoreFlag>,
    /// Root used to discover project-local stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl DeclaredOverrideArgs {
    fn run(self) -> Result<()> {
        let stores = candidate_store_paths(self.store, &self.scan_roots)?;
        let (path, existing) = load_declared_link_by_id(&stores, &self.id)
            .map_err(|err| anyhow!(err.to_string()))?
            .ok_or_else(|| {
                anyhow!(
                    "no declared link `{}` found in {}",
                    self.id,
                    store_search_label(&stores)
                )
            })?;

        let mut replacement = existing;
        replacement.state = DeclaredLinkState::Overridden;
        replacement.overridden_by = Some(self.overridden_by);
        if let Some(reason) = self.reason {
            replacement.reason = Some(reason);
        }

        let outcome = upsert_declared_link(&path, replacement.clone())
            .map_err(|err| anyhow!(err.to_string()))?;

        let verb = if outcome.changed {
            "overrode"
        } else {
            "unchanged"
        };
        println!(
            "{verb} declared link `{}` in {}",
            replacement.id,
            path.display()
        );
        Ok(())
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

#[derive(Debug)]
struct DeclaredListRecord {
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
    link: std::result::Result<DeclaredLink, String>,
}

fn declared_record_id(record: &DeclaredListRecord) -> &str {
    match &record.link {
        Ok(link) => &link.id,
        Err(_) => "",
    }
}

fn append_declared_records(
    records: &mut Vec<DeclaredListRecord>,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(DeclaredListRecord {
                store,
                provenance,
                path,
                link: Err(format!("failed to read declared config: {err}")),
            });
            return;
        }
    };

    match parse_declared_document(&text) {
        Ok(document) => {
            records.extend(
                document
                    .links()
                    .iter()
                    .cloned()
                    .map(|link| DeclaredListRecord {
                        store,
                        provenance,
                        path: path.clone(),
                        link: Ok(link),
                    }),
            )
        }
        Err(err) => records.push(DeclaredListRecord {
            store,
            provenance,
            path,
            link: Err(format!("failed to parse declared config: {err}")),
        }),
    }
}

fn render_declared_record(
    path: &std::path::Path,
    store: DeclaredStoreFlag,
    provenance: Provenance,
    link: &DeclaredLink,
) -> String {
    [
        store_label(store).to_string(),
        provenance_label(provenance).to_string(),
        state_label(link.state).to_string(),
        link.id.clone(),
        relation_label(&link.relation).to_string(),
        endpoint_label(&link.source),
        endpoint_label(&link.target),
        link.reason.clone().unwrap_or_default(),
        link.overridden_by.clone().unwrap_or_default(),
        link.label.clone().unwrap_or_default(),
        path.display().to_string(),
    ]
    .join("\t")
}

fn store_label(store: DeclaredStoreFlag) -> &'static str {
    match store {
        DeclaredStoreFlag::All => "all",
        DeclaredStoreFlag::Project => "project",
        DeclaredStoreFlag::User => "user",
    }
}

fn provenance_label(provenance: Provenance) -> &'static str {
    match provenance {
        Provenance::LocalDeclared => "local_declared",
        Provenance::GlobalDeclared => "global_declared",
        Provenance::StrongDiscovered => "strong_discovered",
        Provenance::Discovered => "discovered",
        Provenance::Convention => "convention",
        Provenance::Cached => "cached",
    }
}

fn state_label(state: DeclaredLinkState) -> &'static str {
    match state {
        DeclaredLinkState::Active => "active",
        DeclaredLinkState::Ignored => "ignored",
        DeclaredLinkState::Overridden => "overridden",
    }
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

fn relation_label(relation: &RelationKind) -> &'static str {
    match relation {
        RelationKind::AssociatedWith => "associated_with",
        RelationKind::BelongsToRepo => "belongs_to_repo",
        RelationKind::CheckedOutBranch => "checked_out_branch",
        RelationKind::WorkspaceContainsRepo => "workspace_contains_repo",
        RelationKind::BranchHasForgePr => "branch_has_forge_pr",
        RelationKind::LinkedToMux => "linked_to_mux",
        RelationKind::RootedIn => "rooted_in",
        RelationKind::ForksWorkspace => "forks_workspace",
        RelationKind::ForksRepo => "forks_repo",
        RelationKind::CreatedWorktree => "created_worktree",
        RelationKind::ReferencedWorktree => "referenced_worktree",
        RelationKind::ParentSession => "parent_session",
        RelationKind::ChildSession => "child_session",
        RelationKind::CreatedBranch => "created_branch",
        RelationKind::AssociatedBranch => "associated_branch",
        RelationKind::ParentFork => "parent_fork",
        RelationKind::RootedAtPath => "rooted_at_path",
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

fn endpoint_label(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::Repo { common_dir } => {
            format!("repo:common_dir={common_dir}")
        }
        DeclaredEndpoint::Worktree {
            repo_common_dir,
            root,
        } => {
            format!("worktree:repo_common_dir={repo_common_dir},root={root}")
        }
        DeclaredEndpoint::Workspace { root } => {
            format!("workspace:root={root}")
        }
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => {
            format!(
                "agent_session:harness_key={harness_key},state_scope={state_scope},session_key={session_key}"
            )
        }
        DeclaredEndpoint::MuxSession { native_id } => {
            format!("mux_session:native_id={native_id}")
        }
        DeclaredEndpoint::Branch {
            repo_common_dir,
            refname,
        } => {
            format!("branch:repo_common_dir={repo_common_dir},refname={refname}")
        }
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => {
            format!("fork:provider_source_key={provider_source_key}")
        }
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => {
            format!(
                "forge_pr:provider={provider},host={host},owner={owner},repo={repo},number={number}"
            )
        }
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

/// Resolve which config file a write should target.
///
/// `Some(Project)` / `Some(User)` short-circuit the nearest-store walk;
/// `Some(All)` is rejected because writes have to pick exactly one store.
/// When `store` is `None`, run discovery from the scan roots and ask
/// [`select_store_for_declaration`] to pick the nearest project store,
/// falling back to user config.
fn resolve_write_store(
    store: Option<DeclaredStoreFlag>,
    source: Option<&DeclaredEndpoint>,
    target: Option<&DeclaredEndpoint>,
    scan_roots: &[PathBuf],
) -> Result<PathBuf> {
    match store {
        Some(DeclaredStoreFlag::All) => {
            bail!("`--store all` is not valid for write commands; pick `project` or `user`")
        }
        Some(DeclaredStoreFlag::User) => {
            let loader = ConfigLoader::from_env();
            loader.user_config_path().ok_or_else(|| {
                anyhow!("no user config path available; set $HOME or $XDG_CONFIG_HOME")
            })
        }
        Some(DeclaredStoreFlag::Project) => project_store_path(scan_roots),
        None => match (source, target) {
            (Some(source), Some(target)) => {
                let snapshot = discover_for_store_selection(scan_roots)?;
                let loader = ConfigLoader::from_env();
                let selection = select_store_for_declaration(source, target, &snapshot, &loader)
                    .ok_or_else(|| {
                        anyhow!(
                            "could not pick a declared-link store; \
                             pass --store user or --store project"
                        )
                    })?;
                Ok(selection.path)
            }
            _ => bail!(
                "automatic store selection requires both source and target endpoints; \
                 use --store user or --store project"
            ),
        },
    }
}

fn project_store_path(scan_roots: &[PathBuf]) -> Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let loader = ConfigLoader::from_env();
    let roots = effective_scan_roots(scan_roots, &cwd);

    for root in &roots {
        if let Some(path) = loader.locate_project_config(root) {
            return Ok(path);
        }
    }
    // No existing project config along any scan root: fall back to the
    // first scan root (or cwd) and create one there.
    let fallback = roots.first().cloned().unwrap_or(cwd);
    Ok(fallback.join(PROJECT_CONFIG_FILENAME))
}

/// Effective list of roots used for nearest-store probing. If the caller
/// did not pass any `--scan-root`, we default to the current working
/// directory.
fn effective_scan_roots(scan_roots: &[PathBuf], cwd: &Path) -> Vec<PathBuf> {
    if scan_roots.is_empty() {
        vec![cwd.to_path_buf()]
    } else {
        scan_roots.to_vec()
    }
}

fn discover_for_store_selection(scan_roots: &[PathBuf]) -> Result<GraphSnapshot> {
    let cwd = std::env::current_dir()?;
    let roots = effective_scan_roots(scan_roots, &cwd);
    conspectus::discovery::discover_local_at_roots(roots)
}

/// Candidate stores the read-modify-write helpers should look in when
/// removing or mutating an existing declaration. Order matters: writes
/// stop at the first store that holds a matching id.
fn candidate_store_paths(
    store: Option<DeclaredStoreFlag>,
    scan_roots: &[PathBuf],
) -> Result<Vec<PathBuf>> {
    let loader = ConfigLoader::from_env();
    let mut paths = Vec::new();

    let include_project = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::Project)
    );
    let include_user = matches!(
        store,
        None | Some(DeclaredStoreFlag::All) | Some(DeclaredStoreFlag::User)
    );

    if include_project {
        let cwd = std::env::current_dir()?;
        let roots = effective_scan_roots(scan_roots, &cwd);
        let mut seen = BTreeSet::new();
        for root in roots {
            if let Some(path) = loader.locate_project_config(root)
                && seen.insert(path.clone())
            {
                paths.push(path);
            }
        }
    }

    if include_user && let Some(path) = loader.user_config_path() {
        paths.push(path);
    }

    Ok(paths)
}

fn store_search_label(paths: &[PathBuf]) -> String {
    if paths.is_empty() {
        "any declared-link store".to_string()
    } else {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// Borrow checker convenience: lets us reuse the existing
/// [`DeclaredStoreSelection`] type for emitted CLI messages.
fn _selection_display(selection: &DeclaredStoreSelection) -> String {
    let kind = match selection.kind {
        DeclaredStoreKind::Project => "project",
        DeclaredStoreKind::User => "user",
    };
    format!("{kind} {}", selection.path.display())
}
