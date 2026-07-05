//! Declared-link store: read-modify-write helpers (H-REF-005).
//!
//! Encapsulates the file-I/O side of the declared TOML store:
//! finding the target file, loading it for edit, upserting or
//! removing a link, and writing the result back atomically.
//! Consumers reach this through re-exports on the parent
//! [`crate::declared`] module.
//!
//! Kept separate from the TOML model / parse / validate side
//! (`super`) and the snapshot-driven store selection
//! (`super::snapshot`) so a new feature that only touches the
//! write path doesn't have to page in the graph model.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{
    DECLARED_SCHEMA_VERSION, DeclaredDocument, DeclaredLink, DeclaredParseError, DeclaredSection,
    DeclaredWriteError, parse_declared_document, to_toml,
};

/// Which config file a write should target (project vs user
/// scope) alongside its resolved on-disk path. Produced by
/// [`super::snapshot::select_store_for_declaration`]; consumed
/// by the write helpers below.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredStoreSelection {
    pub kind: DeclaredStoreKind,
    pub path: PathBuf,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum DeclaredStoreKind {
    Project,
    User,
}

/// Result of a store write. `changed = false` means the write
/// was a no-op (link already at the target shape); `link_count`
/// is the post-write count so operator confirmations can
/// display "N links stored" without a re-read.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeclaredWriteOutcome {
    pub path: PathBuf,
    pub changed: bool,
    pub link_count: usize,
}

/// Load every declared link with `id` from `paths` and return
/// the first match together with the file it came from. Used
/// by `override` so it can read the existing declaration, flip
/// its state, and write it back to the same store.
pub fn load_declared_link_by_id(
    paths: &[PathBuf],
    id: &str,
) -> Result<Option<(PathBuf, DeclaredLink)>, DeclaredParseError> {
    for path in paths {
        let text = match fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(_) => continue,
        };
        let document = parse_declared_document(&text)?;
        if let Some(link) = document.links().iter().find(|link| link.id == id) {
            return Ok(Some((path.clone(), link.clone())));
        }
    }
    Ok(None)
}

pub fn upsert_declared_link(
    path: impl AsRef<Path>,
    link: DeclaredLink,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    let path = path.as_ref();
    let (mut document, declared) = load_document_for_write(path)?;
    let mut links = declared.links().to_vec();
    let mut changed = true;

    if let Some(existing) = links.iter_mut().find(|existing| existing.id == link.id) {
        changed = existing != &link;
        *existing = link;
    } else {
        links.push(link);
    }

    write_declared_links_if_changed(path, &mut document, links, changed)
}

pub fn remove_declared_link(
    path: impl AsRef<Path>,
    id: &str,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    let path = path.as_ref();
    let (mut document, declared) = load_document_for_write(path)?;
    let mut links = declared.links().to_vec();
    let original_len = links.len();
    links.retain(|link| link.id != id);
    let changed = links.len() != original_len;

    write_declared_links_if_changed(path, &mut document, links, changed)
}

fn load_document_for_write(
    path: &Path,
) -> Result<(toml_edit::DocumentMut, DeclaredDocument), DeclaredWriteError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
        Err(err) => {
            return Err(DeclaredWriteError::Read {
                path: path.to_path_buf(),
                source: err,
            });
        }
    };

    let edit_document = if text.trim().is_empty() {
        toml_edit::DocumentMut::new()
    } else {
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| DeclaredWriteError::Parse {
                path: path.to_path_buf(),
                message: format!("malformed TOML: {err}"),
            })?
    };
    let declared = parse_declared_document(&text).map_err(|err| DeclaredWriteError::Parse {
        path: path.to_path_buf(),
        message: err.to_string(),
    })?;

    Ok((edit_document, declared))
}

fn write_declared_links_if_changed(
    path: &Path,
    document: &mut toml_edit::DocumentMut,
    mut links: Vec<DeclaredLink>,
    changed: bool,
) -> Result<DeclaredWriteOutcome, DeclaredWriteError> {
    links.sort_by(|left, right| left.id.cmp(&right.id));
    let link_count = links.len();

    if changed {
        if links.is_empty() {
            document.as_table_mut().remove("declared");
        } else {
            replace_declared_section(document, links)?;
        }
        write_document(path, document)?;
    }

    Ok(DeclaredWriteOutcome {
        path: path.to_path_buf(),
        changed,
        link_count,
    })
}

/// Persist `document` to `path`. If the document is empty (no
/// top-level keys remain after `[declared]` was stripped), remove the
/// file instead so the store leaves no dangling header behind.
fn write_document(
    path: &Path,
    document: &toml_edit::DocumentMut,
) -> Result<(), DeclaredWriteError> {
    if document.as_table().is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(err) => Err(DeclaredWriteError::Write {
                path: path.to_path_buf(),
                source: err,
            }),
        }
    } else {
        write_atomic(path, &document.to_string()).map_err(|err| DeclaredWriteError::Write {
            path: path.to_path_buf(),
            source: err,
        })
    }
}

fn replace_declared_section(
    document: &mut toml_edit::DocumentMut,
    links: Vec<DeclaredLink>,
) -> Result<(), DeclaredWriteError> {
    let declared_document = DeclaredDocument {
        declared: Some(DeclaredSection {
            schema_version: DECLARED_SCHEMA_VERSION,
            links,
        }),
    };
    let text = to_toml(&declared_document).map_err(|err| DeclaredWriteError::Serialize {
        message: err.to_string(),
    })?;
    let mut replacement =
        text.parse::<toml_edit::DocumentMut>()
            .map_err(|err| DeclaredWriteError::Serialize {
                message: format!("serialized declared section did not parse: {err}"),
            })?;
    document["declared"] = replacement
        .as_table_mut()
        .remove("declared")
        .ok_or_else(|| DeclaredWriteError::Serialize {
            message: "serialized declared section was missing".to_string(),
        })?;
    Ok(())
}

pub(crate) fn write_atomic(path: &Path, text: &str) -> io::Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config");

    for attempt in 0..100 {
        let temp_path = parent.join(format!(".{file_name}.tmp-{}-{attempt}", std::process::id()));
        let mut file = match fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(file) => file,
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        };
        if let Err(err) = file
            .write_all(text.as_bytes())
            .and_then(|_| file.sync_all())
        {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        drop(file);
        if let Err(err) = fs::rename(&temp_path, path) {
            let _ = fs::remove_file(&temp_path);
            return Err(err);
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not allocate temporary file next to {}",
            path.display()
        ),
    ))
}
