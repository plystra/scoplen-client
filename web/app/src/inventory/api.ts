// SPDX-License-Identifier: Apache-2.0
//
// What the inventory screens need from the core. The core implements this
// with typed commands (docs/interface.md); the screens never see how. Every
// identifier is a lowercase hyphenated UUID; every text the user typed is
// returned as typed. Nothing here carries a secret back to the interface.

import { createContext, useContext } from "react";

export type Id = string;

/** How a login reaches its host, as the interface names it. */
export type RouteLabel =
  | { kind: "direct" }
  | { kind: "jump"; name: string }
  | { kind: "proxy"; name: string }
  | { kind: "command"; name: string }
  | { kind: "bastion"; name: string };

/** A credential as the interface may show it: never its secret. */
export interface CredentialLabel {
  id: Id;
  kind: "password" | "key" | "agent" | "securityKey" | "deviceKey" | "external" | "certificate";
  /** The name the user gave it, if any. */
  name: string | null;
  /** For keys: the SHA-256 fingerprint as OpenSSH prints it. */
  fingerprint: string | null;
  /** For keys: the key's comment. */
  comment: string | null;
  /** For keys: the public key in `authorized_keys` form, for the user to copy to hosts. */
  publicKey: string | null;
}

/** One row of the host list. */
export interface HostSummary {
  id: Id;
  name: string;
  address: string;
  /** 22 unless set. */
  port: number;
  /** The username of the default (or only) login; null when the host has none. */
  username: string | null;
  loginCount: number;
  /** How the default login connects. */
  route: RouteLabel;
  favorite: boolean;
  tags: Record<string, string>;
  groups: Id[];
  /** Brought back by a change made after it was deleted on another device. */
  restored: boolean;
}

export interface LoginSummary {
  id: Id;
  name: string | null;
  username: string;
  /** Null: any usable credential on this device. */
  credential: CredentialLabel | null;
  route: RouteLabel;
  isDefault: boolean;
}

export interface HostDetails extends HostSummary {
  notes: string | null;
  logins: LoginSummary[];
}

export interface GroupSummary {
  id: Id;
  name: string;
  parent: Id | null;
  hostCount: number;
}

/**
 * Which parts of the inventory the user has reached (`01-product-definition.md`
 * §7.6). A part not reached is not shown in the sidebar.
 */
export interface Areas {
  favorites: boolean;
  recent: boolean;
  groups: boolean;
  keys: boolean;
  routes: boolean;
}

/** Which hosts to list. */
export type HostSource = { kind: "all" } | { kind: "favorites" } | { kind: "recent" } | { kind: "group"; id: Id };

/** How a new host signs in. */
export type SignIn =
  | { kind: "password"; password: string }
  | { kind: "keyFile"; path: string }
  | { kind: "keyText"; key: string }
  | { kind: "generateKey" }
  | { kind: "agent" };

export interface NewHost {
  /** Defaults to the address when null. */
  name: string | null;
  address: string;
  /** Defaults to 22 when null. */
  port: number | null;
  username: string;
  signIn: SignIn;
}

/** Why a new host could not be added; nothing was saved. */
export type AddHostError =
  | { kind: "invalidAddress" }
  | { kind: "invalidPort" }
  | { kind: "emptyUsername" }
  | { kind: "emptyPassword" }
  | { kind: "keyNotFound" }
  | { kind: "notAPrivateKey" }
  | { kind: "publicKey" }
  | { kind: "puttyKey" }
  | { kind: "failed"; reference: string };

export type Outcome<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

/** A failure the user can only retry; nothing changed. */
export interface Failure {
  kind: "failed";
  reference: string;
}

/** A deletion that can be undone for a while. */
export interface Deletion {
  /** Passed to `undoDelete`. */
  token: string;
}

export interface InventoryApi {
  areas(): Promise<Outcome<Areas, Failure>>;
  groups(): Promise<Outcome<GroupSummary[], Failure>>;
  /** Hosts from `source` matching `query` (name, address, username, tags), favorites first, then by name. */
  hosts(source: HostSource, query: string): Promise<Outcome<HostSummary[], Failure>>;
  host(id: Id): Promise<Outcome<HostDetails | null, Failure>>;
  addHost(host: NewHost): Promise<Outcome<HostDetails, AddHostError>>;
  setFavorite(id: Id, favorite: boolean): Promise<Outcome<null, Failure>>;
  /** Deletes the host and its logins, and keys only it used that were never named. */
  deleteHost(id: Id): Promise<Outcome<Deletion, Failure>>;
  undoDelete(token: string): Promise<Outcome<null, Failure>>;
  /** Asks the system for a private key file; null when the user cancels. */
  chooseKeyFile(): Promise<string | null>;
  /** Calls `listener` after any change to the inventory; returns a function that stops it. */
  onChange(listener: () => void): () => void;
}

export const InventoryContext = createContext<InventoryApi | null>(null);

export function useInventory(): InventoryApi {
  const api = useContext(InventoryContext);
  if (!api) throw new Error("useInventory must be used inside InventoryContext");
  return api;
}
