# Interface and the core

The interface (`web/app`, `web/ui`) and the core (`crates/`, `src-tauri`) are built separately. Each screen states what it needs from the core as a TypeScript contract; the core implements that contract with typed commands. This page lists the contracts, what each operation must do, and how a contract is connected once its commands exist.

The screens never read the store, decide business rules, or hold secrets. Anything the screens must not decide — validation beyond "is this field empty", what a deletion removes, which areas the user has reached — is the core's.

## Seeing the screens

`pnpm --dir web/app dev`, then open `/gallery.html`. The gallery renders every screen and state with sample data (`web/app/src/gallery/sample-inventory.ts`), in English or Chinese, as on macOS or Windows. It is a development tool: it is not built into the application, and its sample data is not a reference implementation of the core.

## Window

`web/app/src/frame.tsx`. A strip of tabs above the content. The first tab is always Hosts; each session adds a tab after it (roadmap C5). On macOS the strip is the title bar: the window is created with `titleBarStyle: "Overlay"` and a hidden title, the strip leaves 84 px for the window controls, and its empty space carries `data-tauri-drag-region`. Windows keeps its native title bar. Settings opens from the gear at the right of the strip.

## Inventory (roadmap C3)

Contract: `web/app/src/inventory/api.ts` (`InventoryApi`). Screens: `inventory/home.tsx` (sidebar, list, details), `inventory/host-details.tsx`, `inventory/add-host.tsx`.

| Operation | What the core does |
| --- | --- |
| `areas()` | Which sidebar sources exist (`01-product-definition.md` §7.6): `favorites` when any host is a favorite, `recent` when the session history names a host, `groups` when any group exists, `keys` and `routes` when the user has reached them (a key used by two logins or opened from a host; a route created). |
| `groups()` | Every group with its parent and host count. |
| `hosts(source, query)` | Hosts from the source (all, favorites, recent, or one group and its subgroups) whose name, address, username, or `key:value` tag contains the query, case-insensitive; favorites first, then by name. Orphaned hosts are not listed. `username`, `loginCount`, and `route` describe the default login, else the only one. |
| `host(id)` | The host with every login, `null` when it no longer exists. A key credential carries its public key. |
| `addHost(host)` | Creates the host, one login, and the credential, with a direct route, as one change (§7.6 tier 0). `name` defaults to the address and `port` to 22. Password and pasted or generated keys are stored as shared-binding credentials; a key file is read and stored the same way; `agent` creates an agent credential. Each `AddHostError` names the field it belongs to; nothing is saved on error. |
| `setFavorite(id, favorite)` | Sets the favorite field. |
| `deleteHost(id)` | Deletes the host, its logins, and unnamed credentials no other login uses, and returns a token. |
| `undoDelete(token)` | Restores everything that deletion removed, for as long as the store keeps the tombstones. |
| `chooseKeyFile()` | Opens the system file picker starting in `~/.ssh`, returns the path or `null`. |
| `onChange(listener)` | Calls the listener after any inventory change; built on the `storeChanged` event (`ipc/store-events.ts`). |

Every operation that can fail returns `Outcome`; a `Failure` carries a reference for support, free of secrets.

## Connecting a contract

1. Add the commands in `src-tauri` with `#[specta::specta]` and run `pnpm bindings`.
2. Write `web/app/src/inventory/core-api.ts`: an `InventoryApi` that calls the generated `commands`, mapping their results to the contract's types where they differ.
3. In `web/app/src/app.tsx`, once the local data is open, render `Frame` with `HostsHome` inside an `InventoryContext.Provider` holding that API. The about text moves into Settings.
4. The screen tests (`web/app/test/hosts.test.tsx`) run against the gallery's sample inventory and describe the expected behavior; add tests for the adapter itself.

A contract change is made in `api.ts`, its screens, and this page together.
