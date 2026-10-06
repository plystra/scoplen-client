// SPDX-License-Identifier: Apache-2.0

//! Versioned schema migrations of the local store.
//!
//! Migrations run at startup, all pending ones in one transaction, so a store
//! is never left between versions (`scoplen-docs/11-client-architecture.md` §2).
//! A migration is never edited once released; a change is a new migration.

/// The ordered migrations. Version `n` is `MIGRATIONS[n - 1]`.
pub(crate) const MIGRATIONS: &[&str] = &[
    // 1: device identity and replicated objects.
    r#"
    CREATE TABLE device (
        singleton  INTEGER PRIMARY KEY CHECK (singleton = 1),
        device_id  BLOB    NOT NULL CHECK (length(device_id) = 16),
        clock      INTEGER NOT NULL,
        created_at INTEGER NOT NULL
    ) STRICT;

    -- Replicated objects in their merged form, as the deterministic CBOR
    -- envelope with per-field clocks (`04-object-model.md` §4). A NULL vault is
    -- this device's own collection before sync enrollment.
    CREATE TABLE objects (
        id         BLOB    PRIMARY KEY CHECK (length(id) = 16),
        vault      BLOB    CHECK (vault IS NULL OR length(vault) = 16),
        type       INTEGER NOT NULL,
        envelope   BLOB    NOT NULL,
        deleted    INTEGER NOT NULL CHECK (deleted IN (0, 1)),
        updated_at INTEGER NOT NULL
    ) STRICT, WITHOUT ROWID;

    CREATE INDEX objects_by_type ON objects (type, deleted);
    CREATE INDEX objects_by_vault ON objects (vault);
    "#,
];
