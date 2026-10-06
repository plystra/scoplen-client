// SPDX-License-Identifier: Apache-2.0

//! Reading typed values from object fields and writing field changes.
//!
//! Readers treat a cleared (`null`) field as absent (D-57). Writers produce
//! the field paths and values of one change; clearing an optional scalar
//! writes `null`, and removing a map entry writes `null` to that entry.

use std::collections::{BTreeMap, BTreeSet};

use scoplen_model::{FieldPath, MapKey, Object, cbor::Value};
use uuid::Uuid;

use super::RecordError;

/// A change to one optional field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Edit<T> {
    /// Leave the field as it is.
    #[default]
    Keep,
    /// Set the field.
    Set(T),
    /// Clear the field.
    Clear,
}

impl<T> Edit<T> {
    /// `Set` for `Some`, `Clear` for `None`.
    pub fn from_option(value: Option<T>) -> Edit<T> {
        value.map_or(Edit::Clear, Edit::Set)
    }
}

/// Field values of one change.
#[derive(Default)]
pub(crate) struct Writes(pub(crate) Vec<(FieldPath, Value)>);

impl Writes {
    pub(crate) fn set(&mut self, field: u64, value: Option<Value>) {
        if let Some(value) = value {
            self.0.push((FieldPath::Field(field), value));
        }
    }

    pub(crate) fn edit<T>(&mut self, field: u64, edit: Edit<T>, encode: impl FnOnce(T) -> Value) {
        match edit {
            Edit::Keep => {}
            Edit::Set(value) => self.0.push((FieldPath::Field(field), encode(value))),
            Edit::Clear => self.0.push((FieldPath::Field(field), Value::Null)),
        }
    }

    /// Sets or removes map entries; `None` removes the entry.
    pub(crate) fn entries<K, V>(
        &mut self,
        field: u64,
        entries: BTreeMap<K, Option<V>>,
        key: impl Fn(K) -> MapKey,
        value: impl Fn(V) -> Value,
    ) -> Result<(), RecordError> {
        for (k, v) in entries {
            let path = FieldPath::map_entry(field, key(k)).map_err(RecordError::Model)?;
            self.0.push((path, v.map_or(Value::Null, &value)));
        }
        Ok(())
    }
}

pub(crate) fn text_value(text: String) -> Value {
    Value::Text(text)
}

pub(crate) fn uuid_value(id: Uuid) -> Value {
    Value::Bytes(id.as_bytes().to_vec())
}

pub(crate) fn uuid_key(id: Uuid) -> MapKey {
    MapKey::Bytes(id.as_bytes().to_vec())
}

pub(crate) fn text_key(key: String) -> MapKey {
    MapKey::Text(key)
}

/// A value of a field that is not cleared.
fn value(object: &Object, field: u64) -> Option<&Value> {
    object.set_field(field).map(|entry| &entry.value)
}

pub(crate) fn text(object: &Object, field: u64) -> Result<Option<String>, RecordError> {
    match value(object, field) {
        None => Ok(None),
        Some(Value::Text(text)) => Ok(Some(text.clone())),
        Some(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

pub(crate) fn required_text(object: &Object, field: u64) -> Result<String, RecordError> {
    text(object, field)?.ok_or(RecordError::Shape(object.object_type, field))
}

pub(crate) fn uint(object: &Object, field: u64) -> Result<Option<u64>, RecordError> {
    match value(object, field) {
        None => Ok(None),
        Some(Value::UInt(n)) => Ok(Some(*n)),
        Some(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

pub(crate) fn port(object: &Object, field: u64) -> Result<Option<u16>, RecordError> {
    uint(object, field)?
        .map(|n| u16::try_from(n).map_err(|_| RecordError::Shape(object.object_type, field)))
        .transpose()
}

pub(crate) fn flag(object: &Object, field: u64) -> Result<bool, RecordError> {
    match value(object, field) {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

pub(crate) fn uuid(object: &Object, field: u64) -> Result<Option<Uuid>, RecordError> {
    match value(object, field) {
        None => Ok(None),
        Some(Value::Bytes(bytes)) => Uuid::from_slice(bytes)
            .map(Some)
            .map_err(|_| RecordError::Shape(object.object_type, field)),
        Some(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

pub(crate) fn bytes(object: &Object, field: u64) -> Result<Option<Vec<u8>>, RecordError> {
    match value(object, field) {
        None => Ok(None),
        Some(Value::Bytes(bytes)) => Ok(Some(bytes.clone())),
        Some(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

/// The live entries of a decomposed map field, skipping removed ones.
pub(crate) fn map_entries(object: &Object, field: u64) -> impl Iterator<Item = (&MapKey, &Value)> {
    object.field_entries(field).filter_map(|(path, entry)| match path {
        FieldPath::MapEntry { key, .. } if !matches!(entry.value, Value::Null) => {
            Some((key, &entry.value))
        }
        _ => None,
    })
}

pub(crate) fn text_map(
    object: &Object,
    field: u64,
) -> Result<BTreeMap<String, String>, RecordError> {
    map_entries(object, field)
        .map(|(key, value)| match (key, value) {
            (MapKey::Text(k), Value::Text(v)) => Ok((k.clone(), v.clone())),
            _ => Err(RecordError::Shape(object.object_type, field)),
        })
        .collect()
}

pub(crate) fn text_set(object: &Object, field: u64) -> Result<BTreeSet<String>, RecordError> {
    map_entries(object, field)
        .filter(|(_, value)| matches!(value, Value::Bool(true)))
        .map(|(key, _)| match key {
            MapKey::Text(k) => Ok(k.clone()),
            MapKey::Bytes(_) => Err(RecordError::Shape(object.object_type, field)),
        })
        .collect()
}

pub(crate) fn uuid_set(object: &Object, field: u64) -> Result<BTreeSet<Uuid>, RecordError> {
    map_entries(object, field)
        .filter(|(_, value)| matches!(value, Value::Bool(true)))
        .map(|(key, _)| map_uuid(object, field, key))
        .collect()
}

pub(crate) fn map_uuid(object: &Object, field: u64, key: &MapKey) -> Result<Uuid, RecordError> {
    match key {
        MapKey::Bytes(bytes) => {
            Uuid::from_slice(bytes).map_err(|_| RecordError::Shape(object.object_type, field))
        }
        MapKey::Text(_) => Err(RecordError::Shape(object.object_type, field)),
    }
}

/// Reads an integer-keyed nested map such as a session spec.
pub(crate) fn nested(value: &Value, key: u64) -> Option<&Value> {
    match value {
        Value::Map(entries) => {
            entries.iter().find(|(k, _)| matches!(k, Value::UInt(n) if *n == key)).map(|(_, v)| v)
        }
        _ => None,
    }
}
