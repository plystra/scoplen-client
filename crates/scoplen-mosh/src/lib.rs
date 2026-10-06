// SPDX-License-Identifier: Apache-2.0

//! Mosh client protocol for the Scoplen desktop client: bootstrapping
//! `mosh-server` over SSH and the State Synchronization Protocol over UDP
//! (`scoplen-docs/11-client-architecture.md` §9).
//!
//! The protocol is implemented under roadmap gate C12; this crate holds its
//! place in the workspace layout of `scoplen-docs/02-workspace-and-repositories.md` §4.2.
