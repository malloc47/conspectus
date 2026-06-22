//! rkyv `with`-adapter glue for model fields whose native types
//! don't have rkyv impls of their own.
//!
//! See ADR 0083 §"Model derives" for the rationale. The model's
//! [`super::Metadata`] alias (`BTreeMap<String, serde_json::Value>`)
//! cannot be archived directly because `serde_json::Value` is not a
//! rkyv-aware type and conspectus does not own it. Instead, the
//! `MetadataAsJson` adapter below encodes the whole map as a single
//! JSON-text `String` in the archive. The live model API stays
//! exactly as it is — producers keep calling `.insert("key",
//! Value::String(...))` and consumers keep calling
//! `.get("key")?.as_str()`. The Value→bytes→Value conversion is
//! paid once per archive cycle on the daemon side and once per
//! deserialize on the reader side, never per access.

use std::collections::BTreeMap;

use rkyv::Archive;
use rkyv::with::{ArchiveWith, DeserializeWith, SerializeWith};
use serde_json::Value;

/// rkyv `#[rkyv(with = MetadataAsJson)]` adapter for
/// [`super::Metadata`] (`BTreeMap<String, serde_json::Value>`) and
/// any field with the same concrete type. Stores the JSON
/// serialization of the whole map as a `String` in the archive.
pub struct MetadataAsJson;

impl ArchiveWith<BTreeMap<String, Value>> for MetadataAsJson {
    type Archived = <String as Archive>::Archived;
    type Resolver = <String as Archive>::Resolver;

    fn resolve_with(
        field: &BTreeMap<String, Value>,
        resolver: Self::Resolver,
        out: rkyv::Place<Self::Archived>,
    ) {
        // The resolver was produced by `serialize_with`, which is
        // where the JSON encoding cost actually lives. By the time
        // `resolve_with` runs the bytes have already been written;
        // we just need a placeholder that produces the same string
        // for layout. Serialization errors there are bubbled up via
        // `S::Error`, so encoding here is purely positional.
        let text = serde_json::to_string(field).unwrap_or_else(|_| "{}".to_string());
        text.resolve(resolver, out);
    }
}

impl<S> SerializeWith<BTreeMap<String, Value>, S> for MetadataAsJson
where
    S: rkyv::rancor::Fallible + ?Sized,
    S::Error: rkyv::rancor::Source,
    String: rkyv::Serialize<S>,
{
    fn serialize_with(
        field: &BTreeMap<String, Value>,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        let text = serde_json::to_string(field).map_err(rkyv::rancor::Source::new)?;
        rkyv::Serialize::<S>::serialize(&text, serializer)
    }
}

impl<D> DeserializeWith<<String as Archive>::Archived, BTreeMap<String, Value>, D>
    for MetadataAsJson
where
    D: rkyv::rancor::Fallible + ?Sized,
    D::Error: rkyv::rancor::Source,
{
    fn deserialize_with(
        archived: &<String as Archive>::Archived,
        _deserializer: &mut D,
    ) -> Result<BTreeMap<String, Value>, D::Error> {
        let text: &str = archived.as_str();
        serde_json::from_str(text).map_err(rkyv::rancor::Source::new)
    }
}
