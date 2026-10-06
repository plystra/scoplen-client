# Architecture

The canonical architecture is `scoplen-docs/11-client-architecture.md`. This page records how this repository realizes it.

## Layers

```text
web/app (React)  ── typed commands, binary channels ──  src-tauri  ──  scoplen-client-core
      │                                                                      │
   web/ui (design system)                                         scoplen-client-platform
```

- `web/app` renders state and sends intents. It never holds secrets, never performs cryptography, and never talks to the network.
- `src-tauri` adapts commands to `scoplen-client-core` and owns windows. Its capability file (`src-tauri/capabilities/main.json`) grants the main window only Tauri's core defaults and the application's own commands. The content security policy in `tauri.conf.json` allows only the application's own resources and IPC.
- `scoplen-client-core` holds every behavior, so it is the same on both platforms and testable without a window.
- `scoplen-client-platform` is the only crate that may contain `unsafe` code or FFI; it is denied by default there too and allowed per module with each block documented.

## IPC

Commands are declared in `src-tauri/src/commands.rs` with `#[specta::specta]`. `pnpm bindings` writes `web/app/src/ipc/bindings.ts` from them; a test fails when the committed file differs from what the commands generate, so the frontend and the core cannot drift.

Streams use Tauri channels carrying raw binary frames: `FrameSender` on the Rust side (`src-tauri/src/frames.rs`) and `frameChannel` on the frontend (`web/app/src/ipc/frames.ts`). Frames arrive intact and in order; nothing is encoded as JSON.

## Local data

`scoplen-client-core::store` is the SQLCipher store. It is opened with a raw 32-byte key, so SQLCipher runs no key derivation of its own; a wrong key fails on the first page. Migrations in `store/migrations.rs` run at startup in one transaction, and a store with a newer schema than the build knows is refused rather than modified. Replicated objects are kept as their deterministic CBOR envelopes with per-field clocks. A local write is stamped with the next device clock, applied to the stored object, merged with the stored version through `scoplen_model::merge`, validated, and committed; subscribers hear about it only after the commit. The store also enforces the 100,000-objects-per-vault limit (D-36). Objects created before sync enrollment have no vault.

`local_key` decides how the database key is protected: by the platform keystore (`scoplen-client-platform::keystore`), by the keystore plus a passphrase envelope (`scoplen_crypto::LocalDatabaseKeyEnvelope`), or by the passphrase alone on a system without a keystore. Setting or changing the passphrase rewraps the same key, so the store is never re-encrypted. `local_data` is the state machine the interface drives: open, needs passphrase, needs a new passphrase, or unreadable. An unreadable store is renamed and kept, never deleted.

`repository` gives typed access to every object type of `04-object-model.md` §4. A record is a read view of a stored object; a change names only the fields it sets, with `Edit::Clear` writing `null` to clear an optional field (D-57) and map changes naming the entries to set or remove. Because a change touches only its own fields, fields this build does not know survive every write. Credential secrets are never part of a record: `credential_secret` reads one when a connection needs it, and a change containing a secret is redacted when formatted. Gateway networks are authored by the organization's server and are read-only on a device.

After every committed change the shell sends the frontend a `storeChanged` event with the type and identifiers of the changed objects; `useStoreChanges` (`web/app/src/ipc/store-events.ts`) subscribes to it. `LocalData` keeps the listener attached to whichever store is open, so events continue after unlocking or starting with empty data.

`SQLCipher` is built with a vendored, statically linked OpenSSL on both platforms, so a build never links a system OpenSSL by accident.

## Localization

Messages are ICU MessageFormat catalogs in `web/app/src/messages`. English (`en.ts`) is the source; every other catalog has its type, so a missing key fails type checking, and the catalog tests format every message. The interface language is chosen in the core (`Locale::negotiate`) from the system's preferred languages: Simplified Chinese for `zh-Hans` and Simplified-script regions, otherwise English. A language setting arrives with Preference objects (roadmap C2).

## Design system

`web/ui/src/styles.css` defines the tokens of `scoplen-docs/18-visual-identity.md` as semantic names (`background`, `foreground`, `primary`, `muted-foreground`, `border`, `ring`, `attention`) for Tailwind. Light and dark follow the system; increased contrast raises secondary text and borders to the full foreground; reduced motion removes transitions and animation. Fonts are bundled; none is loaded from a third-party service. Chinese headings use a subset of Noto Serif SC at weight 600 containing exactly the characters of the Chinese catalog.

## Device-local records

Records that never replicate (`04-object-model.md` §4.9) live in the same encrypted store, in their own tables (migration 2): this device's half of device-bound credentials, as a stored secret or a keystore handle; the device key pair, created at sync enrollment; the session history, limited to the 1,000 most recent sessions; saved scrollback; and window geometry. None of them is an object, so sync never sends them. Secrets come back as `SecretVec`, which is zeroized when dropped and redacted when formatted. The main window's size and position are saved when it closes and restored at the next launch if they are still on a display.

## No network while sync is disabled

`scripts/check-offline.sh` runs the built application on macOS under a sandbox profile that kills the process on any IP network operation. The application must open its local data, render, and stay idle for 15 seconds; a control run of `curl` under the same profile must be killed. CI runs it on every change. On Windows the same core runs, but no equivalent check exists on hosted runners.

## Launch verification

With `SPL_SMOKE_TEST=1`, the application exits with status 0 once the frontend reports that its first screen rendered (the `shell_ready` command), or `SPL_SMOKE_LINGER_SECS` seconds after that, and with status 1 after 90 seconds otherwise. CI runs a release build this way on macOS and Windows.
