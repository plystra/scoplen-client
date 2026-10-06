// SPDX-License-Identifier: Apache-2.0

//! Platform-independent logic of the Scoplen desktop client.
//!
//! Every client behavior is implemented here, below the user interface and
//! above the platform layer (`scoplen-docs/11-client-architecture.md` §1), so
//! that it is the same on every desktop platform and testable without a window.

pub mod locale;
