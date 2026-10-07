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

export type RecentSessionKind = "terminal" | "files" | "forward";
export type RecentSessionOutcome = "closed" | "failed";

/** A redacted device-local session entry, newest first. */
export interface RecentSession {
  id: Id;
  hostId: Id;
  hostName: string;
  address: string;
  port: number;
  username: string;
  kind: RecentSessionKind;
  /** Decimal Unix milliseconds from the IPC boundary. */
  startedAt: string;
  endedAt: string | null;
  outcome: RecentSessionOutcome | null;
}

export interface GroupSummary {
  id: Id;
  name: string;
  parent: Id | null;
  hostCount: number;
}

export type CredentialKindInput =
  "password" | "key" | "certificate" | "agent" | "securityKey" | "deviceKey" | "external";
export type CredentialBindingInput = "shared" | "device" | "none";

/** A redacted independent credential. It never includes secret material. */
export interface CredentialSummary {
  id: Id;
  name: string | null;
  kind: CredentialKindInput;
  binding: CredentialBindingInput;
  hasSecret: boolean;
  publicKey: string | null;
  provider: Record<string, string>;
  certificateScope: Id | null;
  profileCount: number;
  routeCount: number;
  restored: boolean;
}

/** Input for creating or replacing a credential. `secret` is write-only. */
export interface CredentialInput {
  name: string | null;
  kind: CredentialKindInput;
  binding: CredentialBindingInput;
  secret: string | null;
  publicKey: string | null;
  provider: Record<string, string>;
  certificateScope: Id | null;
}

export type RouteDefinition =
  | { kind: "jump"; hops: Id[] }
  | { kind: "socks5"; proxy: string; credential: Id | null }
  | { kind: "httpConnect"; proxy: string; credential: Id | null }
  | { kind: "command"; command: string }
  | { kind: "managed"; gatewayNetwork: Id };

export interface RouteSummary {
  id: Id;
  name: string;
  definition: RouteDefinition;
  profileCount: number;
  restored: boolean;
}

export interface RouteInput {
  name: string;
  definition: RouteDefinition;
}

export interface AccessProfileSummary {
  id: Id;
  host: Id;
  hostName: string | null;
  name: string | null;
  username: string;
  credential: CredentialLabel | null;
  credentialId: Id | null;
  route: RouteLabel;
  routeId: Id | null;
  terminalProfile: string | null;
  startupCommand: string | null;
  agentForwarding: boolean;
  isDefault: boolean;
  restored: boolean;
}

export interface AccessProfileInput {
  host: Id;
  name: string | null;
  username: string;
  credential: Id | null;
  route: Id | null;
  terminalProfile: string | null;
  startupCommand: string | null;
  agentForwarding: boolean;
  defaultProfile: boolean;
}

export type ObjectEditError =
  | { kind: "invalidId"; field: string }
  | { kind: "notFound"; entity: string }
  | { kind: "emptyName" }
  | { kind: "emptyUsername" }
  | { kind: "textTooLong"; field: string }
  | { kind: "invalidCredential" }
  | { kind: "invalidRoute" }
  | { kind: "invalidProxy" }
  | { kind: "invalidCommand" }
  | { kind: "invalidCredentialBinding" }
  | { kind: "secretRequired" }
  | { kind: "secretNotAllowed" }
  | { kind: "inUse" }
  | { kind: "defaultProfile" }
  | { kind: "managedRoute" }
  | { kind: "failed"; reference: string };

export interface EditHost {
  name: string;
  address: string;
  port: number;
  notes: string | null;
  tags: Record<string, string>;
  groups: Id[];
}

export type EditHostError =
  | { kind: "emptyName" }
  | { kind: "invalidAddress" }
  | { kind: "invalidPort" }
  | { kind: "notesTooLong" }
  | { kind: "invalidTag" }
  | { kind: "groupNotFound" }
  | { kind: "failed"; reference: string };

export interface GroupInput {
  name: string;
  parent: Id | null;
}

export type GroupError =
  { kind: "emptyName" } | { kind: "parentNotFound" } | { kind: "selfParent" } | { kind: "failed"; reference: string };

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
  | { kind: "keyTooLarge" }
  | { kind: "keyReuseLimit" }
  | { kind: "notAPrivateKey" }
  | { kind: "publicKey" }
  | { kind: "puttyKey" }
  | { kind: "failed"; reference: string };

export interface OpenSshImportEntry {
  alias: string;
  address: string;
  port: number;
  username: string;
  identityFile: string | null;
}

export interface OpenSshUnsupportedDirective {
  line: number;
  directive: string;
}

export type OpenSshSkipReason =
  | { kind: "unsupportedDirective"; directive: string }
  | { kind: "globalRules"; directive: string }
  | { kind: "duplicateAlias" }
  | { kind: "invalidValue"; directive: string };

export interface OpenSshSkippedHost {
  alias: string;
  line: number;
  reason: OpenSshSkipReason;
}

export interface OpenSshImportPreview {
  path: string;
  entries: OpenSshImportEntry[];
  skippedHosts: OpenSshSkippedHost[];
  unsupported: OpenSshUnsupportedDirective[];
}

export interface OpenSshImportResult {
  hosts: HostDetails[];
  skippedHosts: OpenSshSkippedHost[];
  unsupported: OpenSshUnsupportedDirective[];
}

export type OpenSshImportError =
  | { kind: "fileNotFound" }
  | { kind: "notText" }
  | { kind: "tooLarge" }
  | { kind: "sourceChanged" }
  | { kind: "noHosts" }
  | { kind: "inventoryNotEmpty" }
  | { kind: "identityNotFound"; path: string }
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
  /** The newest live device-local sessions, newest first. */
  recentSessions(): Promise<Outcome<RecentSession[], Failure>>;
  host(id: Id): Promise<Outcome<HostDetails | null, Failure>>;
  addHost(host: NewHost): Promise<Outcome<HostDetails, AddHostError>>;
  updateHost(id: Id, host: EditHost): Promise<Outcome<HostDetails, EditHostError>>;
  createGroup(group: GroupInput): Promise<Outcome<GroupSummary, GroupError>>;
  updateGroup(id: Id, group: GroupInput): Promise<Outcome<GroupSummary, GroupError>>;
  accessProfiles(): Promise<Outcome<AccessProfileSummary[], ObjectEditError>>;
  createAccessProfile(profile: AccessProfileInput): Promise<Outcome<AccessProfileSummary, ObjectEditError>>;
  updateAccessProfile(id: Id, profile: AccessProfileInput): Promise<Outcome<AccessProfileSummary, ObjectEditError>>;
  deleteAccessProfile(id: Id): Promise<Outcome<null, ObjectEditError>>;
  credentials(): Promise<Outcome<CredentialSummary[], ObjectEditError>>;
  createCredential(credential: CredentialInput): Promise<Outcome<CredentialSummary, ObjectEditError>>;
  updateCredential(id: Id, credential: CredentialInput): Promise<Outcome<CredentialSummary, ObjectEditError>>;
  deleteCredential(id: Id): Promise<Outcome<null, ObjectEditError>>;
  routes(): Promise<Outcome<RouteSummary[], ObjectEditError>>;
  createRoute(route: RouteInput): Promise<Outcome<RouteSummary, ObjectEditError>>;
  updateRoute(id: Id, route: RouteInput): Promise<Outcome<RouteSummary, ObjectEditError>>;
  deleteRoute(id: Id): Promise<Outcome<null, ObjectEditError>>;
  setFavorite(id: Id, favorite: boolean): Promise<Outcome<null, Failure>>;
  /** Deletes the host and its logins, and keys only it used that were never named. */
  deleteHost(id: Id): Promise<Outcome<Deletion, Failure>>;
  undoDelete(token: string): Promise<Outcome<null, Failure>>;
  /** Asks the system for a private key file; null when the user cancels. */
  chooseKeyFile(): Promise<string | null>;
  chooseOpenSshConfig(): Promise<string | null>;
  previewOpenSshConfig(path: string): Promise<Outcome<OpenSshImportPreview, OpenSshImportError>>;
  importOpenSshConfig(preview: OpenSshImportPreview): Promise<Outcome<OpenSshImportResult, OpenSshImportError>>;
  /** Calls `listener` after any change to the inventory; returns a function that stops it. */
  onChange(listener: () => void): () => void;
}

export const InventoryContext = createContext<InventoryApi | null>(null);

export function useInventory(): InventoryApi {
  const api = useContext(InventoryContext);
  if (!api) throw new Error("useInventory must be used inside InventoryContext");
  return api;
}
