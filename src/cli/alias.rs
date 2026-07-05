//! `conspectus alias` subcommand tree (H-REF-006 wave 9).
//!
//! Currently just `alias list`, which walks the discovered
//! project + user alias stores and prints one line per entry.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Subcommand};

use conspectus::aliases::{AliasEntry, AliasesDocument, parse_aliases_document};
use conspectus::config::ConfigLoader;
use conspectus::declared::DeclaredEndpoint;

use super::{DeclaredStoreFlag, store_label};

#[derive(Debug, Args)]
pub(super) struct AliasArgs {
    #[command(subcommand)]
    command: AliasCommand,
}

impl AliasArgs {
    pub(super) fn run(self) -> Result<()> {
        match self.command {
            AliasCommand::List(args) => args.run(),
        }
    }
}

#[derive(Debug, Subcommand)]
enum AliasCommand {
    /// List session aliases from the discovered config stores.
    List(AliasListArgs),
}

#[derive(Debug, Args)]
struct AliasListArgs {
    /// Limit the list to a store.
    #[arg(long, value_enum, default_value_t = DeclaredStoreFlag::All)]
    store: DeclaredStoreFlag,
    /// Root used to discover project-local alias stores.
    #[arg(long = "scan-root", value_name = "PATH")]
    scan_roots: Vec<PathBuf>,
}

impl AliasListArgs {
    fn run(self) -> Result<()> {
        let loader = ConfigLoader::from_env();
        let cwd = std::env::current_dir()?;
        let scan_roots = if self.scan_roots.is_empty() {
            vec![cwd]
        } else {
            self.scan_roots
        };

        let mut records: Vec<AliasListRecord> = Vec::new();
        if matches!(
            self.store,
            DeclaredStoreFlag::All | DeclaredStoreFlag::Project
        ) {
            let mut project_paths = BTreeSet::new();
            for root in &scan_roots {
                if let Some(path) = loader.locate_project_config(root) {
                    project_paths.insert(path);
                }
            }
            for path in project_paths {
                append_alias_records(&mut records, DeclaredStoreFlag::Project, path);
            }
        }
        if matches!(self.store, DeclaredStoreFlag::All | DeclaredStoreFlag::User)
            && let Some(path) = loader.user_config_path()
        {
            append_alias_records(&mut records, DeclaredStoreFlag::User, path);
        }

        records.sort_by(|left, right| {
            (
                store_label(left.store),
                left.path.as_path(),
                alias_record_key(left),
            )
                .cmp(&(
                    store_label(right.store),
                    right.path.as_path(),
                    alias_record_key(right),
                ))
        });

        for record in records {
            match record.entry {
                Ok(entry) => println!(
                    "{}\t{}\t{}\t{}",
                    store_label(record.store),
                    record.path.display(),
                    format_alias_endpoint(&entry.node),
                    entry.display_name
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

#[derive(Debug)]
struct AliasListRecord {
    store: DeclaredStoreFlag,
    path: PathBuf,
    entry: std::result::Result<AliasEntry, String>,
}

fn alias_record_key(record: &AliasListRecord) -> String {
    match &record.entry {
        Ok(entry) => format_alias_endpoint(&entry.node),
        Err(_) => String::new(),
    }
}

fn append_alias_records(
    records: &mut Vec<AliasListRecord>,
    store: DeclaredStoreFlag,
    path: PathBuf,
) {
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to read aliases: {err}")),
            });
            return;
        }
    };
    let parsed: AliasesDocument = match parse_aliases_document(&text) {
        Ok(document) => document,
        Err(err) => {
            records.push(AliasListRecord {
                store,
                path,
                entry: Err(format!("failed to parse aliases: {err}")),
            });
            return;
        }
    };
    for entry in parsed.entries() {
        records.push(AliasListRecord {
            store,
            path: path.clone(),
            entry: Ok(entry.clone()),
        });
    }
}

/// H-REF-006 wave 9: shared with the declared subcommand
/// (still in cli/mod.rs). Retained as `pub(super)` so both
/// consumers reach it through the same declaration.
pub(super) fn format_alias_endpoint(endpoint: &DeclaredEndpoint) -> String {
    match endpoint {
        DeclaredEndpoint::AgentSession {
            harness_key,
            state_scope,
            session_key,
        } => format!("agent_session:{harness_key}:{state_scope}:{session_key}"),
        DeclaredEndpoint::MuxSession { native_id } => format!("mux_session:{native_id}"),
        DeclaredEndpoint::Pin { id } => format!("pin:{id}"),
        DeclaredEndpoint::RuntimeProcess { observation_key } => {
            format!("runtime_process:{observation_key}")
        }
        DeclaredEndpoint::Repo { common_dir } => format!("repo:{common_dir}"),
        DeclaredEndpoint::Checkout { root, .. } => format!("checkout:{root}"),
        DeclaredEndpoint::Workspace { root } => format!("workspace:{root}"),
        DeclaredEndpoint::Branch { refname, .. } => format!("branch:{refname}"),
        DeclaredEndpoint::Fork {
            provider_source_key,
        } => format!("fork:{provider_source_key}"),
        DeclaredEndpoint::ForgePr {
            provider,
            host,
            owner,
            repo,
            number,
        } => format!("forge_pr:{provider}:{host}/{owner}/{repo}#{number}"),
    }
}
