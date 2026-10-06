# Scoplen client principles

This repository is a component of Scoplen, a Plystra project, and the full Plystra philosophy applies. The project-level adoption record is [`scoplen-docs/PROJECT_PRINCIPLES.md`](../scoplen-docs/PROJECT_PRINCIPLES.md); this record applies it to the desktop client.

## Review record

- Philosophy version reviewed: Plystra Craft 1.0.1
- Source commit reviewed: `plystra/craft@dd617b370203d5dce1d30b8b81284d16d96a432c`
- Review date and maintainer: 2026-10-07, immoses (Moses Qiu)
- Maturity: Exploration
- Maintenance: Active
- Components: the desktop application for macOS and Windows, its Rust core and platform layer, the `@scoplen/ui` design system, and later the `@scoplen/terminal` package
- Public data surface: none yet. The client sends nothing to the vendor in Phase 1 (`scoplen-docs/16-decision-record.md`, D-32, D-52); user data stays on the device and, when sync is enabled, on the server the user chooses.
- Review status: Reviewed with open gaps

## Applicable standards

The repository applies `plystra-craft`, `plystra-craft-code`, and `plystra-craft-design` from `.agents/skills/`. Source code is Apache-2.0. Shipped JavaScript and Rust dependencies are checked for licenses compatible with that (`pnpm license-check`, `cargo deny`). Interface copy follows `scoplen-docs/11-client-architecture.md` §4.1, and visible changes go through the Craft UI review checklist.

## Open gaps

- The application shell and the encrypted local store exist. Inventory, terminal, connections, sync, and organization features are open roadmap gates, so the product's primary loop cannot yet be used.
- The Windows TPM path of key storage cannot run on hosted CI runners, which have no TPM; only its DPAPI fallback is exercised there.
- The Windows build and launch are defined in CI but have not been verified locally or by a completed CI run.
- An accessibility audit against WCAG 2.2 AA on both platforms is scheduled for roadmap C15; the current evidence is automated axe checks, contrast checks of the tokens, and keyboard tests of the components.
