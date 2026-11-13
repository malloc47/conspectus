//! The pin slot store (ADR 0141): `$XDG_DATA_HOME/conspectus/pins.toml`.
//!
//! ```toml
//! [pins]
//! schema_version = 2
//!
//! [[pins.slots]]
//! id = "conspectus-main"
//! kind = "mux"
//! target = "history:h_7f3a9c0e21d4"
//! pinned_at = 2026-10-19T12:00:00Z
//!
//! [pins.slots.overrides]
//! argv = ["claude", "--dangerously-skip-permissions"]
//! ```
//!
//! Slot order is the order of the tables. Writes edit the document in
//! place with `toml_edit`, so comments, formatting, and unknown keys on
//! every slot a gesture doesn't touch survive, and each write goes
//! through a sibling temporary file renamed into place under an
//! advisory lock. A file with another `schema_version` is read for
//! nothing and never rewritten, so an older binary can't misread a
//! newer table. Only operator gestures write this file; the daemon
//! never does (ADR 0143).

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use toml_edit::{Array, ArrayOfTables, DocumentMut, Item, Table, Value};

use crate::declared::{create_private_dir_all, open_private_lock, write_private_atomic};
use crate::model::{FieldValue, PinKind, PinSlot, SlotFields};

/// Schema version of the `[pins]` table this binary reads and writes.
pub const PINS_SCHEMA_VERSION: u32 = 2;

/// File name inside `$XDG_DATA_HOME/conspectus/`.
pub const PINS_FILE_NAME: &str = "pins.toml";

#[derive(Debug, Deserialize)]
struct RawFile {
    pins: Option<RawPins>,
}

#[derive(Debug, Deserialize)]
struct RawPins {
    schema_version: Option<u32>,
    #[serde(default)]
    slots: Vec<toml::Value>,
}

#[derive(Debug, Deserialize)]
struct RawSlot {
    id: String,
    kind: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    pinned_at: Option<toml::Value>,
    #[serde(default)]
    overrides: std::collections::BTreeMap<String, FieldValue>,
    #[serde(default)]
    snapshot: std::collections::BTreeMap<String, FieldValue>,
}

/// What a read found.
#[derive(Clone, Debug, Default)]
pub struct SlotLoad {
    pub slots: Vec<PinSlot>,
    /// One line per slot or file problem. A bad slot is skipped here
    /// but kept in the file by every write.
    pub problems: Vec<String>,
    /// The file declares a schema this binary doesn't understand, so
    /// it is neither read nor written.
    pub unsupported: bool,
}

#[derive(Debug)]
pub enum SlotError {
    Io(io::Error),
    Parse(String),
    /// The file's schema version isn't this binary's.
    UnsupportedSchema(u32),
    DuplicateId(String),
    NotFound(String),
    Invalid(String),
}

impl fmt::Display for SlotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "{err}"),
            Self::Parse(message) => write!(f, "malformed pins file: {message}"),
            Self::UnsupportedSchema(version) => write!(
                f,
                "pins file has schema_version {version}; this binary reads {PINS_SCHEMA_VERSION}"
            ),
            Self::DuplicateId(id) => write!(f, "a pin named `{id}` already exists"),
            Self::NotFound(id) => write!(f, "no pin named `{id}`"),
            Self::Invalid(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SlotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for SlotError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

/// Where a moved slot lands.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Placement {
    Top,
    Bottom,
    Before(String),
    After(String),
    /// One place toward the top (`-1`) or bottom (`1`).
    Step(i32),
}

/// What a write did.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotWrite {
    pub path: PathBuf,
    /// `false` when the call was a no-op.
    pub changed: bool,
    pub slot_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SlotStore {
    path: PathBuf,
}

impl SlotStore {
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// `$XDG_DATA_HOME/conspectus/pins.toml`, falling back to
    /// `$HOME/.local/share/conspectus/pins.toml`.
    pub fn from_env() -> Option<Self> {
        let data = env_path("XDG_DATA_HOME")
            .or_else(|| env_path("HOME").map(|home| home.join(".local").join("share")))?;
        Some(Self::at(data.join("conspectus").join(PINS_FILE_NAME)))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// An I/O error that names the file it concerns.
    fn io_error(&self, err: io::Error) -> SlotError {
        SlotError::Io(io::Error::new(
            err.kind(),
            format!("{}: {err}", self.path.display()),
        ))
    }

    /// Read the slots. A missing file is no slots.
    pub fn load(&self) -> SlotLoad {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return SlotLoad::default(),
            Err(err) => {
                return SlotLoad {
                    problems: vec![format!("{}: {err}", self.path.display())],
                    ..SlotLoad::default()
                };
            }
        };
        parse_slots(&text, &self.path)
    }

    /// Append a new slot.
    pub fn add(&self, slot: PinSlot) -> Result<SlotWrite, SlotError> {
        validate_slot(&slot)?;
        self.edit(|doc, load| {
            if load.slots.iter().any(|existing| existing.id == slot.id)
                || find_index(slots_of(doc), &slot.id).is_some()
            {
                return Err(SlotError::DuplicateId(slot.id.clone()));
            }
            let mut table = Table::new();
            apply_slot(&mut table, &slot);
            ensure_slots(doc).push(table);
            Ok(true)
        })
    }

    /// Put a removed slot back at `index` in the order (counting every
    /// table in the file, as [`PinSlot::order`] does), or at the end when
    /// the file is shorter now. Refuses when the id has been taken since.
    /// The other slots keep their comments and formatting.
    pub fn restore(&self, slot: &PinSlot, index: usize) -> Result<SlotWrite, SlotError> {
        validate_slot(slot)?;
        self.edit(|doc, load| {
            if load.slots.iter().any(|existing| existing.id == slot.id)
                || find_index(slots_of(doc), &slot.id).is_some()
            {
                return Err(SlotError::DuplicateId(slot.id.clone()));
            }
            let tables = ensure_slots(doc);
            let mut order: Vec<Table> = (0..tables.len())
                .filter_map(|i| tables.get(i).cloned())
                .collect();
            let mut table = Table::new();
            apply_slot(&mut table, slot);
            order.insert(index.min(order.len()), table);
            // A parsed table renders at its stored position, so hand
            // every slot a fresh, increasing one (see `move_slot`).
            let mut next = order.iter().filter_map(Table::position).min().unwrap_or(1);
            let mut rebuilt = ArrayOfTables::new();
            for mut table in order {
                assign_positions(&mut table, &mut next);
                rebuilt.push(table);
            }
            *tables = rebuilt;
            Ok(true)
        })
    }

    /// Change a slot in place. `change` sees the slot as loaded and
    /// returns whether it changed anything; the id can't change here
    /// (use [`Self::rename`]).
    pub fn update(
        &self,
        id: &str,
        change: impl FnOnce(&mut PinSlot) -> Result<(), SlotError>,
    ) -> Result<SlotWrite, SlotError> {
        self.edit(|doc, load| {
            let mut slot = load
                .slots
                .iter()
                .find(|slot| slot.id == id)
                .cloned()
                .ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            let before = slot.clone();
            change(&mut slot)?;
            if slot.id != before.id {
                return Err(SlotError::Invalid("update can't rename a slot".into()));
            }
            validate_slot(&slot)?;
            if slot == before {
                return Ok(false);
            }
            let index =
                find_index(slots_of(doc), id).ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            let tables = ensure_slots(doc);
            let table = tables
                .get_mut(index)
                .ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            apply_slot(table, &slot);
            Ok(true)
        })
    }

    /// The names [`SlotStore::rename`] would refuse, checked without
    /// writing: a new id that is not a pin id or that another slot has. A
    /// caller that renames something else first (a live mux's session)
    /// asks this before it does; the rename checks again under the file's
    /// lock.
    pub fn check_rename(&self, id: &str, new_id: &str) -> Result<(), SlotError> {
        validate_id(new_id)?;
        if id != new_id && self.load().slots.iter().any(|slot| slot.id == new_id) {
            return Err(SlotError::DuplicateId(new_id.to_string()));
        }
        Ok(())
    }

    /// Rename a slot, keeping its place.
    pub fn rename(&self, id: &str, new_id: &str) -> Result<SlotWrite, SlotError> {
        validate_id(new_id)?;
        if id == new_id {
            return Ok(SlotWrite {
                path: self.path.clone(),
                changed: false,
                slot_count: self.load().slots.len(),
            });
        }
        self.edit(|doc, load| {
            if load.slots.iter().any(|slot| slot.id == new_id) {
                return Err(SlotError::DuplicateId(new_id.to_string()));
            }
            let index =
                find_index(slots_of(doc), id).ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            let table = ensure_slots(doc)
                .get_mut(index)
                .ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            table.insert("id", Item::Value(Value::from(new_id)));
            Ok(true)
        })
    }

    /// Remove a slot. Removing one that isn't there is a no-op.
    pub fn remove(&self, id: &str) -> Result<SlotWrite, SlotError> {
        self.edit(|doc, _| {
            let Some(index) = find_index(slots_of(doc), id) else {
                return Ok(false);
            };
            ensure_slots(doc).remove(index);
            Ok(true)
        })
    }

    /// Move a slot within the order.
    pub fn move_slot(&self, id: &str, placement: &Placement) -> Result<SlotWrite, SlotError> {
        self.edit(|doc, _| {
            let tables = ensure_slots(doc);
            let ids: Vec<String> = (0..tables.len())
                .map(|i| table_id(tables.get(i)).unwrap_or_default())
                .collect();
            let from = ids
                .iter()
                .position(|candidate| candidate == id)
                .ok_or_else(|| SlotError::NotFound(id.to_string()))?;
            let anchor = |name: &str| {
                ids.iter()
                    .position(|candidate| candidate == name)
                    .ok_or_else(|| SlotError::NotFound(name.to_string()))
            };
            let to = match placement {
                Placement::Top => 0,
                Placement::Bottom => ids.len() - 1,
                Placement::Step(delta) => from
                    .saturating_add_signed(*delta as isize)
                    .min(ids.len() - 1),
                Placement::Before(name) | Placement::After(name) => {
                    let at = anchor(name)?;
                    if at == from {
                        return Err(SlotError::Invalid(
                            "a pin can't move relative to itself".into(),
                        ));
                    }
                    let mut target = if matches!(placement, Placement::Before(_)) {
                        at
                    } else {
                        at + 1
                    };
                    if from < target {
                        target -= 1;
                    }
                    target
                }
            };
            if to == from {
                return Ok(false);
            }
            let mut order: Vec<Table> = (0..tables.len())
                .filter_map(|i| tables.get(i).cloned())
                .collect();
            let moved = order.remove(from);
            order.insert(to, moved);
            // A parsed table renders at its stored position, so hand
            // the moved slots fresh, increasing ones (starting where
            // the first slot began) and let the new order stand.
            let mut next = (0..tables.len())
                .filter_map(|i| tables.get(i).and_then(Table::position))
                .min()
                .unwrap_or(1);
            let mut rebuilt = ArrayOfTables::new();
            for mut table in order {
                assign_positions(&mut table, &mut next);
                rebuilt.push(table);
            }
            *tables = rebuilt;
            Ok(true)
        })
    }

    /// The read-modify-write cycle: lock, read the document, let
    /// `change` edit it, and write it back if it said it changed
    /// something.
    fn edit(
        &self,
        change: impl FnOnce(&mut DocumentMut, &SlotLoad) -> Result<bool, SlotError>,
    ) -> Result<SlotWrite, SlotError> {
        if let Some(dir) = self.path.parent() {
            create_private_dir_all(dir).map_err(|err| self.io_error(err))?;
        }
        let lock = open_private_lock(&self.path.with_extension("toml.lock"))
            .map_err(|err| self.io_error(err))?;
        lock.lock().map_err(|err| self.io_error(err))?;

        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == io::ErrorKind::NotFound => String::new(),
            Err(err) => return Err(self.io_error(err)),
        };
        let load = parse_slots(&text, &self.path);
        if load.unsupported {
            if let Err(err) = toml::from_str::<toml::Table>(&text) {
                return Err(SlotError::Parse(format!("{}: {err}", self.path.display())));
            }
            return Err(SlotError::UnsupportedSchema(schema_version_of(&text)));
        }
        let mut doc = text
            .parse::<DocumentMut>()
            .map_err(|err| SlotError::Parse(err.to_string()))?;
        let changed = change(&mut doc, &load)?;
        let slot_count = slots_of(&doc).map_or(0, ArrayOfTables::len);
        if changed {
            stamp_schema(&mut doc);
            write_private_atomic(&self.path, &doc.to_string()).map_err(|err| self.io_error(err))?;
        }
        drop(lock);
        Ok(SlotWrite {
            path: self.path.clone(),
            changed,
            slot_count,
        })
    }
}

fn env_path(key: &str) -> Option<PathBuf> {
    // A relative base directory is ignored, as the XDG specification
    // says, so slots never land in whatever directory the command ran in.
    match std::env::var_os(key).map(PathBuf::from) {
        Some(path) if path.is_absolute() => Some(path),
        _ => None,
    }
}

fn schema_version_of(text: &str) -> u32 {
    toml::from_str::<RawFile>(text)
        .ok()
        .and_then(|file| file.pins)
        .and_then(|pins| pins.schema_version)
        .unwrap_or(0)
}

/// Parse the slots out of `text`.
pub fn parse_slots(text: &str, path: &Path) -> SlotLoad {
    let mut load = SlotLoad::default();
    if text.trim().is_empty() {
        return load;
    }
    let file: RawFile = match toml::from_str(text) {
        Ok(file) => file,
        Err(err) => {
            load.problems
                .push(format!("{}: malformed TOML: {err}", path.display()));
            load.unsupported = true;
            return load;
        }
    };
    let Some(pins) = file.pins else {
        return load;
    };
    if pins.schema_version != Some(PINS_SCHEMA_VERSION) {
        let found = pins
            .schema_version
            .map_or_else(|| "is missing".to_string(), |v| format!("is {v}"));
        load.problems.push(format!(
            "{}: [pins] schema_version {found}, not {PINS_SCHEMA_VERSION}; the file is left alone",
            path.display(),
        ));
        load.unsupported = true;
        return load;
    }
    let mut seen = std::collections::BTreeSet::new();
    for (index, raw) in pins.slots.into_iter().enumerate() {
        let slot = match raw.try_into::<RawSlot>() {
            Ok(raw) => slot_from_raw(raw, u32::try_from(index).unwrap_or(u32::MAX)),
            Err(err) => Err(err.to_string()),
        };
        match slot {
            Ok(slot) if seen.insert(slot.id.clone()) => load.slots.push(slot),
            Ok(slot) => load.problems.push(format!(
                "{}: entry {index} repeats the id `{}`; skipped",
                path.display(),
                slot.id
            )),
            Err(message) => load.problems.push(format!(
                "{}: entry {index}: {message}; skipped",
                path.display()
            )),
        }
    }
    load
}

fn slot_from_raw(raw: RawSlot, order: u32) -> Result<PinSlot, String> {
    let kind = PinKind::parse(&raw.kind).ok_or_else(|| format!("unknown kind `{}`", raw.kind))?;
    validate_id(&raw.id).map_err(|err| err.to_string())?;
    let pinned_at = match raw.pinned_at {
        None => None,
        Some(toml::Value::Datetime(datetime)) => Some(datetime.to_string()),
        Some(toml::Value::String(text)) => Some(text),
        Some(other) => return Err(format!("pinned_at must be a datetime, not {other}")),
    };
    let slot = PinSlot {
        id: raw.id,
        kind,
        target: raw.target.filter(|target| !target.is_empty()),
        pinned_at,
        overrides: raw.overrides,
        snapshot: raw.snapshot,
        order,
        legacy: false,
    };
    validate_slot(&slot).map_err(|err| err.to_string())?;
    Ok(slot)
}

/// A slot id is a non-empty, single-line name.
pub fn validate_id(id: &str) -> Result<(), SlotError> {
    if id.trim().is_empty() || id != id.trim() || id.contains(['\n', '\r']) {
        return Err(SlotError::Invalid(format!(
            "`{id}` isn't a usable pin name (non-empty, no surrounding spaces or newlines)"
        )));
    }
    Ok(())
}

/// Check what the store would refuse to write: the id, the target's
/// shape, and each override's name and type for the slot's kind.
pub fn validate_slot(slot: &PinSlot) -> Result<(), SlotError> {
    validate_id(&slot.id)?;
    slot.parsed_target().map_err(SlotError::Invalid)?;
    for (name, value) in &slot.overrides {
        if !slot.kind.allows_override(name) {
            return Err(SlotError::Invalid(format!(
                "a {} pin has no overridable field `{name}` (allowed: label, {})",
                slot.kind.as_str(),
                slot.kind.overridable_fields().join(", ")
            )));
        }
        check_type(name, value)?;
    }
    for (name, value) in &slot.snapshot {
        check_type(name, value)?;
    }
    Ok(())
}

fn check_type(name: &str, value: &FieldValue) -> Result<(), SlotError> {
    let wants_list = PinKind::field_is_list(name);
    match (wants_list, value) {
        (true, FieldValue::List(_)) | (false, FieldValue::Text(_)) => Ok(()),
        (true, _) => Err(SlotError::Invalid(format!("`{name}` must be a list"))),
        (false, _) => Err(SlotError::Invalid(format!("`{name}` must be text"))),
    }
}

// --- toml_edit plumbing ------------------------------------------------

fn slots_of(doc: &DocumentMut) -> Option<&ArrayOfTables> {
    doc.get("pins")?
        .as_table()?
        .get("slots")?
        .as_array_of_tables()
}

fn ensure_slots(doc: &mut DocumentMut) -> &mut ArrayOfTables {
    if !doc.get("pins").is_some_and(Item::is_table) {
        doc["pins"] = Item::Table(Table::new());
    }
    let pins = doc["pins"].as_table_mut().expect("pins is a table");
    if !pins.get("slots").is_some_and(Item::is_array_of_tables) {
        pins.insert("slots", Item::ArrayOfTables(ArrayOfTables::new()));
    }
    pins["slots"]
        .as_array_of_tables_mut()
        .expect("slots is an array of tables")
}

fn stamp_schema(doc: &mut DocumentMut) {
    ensure_slots(doc);
    let pins = doc["pins"].as_table_mut().expect("pins is a table");
    pins.set_implicit(true);
    pins.insert(
        "schema_version",
        Item::Value(Value::from(i64::from(PINS_SCHEMA_VERSION))),
    );
}

fn assign_positions(table: &mut Table, next: &mut usize) {
    table.set_position(*next);
    *next += 1;
    for (_, item) in table.iter_mut() {
        if let Item::Table(sub) = item {
            assign_positions(sub, next);
        }
    }
}

fn table_id(table: Option<&Table>) -> Option<String> {
    Some(table?.get("id")?.as_str()?.to_string())
}

fn find_index(slots: Option<&ArrayOfTables>, id: &str) -> Option<usize> {
    let slots = slots?;
    (0..slots.len()).find(|&i| table_id(slots.get(i)).as_deref() == Some(id))
}

/// Write `slot`'s fields into `table`, keeping the table's decor and
/// any keys the schema doesn't know.
fn apply_slot(table: &mut Table, slot: &PinSlot) {
    table.insert("id", Item::Value(Value::from(slot.id.as_str())));
    table.insert("kind", Item::Value(Value::from(slot.kind.as_str())));
    match &slot.target {
        Some(target) => {
            table.insert("target", Item::Value(Value::from(target.as_str())));
        }
        None => {
            table.remove("target");
        }
    }
    match slot.pinned_at.as_deref() {
        Some(text) => {
            let value = text
                .parse::<toml_edit::Datetime>()
                .map_or_else(|_| Value::from(text), Value::from);
            table.insert("pinned_at", Item::Value(value));
        }
        None => {
            table.remove("pinned_at");
        }
    }
    apply_fields(table, "overrides", &slot.overrides);
    apply_fields(table, "snapshot", &slot.snapshot);
}

fn apply_fields(table: &mut Table, key: &str, fields: &SlotFields) {
    if fields.is_empty() {
        table.remove(key);
        return;
    }
    let mut sub = match table.remove(key) {
        Some(Item::Table(existing)) => existing,
        _ => Table::new(),
    };
    let stale: Vec<String> = sub
        .iter()
        .map(|(name, _)| name.to_string())
        .filter(|name| !fields.contains_key(name))
        .collect();
    for name in stale {
        sub.remove(&name);
    }
    for (name, value) in fields {
        let value = match value {
            FieldValue::Text(text) => Value::from(text.as_str()),
            FieldValue::List(list) => {
                let mut array = Array::new();
                for item in list {
                    array.push(item.as_str());
                }
                Value::Array(array)
            }
        };
        sub.insert(name, Item::Value(value));
    }
    table.insert(key, Item::Table(sub));
}

#[cfg(test)]
#[path = "pin_slots_tests.rs"]
mod tests;
