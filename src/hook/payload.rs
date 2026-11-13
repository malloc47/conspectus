//! A hook payload read against the fields its harness's adapter reads
//! (ADR 0206 decisions 10 to 12).
//!
//! `conspectus hook write` parses its stdin with [`parse`] and the
//! adapter's [`HarnessAdapter::hook_payload_fields`]. Each field the
//! adapter names is kept, as deep as it names fields inside it, and every
//! other value is skipped as the parser passes over it, never built: a
//! `PostToolUse` whose `tool_response` holds a megabyte file costs a scan
//! of those bytes and no copy of them, on every tool call.
//!
//! The adapter then reads the result as it would read the whole payload.
//! A field kept is as the payload had it; a field not named is absent,
//! which is no loss, since the adapter never reads it. A named field of
//! the wrong kind reads as `null`: one read as a string, number, or flag
//! that holds an object or a list, or one read for its fields that holds
//! a string, a number, or a flag. The adapters read `null` there as they
//! read any value of the wrong kind: as absent.
//!
//! [`HarnessAdapter::hook_payload_fields`]: crate::discovery::harness::HarnessAdapter::hook_payload_fields

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde_json::{Map, Value};

/// One field of a hook payload that an adapter reads.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadField {
    /// The field's key.
    pub name: &'static str,
    /// The fields read inside it: none for a field read as a string, a
    /// number, or a flag; else those of the object it holds, or of each
    /// object in the list it holds.
    pub fields: &'static [PayloadField],
}

impl PayloadField {
    /// A field read as a string, a number, or a flag.
    pub const fn scalar(name: &'static str) -> Self {
        Self { name, fields: &[] }
    }

    /// A field read for `fields` inside the object, or each object of
    /// the list, it holds.
    pub const fn nested(name: &'static str, fields: &'static [PayloadField]) -> Self {
        Self { name, fields }
    }
}

/// The fields the default attribution record reads: the session, its
/// directory and transcript, and the event's name.
pub const SESSION_FIELDS: [PayloadField; 4] = [
    PayloadField::scalar("session_id"),
    PayloadField::scalar("cwd"),
    PayloadField::scalar("transcript_path"),
    PayloadField::scalar("hook_event_name"),
];

/// `input`, a JSON hook payload, with only `fields` kept. `None` when
/// `input` is not one JSON value, as a whole-value parse would refuse it.
/// A payload that is not an object reads as `null`.
pub fn parse(input: &[u8], fields: &[PayloadField]) -> Option<Value> {
    let mut deserializer = serde_json::Deserializer::from_slice(input);
    let value = Shape::Fields(fields).deserialize(&mut deserializer).ok()?;
    deserializer.end().ok()?;
    Some(value)
}

/// What a position in the payload is read as.
#[derive(Clone, Copy)]
enum Shape<'a> {
    /// A string, a number, a flag, or null.
    Scalar,
    /// An object, or a list of objects, of which these fields are read.
    Fields(&'a [PayloadField]),
}

impl<'a> Shape<'a> {
    fn of(field: &'a PayloadField) -> Self {
        if field.fields.is_empty() {
            Self::Scalar
        } else {
            Self::Fields(field.fields)
        }
    }

    /// `value`, a scalar, where it is read as one; else null.
    fn scalar(self, value: Value) -> Value {
        match self {
            Self::Scalar => value,
            Self::Fields(_) => Value::Null,
        }
    }
}

impl<'de> DeserializeSeed<'de> for Shape<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Shape<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Value, E> {
        Ok(self.scalar(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Value, E> {
        Ok(self.scalar(Value::from(value)))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Value, E> {
        Ok(self.scalar(Value::from(value)))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Value, E> {
        Ok(self.scalar(Value::from(value)))
    }

    fn visit_str<E>(self, value: &str) -> Result<Value, E> {
        Ok(match self {
            Self::Scalar => Value::String(value.to_string()),
            Self::Fields(_) => Value::Null,
        })
    }

    fn visit_string<E>(self, value: String) -> Result<Value, E> {
        Ok(self.scalar(Value::String(value)))
    }

    fn visit_unit<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_none<E>(self) -> Result<Value, E> {
        Ok(Value::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        self.deserialize(deserializer)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Value, A::Error> {
        if let Self::Scalar = self {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            return Ok(Value::Null);
        }
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self)? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Value, A::Error> {
        let Self::Fields(fields) = self else {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            return Ok(Value::Null);
        };
        let mut kept = Map::new();
        while let Some(field) = map.next_key_seed(Key(fields))? {
            match field {
                Some(field) => {
                    let value = map.next_value_seed(Shape::of(field))?;
                    kept.insert(field.name.to_string(), value);
                }
                None => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(Value::Object(kept))
    }
}

/// A key of an object, looked up among the fields read in it without
/// being kept.
struct Key<'a>(&'a [PayloadField]);

impl<'de, 'a> DeserializeSeed<'de> for Key<'a> {
    type Value = Option<&'a PayloadField>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }
}

impl<'a> Visitor<'_> for Key<'a> {
    type Value = Option<&'a PayloadField>;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an object key")
    }

    fn visit_str<E>(self, key: &str) -> Result<Self::Value, E> {
        Ok(self.0.iter().find(|field| field.name == key))
    }
}

#[cfg(test)]
#[path = "payload_tests.rs"]
mod tests;
