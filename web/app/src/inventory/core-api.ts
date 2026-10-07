// SPDX-License-Identifier: Apache-2.0
//
// The production inventory adapter. It is deliberately a thin translation
// layer: the Rust core owns validation, persistence, secrets, and atomicity;
// the interface only receives the redacted DTOs defined by inventory/api.ts.

import {
  commands,
  events,
  type AddHostError as IpcAddHostError,
  type Areas as IpcAreas,
  type EditHost as IpcEditHost,
  type EditHostError as IpcEditHostError,
  type Failure as IpcFailure,
  type GroupError as IpcGroupError,
  type GroupInput as IpcGroupInput,
  type GroupSummary as IpcGroupSummary,
  type HostDetails as IpcHostDetails,
  type HostSource as IpcHostSource,
  type HostSummary as IpcHostSummary,
  type NewHost as IpcNewHost,
  type OpenSshImportError as IpcOpenSshImportError,
  type OpenSshImportPreview as IpcOpenSshImportPreview,
  type OpenSshImportResult as IpcOpenSshImportResult,
} from "../ipc/bindings";
import type {
  AddHostError,
  Deletion,
  EditHost,
  EditHostError,
  Failure,
  GroupError,
  GroupInput,
  GroupSummary,
  HostDetails,
  HostSource,
  InventoryApi,
  NewHost,
  OpenSshImportError,
  OpenSshImportPreview,
  OpenSshImportResult,
  Outcome,
} from "./api";

type Result<T, E> = { status: "ok"; data: T } | { status: "error"; error: E };

const inventoryRecordTypes = new Set(["host", "accessProfile", "credential", "route", "hostGroup"]);

function failure(error: IpcFailure | unknown): Failure {
  if (typeof error === "object" && error !== null && "kind" in error && error.kind === "failed") {
    const reference =
      "reference" in error && typeof error.reference === "string" ? error.reference : "inventory operation failed";
    return { kind: "failed", reference };
  }
  return { kind: "failed", reference: String(error) };
}

function outcome<T, E>(result: Result<T, E>): Outcome<T, E> {
  return result.status === "ok" ? result : { status: "error", error: result.error };
}

function inventoryOutcome<T>(result: Result<T, IpcFailure>): Outcome<T, Failure> {
  return result.status === "ok" ? result : { status: "error", error: failure(result.error) };
}

/** Creates the desktop adapter used by the real Hosts screen. */
export function createInventoryApi(): InventoryApi {
  return {
    areas: async () => inventoryOutcome((await commands.inventoryAreas()) as Result<IpcAreas, IpcFailure>),
    groups: async () => inventoryOutcome((await commands.inventoryGroups()) as Result<IpcGroupSummary[], IpcFailure>),
    hosts: async (source: HostSource, query: string) =>
      inventoryOutcome(
        (await commands.inventoryHosts(source as IpcHostSource, query)) as Result<IpcHostSummary[], IpcFailure>,
      ),
    host: async (id: string) =>
      inventoryOutcome((await commands.inventoryHost(id)) as Result<IpcHostDetails | null, IpcFailure>),
    addHost: async (input: NewHost) =>
      outcome(
        (await commands.inventoryAddHost(input as IpcNewHost)) as Result<HostDetails, IpcAddHostError>,
      ) as Outcome<HostDetails, AddHostError>,
    updateHost: async (id: string, input: EditHost) =>
      outcome(
        (await commands.inventoryUpdateHost(id, input as IpcEditHost)) as Result<HostDetails, IpcEditHostError>,
      ) as Outcome<HostDetails, EditHostError>,
    createGroup: async (input: GroupInput) =>
      outcome(
        (await commands.inventoryCreateGroup(input as IpcGroupInput)) as Result<GroupSummary, IpcGroupError>,
      ) as Outcome<GroupSummary, GroupError>,
    updateGroup: async (id: string, input: GroupInput) =>
      outcome(
        (await commands.inventoryUpdateGroup(id, input as IpcGroupInput)) as Result<GroupSummary, IpcGroupError>,
      ) as Outcome<GroupSummary, GroupError>,
    setFavorite: async (id: string, favorite: boolean) =>
      inventoryOutcome((await commands.inventorySetFavorite(id, favorite)) as Result<null, IpcFailure>),
    deleteHost: async (id: string) =>
      inventoryOutcome((await commands.inventoryDeleteHost(id)) as Result<Deletion, IpcFailure>),
    undoDelete: async (token: string) =>
      inventoryOutcome((await commands.inventoryUndoDelete(token)) as Result<null, IpcFailure>),
    chooseKeyFile: async () => {
      const result = await commands.chooseKeyFile();
      if (result.status === "ok") return result.data;
      throw new Error(failure(result.error).reference);
    },
    chooseOpenSshConfig: async () => {
      const result = await commands.chooseOpenSshConfig();
      if (result.status === "ok") return result.data;
      throw new Error(failure(result.error).reference);
    },
    previewOpenSshConfig: async (path: string) =>
      outcome(
        (await commands.inventoryPreviewOpenSshConfig(path)) as Result<IpcOpenSshImportPreview, IpcOpenSshImportError>,
      ) as Outcome<OpenSshImportPreview, OpenSshImportError>,
    importOpenSshConfig: async (preview: OpenSshImportPreview) =>
      outcome(
        (await commands.inventoryImportOpenSshConfig(preview as IpcOpenSshImportPreview)) as Result<
          IpcOpenSshImportResult,
          IpcOpenSshImportError
        >,
      ) as Outcome<OpenSshImportResult, OpenSshImportError>,
    onChange: (listener) => {
      let stopped = false;
      let unlisten: (() => void | Promise<void>) | undefined;
      const dispose = (callback: () => void | Promise<void>) => {
        void Promise.resolve(callback()).catch(() => undefined);
      };
      void events.storeChanged
        .listen((event) => {
          if (inventoryRecordTypes.has(event.payload.recordType)) listener();
        })
        .then((unlistenFn) => {
          if (stopped) dispose(unlistenFn);
          else unlisten = unlistenFn;
        });
      return () => {
        stopped = true;
        const callback = unlisten;
        unlisten = undefined;
        if (callback) dispose(callback);
      };
    },
  };
}
