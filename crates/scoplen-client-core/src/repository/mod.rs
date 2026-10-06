// SPDX-License-Identifier: Apache-2.0

//! Typed access to each object type (`scoplen-docs/11-client-architecture.md` §2).
//!
//! A record is a typed view of a stored object; a change names only the
//! fields it sets, so fields this build does not know are never lost. Every
//! change goes through [`Store::write`], which merges it with the stored
//! object, so local edits and remote changes combine by the same rules.

mod fields;
mod records;

use std::marker::PhantomData;

use scoplen_model::{ModelError, Object, ObjectType};
use uuid::Uuid;

pub use self::fields::Edit;
pub use self::records::*;
use crate::store::{LocalWrite, Store, StoreError};

/// A failure of a repository operation.
#[derive(Debug, thiserror::Error)]
pub enum RecordError {
    /// A stored field does not have the shape its type requires.
    #[error("field {1} of a {0} object has an unexpected shape")]
    Shape(ObjectType, u64),
    /// A change cannot be expressed in the object model.
    #[error(transparent)]
    Model(ModelError),
    /// The object exists but has another type.
    #[error("object {id} is a {actual}, not a {expected}")]
    WrongType {
        /// The object.
        id: Uuid,
        /// Its type.
        actual: ObjectType,
        /// The type asked for.
        expected: ObjectType,
    },
    /// Objects of this type are authored by the organization's server, not
    /// on this device (`04-object-model.md` §1).
    #[error("{0} objects are read-only on this device")]
    ReadOnly(ObjectType),
    /// The store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// A typed view of one object type.
pub trait Record: Sized {
    /// The registry type.
    const TYPE: ObjectType;
    /// The change that creates or updates a record of this type.
    type Change: Change;
    /// Reads a record from a stored object of this type.
    fn read(object: &Object) -> Result<Self, RecordError>;
}

/// The fields one change sets.
pub trait Change {
    /// The field writes of this change.
    fn into_writes(
        self,
    ) -> Result<Vec<(scoplen_model::FieldPath, scoplen_model::cbor::Value)>, RecordError>;
}

/// Common to every record: its identity, and whether it came back after
/// being deleted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    /// The object's identifier.
    pub id: Uuid,
    /// A change made after the object was deleted brought it back; the
    /// interface presents it as restored (`04-object-model.md` §5 rule 3).
    pub restored: bool,
}

impl Meta {
    pub(crate) fn of(object: &Object) -> Meta {
        Meta { id: object.id, restored: object.is_resurrected() }
    }
}

/// Typed access to the records of one type in a store.
pub struct Repository<'s, R> {
    store: &'s Store,
    record: PhantomData<R>,
}

impl<'s, R: Record> Repository<'s, R> {
    /// The records of type `R` in `store`.
    pub fn new(store: &'s Store) -> Self {
        Repository { store, record: PhantomData }
    }

    /// Every record that is not deleted, oldest first.
    pub fn list(&self) -> Result<Vec<R>, RecordError> {
        self.store.list(R::TYPE)?.iter().map(R::read).collect()
    }

    /// The record, or `None` if it does not exist or is deleted.
    pub fn get(&self, id: Uuid) -> Result<Option<R>, RecordError> {
        match self.store.get(id)? {
            Some(object) if object.is_tombstoned() => Ok(None),
            Some(object) => {
                self.check_type(&object)?;
                R::read(&object).map(Some)
            }
            None => Ok(None),
        }
    }

    /// Creates a record from a change that sets at least its required fields.
    pub fn create(&self, change: R::Change) -> Result<R, RecordError> {
        self.writable()?;
        let fields = change.into_writes()?;
        R::read(&self.store.write(LocalWrite { id: None, object_type: R::TYPE, fields })?)
    }

    /// Applies a change to an existing record.
    pub fn update(&self, id: Uuid, change: R::Change) -> Result<R, RecordError> {
        self.writable()?;
        if let Some(object) = self.store.get(id)? {
            self.check_type(&object)?;
        }
        let fields = change.into_writes()?;
        R::read(&self.store.write(LocalWrite { id: Some(id), object_type: R::TYPE, fields })?)
    }

    /// Deletes a record; it stays restorable until sync purges it.
    pub fn delete(&self, id: Uuid) -> Result<(), RecordError> {
        self.writable()?;
        if let Some(object) = self.store.get(id)? {
            self.check_type(&object)?;
        }
        Ok(self.store.delete(id)?)
    }

    fn writable(&self) -> Result<(), RecordError> {
        if R::TYPE == ObjectType::GATEWAY_NETWORK {
            Err(RecordError::ReadOnly(R::TYPE))
        } else {
            Ok(())
        }
    }

    fn check_type(&self, object: &Object) -> Result<(), RecordError> {
        if object.object_type == R::TYPE {
            Ok(())
        } else {
            Err(RecordError::WrongType {
                id: object.id,
                actual: object.object_type,
                expected: R::TYPE,
            })
        }
    }
}

#[cfg(test)]
mod tests;
