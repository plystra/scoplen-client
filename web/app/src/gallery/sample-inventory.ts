// SPDX-License-Identifier: Apache-2.0
//
// Sample data for the development gallery only. It follows the inventory
// contract (../inventory/api.ts) closely enough to exercise every screen
// state; it is not part of the application and never ships.

import type {
  Areas,
  CredentialLabel,
  EditHost,
  GroupSummary,
  GroupInput,
  HostDetails,
  HostSource,
  Id,
  InventoryApi,
  LoginSummary,
  NewHost,
  Outcome,
  RecentSession,
} from "../inventory/api";

const groups: GroupSummary[] = [
  { id: "g-prod", name: "Production", parent: null, hostCount: 4 },
  { id: "g-prod-db", name: "Databases", parent: "g-prod", hostCount: 1 },
  { id: "g-staging", name: "Staging", parent: null, hostCount: 2 },
  { id: "g-home", name: "Home lab", parent: null, hostCount: 1 },
];

const laptopKey: CredentialLabel = {
  id: "c-laptop",
  kind: "key",
  name: "laptop",
  fingerprint: "SHA256:q3Vw8mJ0yR1cZp7nT5kLx9aD2fGh4sU6eB0iO3wQ1aM",
  comment: "mia@laptop",
  publicKey: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIK7m3x0fQ8yT2w1cZp7nT5kLx9aD2fGh4sU6eB0iO3wQ",
};
const deployKey: CredentialLabel = {
  id: "c-deploy",
  kind: "key",
  name: null,
  fingerprint: "SHA256:Zt1o9Lq0bN3xV7cW2eR5yU8iP4aS6dF1gH0jK3lM9nB",
  comment: "deploy@ci",
  publicKey: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIGq9Lq0bN3xV7cW2eR5yU8iP4aS6dF1gH0jK3lM9nBz",
};
const agent: CredentialLabel = {
  id: "c-agent",
  kind: "agent",
  name: null,
  fingerprint: null,
  comment: null,
  publicKey: null,
};
const nasPassword: CredentialLabel = {
  id: "c-nas",
  kind: "password",
  name: null,
  fingerprint: null,
  comment: null,
  publicKey: null,
};

function login(
  id: Id,
  username: string,
  credential: CredentialLabel | null,
  route: LoginSummary["route"],
  isDefault = true,
  name: string | null = null,
): LoginSummary {
  return { id, name, username, credential, route, isDefault };
}

function host(
  partial: Omit<HostDetails, "username" | "loginCount" | "route" | "restored"> & { restored?: boolean },
): HostDetails {
  const primary = partial.logins.find((l) => l.isDefault) ?? partial.logins[0];
  return {
    restored: false,
    ...partial,
    username: primary?.username ?? null,
    loginCount: partial.logins.length,
    route: primary?.route ?? { kind: "direct" },
  };
}

const jump = { kind: "jump", name: "prod-jump" } as const;

const seededRecentSessions: RecentSession[] = [
  {
    id: "s-api-1",
    hostId: "h-api-1",
    hostName: "prod-api-01",
    address: "10.0.1.24",
    port: 22,
    username: "deploy",
    kind: "terminal",
    startedAt: String(Date.UTC(2026, 9, 7, 9, 15)),
    endedAt: String(Date.UTC(2026, 9, 7, 10, 2)),
    outcome: "closed",
  },
  {
    id: "s-db-1",
    hostId: "h-db-1",
    hostName: "prod-db-01",
    address: "10.0.4.12",
    port: 22,
    username: "dba",
    kind: "files",
    startedAt: String(Date.UTC(2026, 9, 6, 16, 40)),
    endedAt: null,
    outcome: null,
  },
];

export function sampleHosts(): HostDetails[] {
  return [
    host({
      id: "h-api-1",
      name: "prod-api-01",
      address: "10.0.1.24",
      port: 22,
      favorite: true,
      tags: { env: "production", region: "nrt" },
      groups: ["g-prod"],
      notes: "API node behind the public load balancer. Deploys go through CI.",
      logins: [
        login("l-1", "deploy", laptopKey, { kind: "direct" }),
        login("l-1b", "root", agent, { kind: "direct" }, false, "Break glass"),
      ],
    }),
    host({
      id: "h-api-2",
      name: "prod-api-02",
      address: "10.0.1.25",
      port: 22,
      favorite: false,
      tags: { env: "production", region: "nrt" },
      groups: ["g-prod"],
      notes: null,
      logins: [login("l-2", "deploy", laptopKey, { kind: "direct" })],
    }),
    host({
      id: "h-db-1",
      name: "prod-db-01",
      address: "10.0.4.12",
      port: 22,
      favorite: true,
      tags: { env: "production", role: "postgres" },
      groups: ["g-prod", "g-prod-db"],
      notes: null,
      logins: [login("l-3", "dba", laptopKey, jump)],
    }),
    host({
      id: "h-jump",
      name: "prod-jump",
      address: "jump.example.com",
      port: 2222,
      favorite: false,
      tags: { env: "production" },
      groups: ["g-prod"],
      notes: null,
      logins: [login("l-4", "mia", laptopKey, { kind: "direct" })],
    }),
    host({
      id: "h-stg-web",
      name: "staging-web",
      address: "staging.example.com",
      port: 22,
      favorite: false,
      tags: { env: "staging" },
      groups: ["g-staging"],
      notes: null,
      logins: [login("l-5", "deploy", deployKey, { kind: "direct" })],
    }),
    host({
      id: "h-stg-worker",
      name: "staging-worker-03",
      address: "10.20.3.7",
      port: 22,
      favorite: false,
      tags: { env: "staging", role: "queue" },
      groups: ["g-staging"],
      notes: null,
      logins: [login("l-6", "deploy", deployKey, { kind: "bastion", name: "staging-net" })],
      restored: true,
    }),
    host({
      id: "h-nas",
      name: "nas",
      address: "192.168.1.20",
      port: 22,
      favorite: false,
      tags: {},
      groups: ["g-home"],
      notes: "Synology. Admin interface on port 5001.",
      logins: [login("l-7", "admin", nasPassword, { kind: "direct" })],
    }),
  ];
}

const ok = <T>(data: T): Outcome<T, never> => ({ status: "ok", data });
const wait = <T>(value: T) => new Promise<T>((resolve) => setTimeout(() => resolve(value), 120));
const MAX_TEXT_BYTES = 16 * 1024;

function textBytes(value: string): number {
  return new TextEncoder().encode(value).byteLength;
}

/** An in-memory inventory for the gallery. */
export function sampleInventory(
  initial: HostDetails[],
  options: { failHosts?: boolean; failRecentSessions?: boolean } = {},
): InventoryApi {
  let hosts = initial.map((h) => ({ ...h }));
  let availableGroups = groups.map((group) => ({ ...group }));
  const sessions = seededRecentSessions.map((session) => ({ ...session }));
  const deleted = new Map<string, HostDetails>();
  const listeners = new Set<() => void>();
  const changed = () => listeners.forEach((l) => l());
  const groupSummaries = () =>
    availableGroups.map((group) => ({
      ...group,
      hostCount: hosts.filter((host) => host.groups.includes(group.id)).length,
    }));
  const recentSessionRows = () =>
    sessions
      .filter((session) => hosts.some((host) => host.id === session.hostId))
      .map((session) => {
        const current = hosts.find((host) => host.id === session.hostId);
        return current
          ? {
              ...session,
              hostName: current.name,
              address: current.address,
              port: current.port,
              username: current.username ?? session.username,
            }
          : session;
      });

  const matches = (h: HostDetails, query: string) => {
    if (!query) return true;
    const q = query.toLowerCase();
    return [h.name, h.address, h.username ?? "", ...Object.entries(h.tags).map(([k, v]) => `${k}:${v}`)].some((field) =>
      field.toLowerCase().includes(q),
    );
  };
  const inSource = (h: HostDetails, source: HostSource) =>
    source.kind === "all" ||
    (source.kind === "favorites" && h.favorite) ||
    (source.kind === "recent" && recentSessionRows().some((session) => session.hostId === h.id)) ||
    (source.kind === "group" && h.groups.includes(source.id));

  return {
    areas: () =>
      wait(
        ok<Areas>({
          favorites: hosts.some((h) => h.favorite),
          recent: recentSessionRows().length > 0,
          groups: hosts.length > 0,
          keys: true,
          routes: true,
        }),
      ),
    groups: () => wait(ok(hosts.length > 0 ? groupSummaries() : [])),
    hosts: (source, query) =>
      options.failHosts
        ? wait({
            status: "error" as const,
            error: { kind: "failed" as const, reference: "the local store is busy: database is locked" },
          })
        : wait(
            ok(
              hosts
                .filter((h) => inSource(h, source) && matches(h, query))
                .sort((a, b) => Number(b.favorite) - Number(a.favorite) || a.name.localeCompare(b.name)),
            ),
          ),
    recentSessions: () =>
      options.failRecentSessions
        ? wait({
            status: "error" as const,
            error: { kind: "failed" as const, reference: "the local session history is unavailable" },
          })
        : wait(ok(recentSessionRows())),
    host: (id) => wait(ok(hosts.find((h) => h.id === id) ?? null)),
    addHost: (input: NewHost) => {
      if (!/^[a-z0-9.:\-[\]]+$/i.test(input.address))
        return wait({ status: "error" as const, error: { kind: "invalidAddress" as const } });
      if (input.signIn.kind === "keyText" && !input.signIn.key.includes("PRIVATE KEY")) {
        return wait({
          status: "error" as const,
          error: { kind: input.signIn.key.startsWith("ssh-") ? ("publicKey" as const) : ("notAPrivateKey" as const) },
        });
      }
      const credential: CredentialLabel =
        input.signIn.kind === "password"
          ? { id: `c-${Date.now()}`, kind: "password", name: null, fingerprint: null, comment: null, publicKey: null }
          : input.signIn.kind === "agent"
            ? agent
            : { ...deployKey, id: `c-${Date.now()}`, name: null, comment: `${input.username}@scoplen` };
      const created = host({
        id: `h-${Date.now()}`,
        name: input.name ?? input.address,
        address: input.address,
        port: input.port ?? 22,
        favorite: false,
        tags: {},
        groups: [],
        notes: null,
        logins: [login(`l-${Date.now()}`, input.username, credential, { kind: "direct" })],
      });
      hosts = [...hosts, created];
      changed();
      return wait(ok(created));
    },
    updateHost: (id: string, input: EditHost) => {
      const current = hosts.find((h) => h.id === id);
      if (!current)
        return wait({ status: "error" as const, error: { kind: "failed" as const, reference: "host not found" } });
      if (!input.name.trim()) return wait({ status: "error" as const, error: { kind: "emptyName" as const } });
      if (!input.address.trim() || /\s/.test(input.address)) {
        return wait({ status: "error" as const, error: { kind: "invalidAddress" as const } });
      }
      if (!Number.isInteger(input.port) || input.port < 1 || input.port > 65535) {
        return wait({ status: "error" as const, error: { kind: "invalidPort" as const } });
      }
      if (input.notes && textBytes(input.notes) > MAX_TEXT_BYTES) {
        return wait({ status: "error" as const, error: { kind: "notesTooLong" as const } });
      }
      if (
        Object.entries(input.tags).some(
          ([key, value]) => !key.trim() || textBytes(key) > MAX_TEXT_BYTES || textBytes(value) > MAX_TEXT_BYTES,
        )
      ) {
        return wait({ status: "error" as const, error: { kind: "invalidTag" as const } });
      }
      const requestedGroups = [...new Set(input.groups)];
      if (requestedGroups.some((groupId) => !availableGroups.some((group) => group.id === groupId))) {
        return wait({ status: "error" as const, error: { kind: "groupNotFound" as const } });
      }
      const updated = {
        ...current,
        ...input,
        notes: input.notes?.trim() || null,
        groups: requestedGroups,
        tags: { ...input.tags },
      };
      hosts = hosts.map((host) => (host.id === id ? updated : host));
      changed();
      return wait(ok(updated));
    },
    createGroup: (input: GroupInput) => {
      if (!input.name.trim()) return wait({ status: "error" as const, error: { kind: "emptyName" as const } });
      if (input.parent && !availableGroups.some((group) => group.id === input.parent)) {
        return wait({ status: "error" as const, error: { kind: "parentNotFound" as const } });
      }
      const created = { id: `g-${Date.now()}`, name: input.name.trim(), parent: input.parent, hostCount: 0 };
      availableGroups = [...availableGroups, created];
      changed();
      return wait(ok(created));
    },
    updateGroup: (id: string, input: GroupInput) => {
      const current = availableGroups.find((group) => group.id === id);
      if (!current)
        return wait({ status: "error" as const, error: { kind: "failed" as const, reference: "group not found" } });
      if (!input.name.trim()) return wait({ status: "error" as const, error: { kind: "emptyName" as const } });
      if (input.parent === id) return wait({ status: "error" as const, error: { kind: "selfParent" as const } });
      if (input.parent && !availableGroups.some((group) => group.id === input.parent)) {
        return wait({ status: "error" as const, error: { kind: "parentNotFound" as const } });
      }
      const updated = {
        ...current,
        name: input.name.trim(),
        parent: input.parent,
        hostCount: hosts.filter((host) => host.groups.includes(id)).length,
      };
      availableGroups = availableGroups.map((group) => (group.id === id ? updated : group));
      changed();
      return wait(ok(updated));
    },
    setFavorite: (id, favorite) => {
      hosts = hosts.map((h) => (h.id === id ? { ...h, favorite } : h));
      changed();
      return wait(ok(null));
    },
    deleteHost: (id) => {
      const found = hosts.find((h) => h.id === id);
      hosts = hosts.filter((h) => h.id !== id);
      if (found) deleted.set(`undo-${id}`, found);
      changed();
      return wait(ok({ token: `undo-${id}` }));
    },
    undoDelete: (token) => {
      const found = deleted.get(token);
      if (found) hosts = [...hosts, found];
      deleted.delete(token);
      changed();
      return wait(ok(null));
    },
    chooseKeyFile: () => wait("/Users/mia/.ssh/id_ed25519"),
    chooseOpenSshConfig: () => wait("/Users/mia/.ssh/config"),
    previewOpenSshConfig: (path) =>
      wait(
        ok({
          path,
          entries: [
            { alias: "prod-api", address: "prod.example.com", port: 22, username: "deploy", identityFile: null },
          ],
          skippedHosts: [
            { alias: "legacy", line: 5, reason: { kind: "unsupportedDirective", directive: "ForwardAgent" } },
          ],
          unsupported: [{ line: 7, directive: "ForwardAgent" }],
        }),
      ),
    importOpenSshConfig: (preview) => {
      if (hosts.length > 0) return wait({ status: "error" as const, error: { kind: "inventoryNotEmpty" as const } });
      if (preview.entries.length === 0) return wait({ status: "error" as const, error: { kind: "noHosts" as const } });
      const imported = host({
        id: `h-${Date.now()}`,
        name: "prod-api",
        address: "prod.example.com",
        port: 22,
        favorite: false,
        tags: {},
        groups: [],
        notes: null,
        logins: [login(`l-${Date.now()}`, "deploy", agent, { kind: "direct" })],
      });
      hosts = [imported];
      changed();
      return wait(
        ok({
          hosts: [imported],
          skippedHosts: preview.skippedHosts,
          unsupported: preview.unsupported,
        }),
      );
    },
    onChange: (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
