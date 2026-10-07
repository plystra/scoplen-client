// SPDX-License-Identifier: Apache-2.0
import { Button, Confirm, Dialog, IconButton, Notice, ScopeMark, SearchField, Tag } from "@scoplen/ui";
import { Plus, RotateCcw, Star } from "lucide-react";
import { useRef, useState, type KeyboardEvent } from "react";
import { useI18n } from "../i18n";
import { AddHostDialog } from "./add-host";
import {
  useInventory,
  type GroupSummary,
  type HostSource,
  type HostSummary,
  type Id,
  type OpenSshImportError,
  type OpenSshImportPreview,
  type OpenSshSkipReason,
  type RecentSession,
} from "./api";
import { HostDetailsPanel } from "./host-details";
import { routeText } from "./labels";
import type { ObjectSection } from "./objects";
import { useLoad, type Loaded } from "./use-load";

type Toast = { message: string; undo?: string } | null;

/** The Hosts tab: where the inventory is, and the way into every session. */
export function HostsHome({ onOpenObjects }: { onOpenObjects?: (section: ObjectSection) => void } = {}) {
  const { t } = useI18n();
  const api = useInventory();
  const [source, setSource] = useState<HostSource>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Id | null>(null);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<HostSummary | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [toast, setToast] = useState<Toast>(null);
  const [importPreview, setImportPreview] = useState<OpenSshImportPreview | null>(null);
  const [importBusy, setImportBusy] = useState(false);
  const [importError, setImportError] = useState<string | null>(null);
  const [importStale, setImportStale] = useState(false);
  const search = useRef<HTMLInputElement>(null);

  const sourceKey = source.kind === "group" ? `group:${source.id}` : source.kind;
  const areas = useLoad(() => api.areas(), "areas");
  const groups = useLoad(() => api.groups(), "groups");
  const hosts = useLoad(() => api.hosts(source, query.trim()), `${sourceKey}|${query.trim()}`);
  const recentSessions = useLoad(() => api.recentSessions(), "recent-sessions");

  const groupList = groups.state === "ready" ? groups.data : [];
  const title =
    source.kind === "all"
      ? t("hosts.title")
      : source.kind === "favorites"
        ? t("hosts.favorites")
        : source.kind === "recent"
          ? t("hosts.recent")
          : (groupList.find((g) => g.id === source.id)?.name ?? t("hosts.groups"));

  const confirmDelete = async () => {
    if (!deleting) return;
    setDeleteBusy(true);
    const outcome = await api.deleteHost(deleting.id);
    setDeleteBusy(false);
    setDeleting(null);
    if (outcome.status === "ok") {
      if (selected === deleting.id) setSelected(null);
      setToast({ message: t("delete.done", { name: deleting.name }), undo: outcome.data.token });
    } else {
      setToast({
        message: `${t("failed.body")} ${t("startup.error.reference", { reference: outcome.error.reference })}`,
      });
    }
  };

  const reconnect = async (sessionId: Id) => {
    const outcome = await api.reconnectSession(sessionId);
    if (outcome.status === "error") {
      const message =
        outcome.error.kind === "transportUnavailable"
          ? t("hosts.recentSessions.reconnectUnavailable")
          : outcome.error.kind === "failed"
            ? `${t("failed.body")} ${t("startup.error.reference", { reference: outcome.error.reference })}`
            : t("hosts.recentSessions.reconnectFailed");
      setToast({ message });
    }
  };

  const onListKeyDown = (event: KeyboardEvent) => {
    // "/" focuses the search, as in many lists, unless the user is typing.
    if (event.key === "/" && !(event.target instanceof HTMLInputElement)) {
      event.preventDefault();
      search.current?.focus();
    }
  };

  const chooseOpenSshConfig = async () => {
    if (importBusy) return;
    setImportBusy(true);
    try {
      const path = await api.chooseOpenSshConfig();
      if (!path) return;
      const preview = await api.previewOpenSshConfig(path);
      if (preview.status === "ok") {
        setImportError(null);
        setImportStale(false);
        setImportPreview(preview.data);
      } else {
        setToast({ message: `${t("import.previewError")} ${openSshImportErrorText(t, preview.error)}` });
      }
    } catch {
      setToast({ message: t("import.error.picker") });
    } finally {
      setImportBusy(false);
    }
  };

  const confirmImport = async () => {
    if (!importPreview || importPreview.entries.length === 0 || importBusy || importStale) return;
    setImportBusy(true);
    setImportError(null);
    try {
      const result = await api.importOpenSshConfig(importPreview);
      if (result.status === "ok") {
        setImportPreview(null);
        setToast({
          message:
            result.data.skippedHosts.length > 0
              ? t("import.doneWithSkipped", {
                  count: result.data.hosts.length,
                  skipped: result.data.skippedHosts.length,
                })
              : t("import.done", { count: result.data.hosts.length }),
        });
        setSelected(result.data.hosts[0]?.id ?? null);
      } else {
        if (result.error.kind === "sourceChanged") setImportStale(true);
        setImportError(`${t("import.error")} ${openSshImportErrorText(t, result.error)}`);
      }
    } catch {
      setImportError(t("import.error.unexpected"));
    } finally {
      setImportBusy(false);
    }
  };

  const closeImportPreview = () => {
    if (importBusy) return;
    setImportPreview(null);
    setImportError(null);
    setImportStale(false);
  };

  return (
    <div className="flex h-full min-h-0" onKeyDown={onListKeyDown}>
      <Sidebar
        source={source}
        onSource={(next) => {
          setSource(next);
          setSelected(null);
        }}
        favorites={areas.state === "ready" && areas.data.favorites}
        recent={areas.state === "ready" && areas.data.recent}
        groups={areas.state === "ready" && areas.data.groups ? groupList : []}
        keys={areas.state === "ready" && areas.data.keys}
        routes={areas.state === "ready" && areas.data.routes}
        onOpenObjects={onOpenObjects}
      />

      <section aria-labelledby="hosts-title" className="@container flex min-w-0 flex-1 flex-col">
        <header className="flex flex-wrap items-center gap-x-4 gap-y-3 px-6 pt-5 pb-4">
          <div className="flex min-w-32 flex-1 items-baseline gap-3">
            <h1 id="hosts-title" className="truncate font-serif text-xl font-medium">
              {title}
            </h1>
            {hosts.state === "ready" ? (
              <span className="shrink-0 text-sm text-muted-foreground">
                {t("hosts.count", { count: hosts.data.length })}
              </span>
            ) : null}
          </div>
          <SearchField
            ref={search}
            label={t("hosts.search")}
            placeholder={t("hosts.search.placeholder")}
            clearLabel={t("hosts.search.clear")}
            value={query}
            onChange={setQuery}
            className="min-w-40 flex-1 basis-40 @2xl:max-w-64"
          />
          <Button variant="primary" onClick={() => setAdding(true)} className="gap-1.5 pl-3">
            <Plus aria-hidden="true" className="size-4" />
            {t("hosts.add")}
          </Button>
        </header>

        <div className="min-h-0 flex-1 overflow-y-auto px-3 pb-6">
          {source.kind === "recent" ? (
            <RecentSessionSection sessions={recentSessions} onSelect={setSelected} onReconnect={reconnect} />
          ) : null}
          {hosts.state === "loading" ? (
            <p className="sr-only" role="status">
              {t("hosts.loading")}
            </p>
          ) : null}
          {hosts.state === "failed" ? (
            <p role="alert" className="px-3 py-6 text-sm text-attention">
              {t("failed.body")} {t("startup.error.reference", { reference: hosts.reference })}
            </p>
          ) : null}
          {hosts.state === "ready" ? (
            <HostList
              hosts={hosts.data}
              selected={selected}
              onSelect={setSelected}
              onFavorite={(host) => void api.setFavorite(host.id, !host.favorite)}
              empty={
                query.trim() !== "" ? (
                  <Empty>
                    <p>{t("hosts.noMatch", { query: query.trim() })}</p>
                    <Button variant="ghost" className="mt-3" onClick={() => setQuery("")}>
                      {t("hosts.noMatch.clear")}
                    </Button>
                  </Empty>
                ) : source.kind === "all" ? (
                  <Empty>
                    <h2 className="font-serif text-lg font-medium text-foreground">{t("hosts.empty.title")}</h2>
                    <p className="mt-2 max-w-sm">{t("hosts.empty.body")}</p>
                    <div className="mt-5 flex flex-wrap justify-center gap-2">
                      <Button variant="primary" onClick={() => setAdding(true)}>
                        {t("hosts.add")}
                      </Button>
                      <Button variant="ghost" busy={importBusy} onClick={() => void chooseOpenSshConfig()}>
                        {t("hosts.importOpenSsh")}
                      </Button>
                    </div>
                  </Empty>
                ) : (
                  <Empty>
                    <p>
                      {source.kind === "favorites"
                        ? t("hosts.empty.favorites")
                        : source.kind === "recent"
                          ? t("hosts.empty.recent")
                          : t("hosts.empty.group")}
                    </p>
                  </Empty>
                )
              }
            />
          ) : null}
        </div>
      </section>

      {selected ? (
        <HostDetailsPanel
          id={selected}
          groups={groupList}
          onClose={() => setSelected(null)}
          onDelete={(host) => setDeleting(host)}
        />
      ) : null}

      <AddHostDialog
        open={adding}
        onOpenChange={setAdding}
        onAdded={(host) => {
          setSelected(host.id);
          setToast({ message: t("add.done", { name: host.name }) });
        }}
      />

      <Confirm
        open={deleting !== null}
        onOpenChange={(open) => !open && setDeleting(null)}
        title={t("delete.title", { name: deleting?.name ?? "" })}
        body={<p className="m-0">{t("delete.body")}</p>}
        confirmLabel={t("delete.confirm")}
        cancelLabel={t("delete.cancel")}
        onConfirm={() => void confirmDelete()}
        busy={deleteBusy}
        destructive
      />

      <Dialog
        open={importPreview !== null}
        onOpenChange={(open) => !open && closeImportPreview()}
        title={t("import.title")}
        closeLabel={t("import.close")}
        footer={
          <>
            <Button variant="ghost" autoFocus disabled={importBusy} onClick={closeImportPreview}>
              {t("import.cancel")}
            </Button>
            <Button
              variant="primary"
              busy={importBusy}
              disabled={!importPreview || importPreview.entries.length === 0 || importStale}
              onClick={() => void confirmImport()}
            >
              {t("import.confirm")}
            </Button>
          </>
        }
      >
        {importPreview ? (
          <div className="space-y-4 text-sm">
            <p className="m-0 break-all text-muted-foreground">{t("import.file", { path: importPreview.path })}</p>
            <p className="m-0">{t("import.body", { count: importPreview.entries.length })}</p>
            {importPreview.entries.length === 0 ? <p className="m-0">{t("import.none")}</p> : null}
            <section aria-labelledby="import-ready-title">
              <h3 id="import-ready-title" className="m-0 font-medium">
                {t("import.ready", { count: importPreview.entries.length })}
              </h3>
              {importPreview.entries.length > 0 ? (
                <ul className="mt-2 mb-0 list-none space-y-2 p-0">
                  {importPreview.entries.map((entry) => (
                    <li
                      key={entry.alias}
                      className="flex flex-wrap justify-between gap-x-3 border-b border-border pb-2"
                    >
                      <span className="break-all font-medium">{entry.alias}</span>
                      <span className="break-all font-mono text-xs text-muted-foreground">
                        {entry.username}@{entry.address}
                        {entry.port === 22 ? "" : `:${entry.port}`}
                      </span>
                    </li>
                  ))}
                </ul>
              ) : null}
            </section>
            {importPreview.skippedHosts.length > 0 ? (
              <section aria-labelledby="import-skipped-title">
                <h3 id="import-skipped-title" className="m-0 font-medium">
                  {t("import.skipped", { count: importPreview.skippedHosts.length })}
                </h3>
                <ul className="mt-2 mb-0 list-none space-y-2 p-0">
                  {importPreview.skippedHosts.map((host, index) => (
                    <li key={`${host.line}:${host.alias}:${index}`} className="break-words">
                      <span className="font-medium">{host.alias}</span>
                      <span className="text-muted-foreground">
                        {" · "}
                        {t("import.line", { line: host.line })}
                        {" · "}
                        {openSshSkipReasonText(t, host.reason)}
                      </span>
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
            {importPreview.unsupported.length > 0 ? (
              <section aria-labelledby="import-unsupported-title">
                <h3 id="import-unsupported-title" className="m-0 font-medium">
                  {t("import.unsupported", { count: importPreview.unsupported.length })}
                </h3>
                <ul className="mt-2 mb-0 list-none space-y-1 p-0 text-muted-foreground">
                  {importPreview.unsupported.map((item, index) => (
                    <li key={`${item.line}:${item.directive}:${index}`} className="break-all">
                      {t("import.line", { line: item.line })}: {item.directive}
                    </li>
                  ))}
                </ul>
              </section>
            ) : null}
            {importError ? (
              <p role="alert" className="m-0 text-attention">
                {importError}
              </p>
            ) : null}
          </div>
        ) : null}
      </Dialog>

      {toast ? (
        <Notice
          message={toast.message}
          onDismiss={() => setToast(null)}
          action={
            toast.undo
              ? {
                  label: t("delete.undo"),
                  onAction: () => {
                    const token = toast.undo!;
                    setToast(null);
                    void api.undoDelete(token);
                  },
                }
              : undefined
          }
        />
      ) : null}
    </div>
  );
}

function openSshImportErrorText(t: ReturnType<typeof useI18n>["t"], error: OpenSshImportError): string {
  switch (error.kind) {
    case "fileNotFound":
      return t("import.error.fileNotFound");
    case "notText":
      return t("import.error.notText");
    case "tooLarge":
      return t("import.error.tooLarge");
    case "sourceChanged":
      return t("import.error.sourceChanged");
    case "noHosts":
      return t("import.error.noHosts");
    case "inventoryNotEmpty":
      return t("import.error.inventoryNotEmpty");
    case "identityNotFound":
      return t("import.error.identityNotFound", { path: error.path });
    default:
      return t("startup.error.reference", { reference: error.reference });
  }
}

function openSshSkipReasonText(t: ReturnType<typeof useI18n>["t"], reason: OpenSshSkipReason): string {
  switch (reason.kind) {
    case "unsupportedDirective":
      return t("import.skip.unsupportedDirective", { directive: reason.directive });
    case "globalRules":
      return t("import.skip.globalRules", { directive: reason.directive });
    case "duplicateAlias":
      return t("import.skip.duplicateAlias");
    case "invalidValue":
      return t("import.skip.invalidValue", { directive: reason.directive });
  }
}

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-20 text-center text-sm text-muted-foreground">{children}</div>
  );
}

function RecentSessionSection({
  sessions,
  onSelect,
  onReconnect,
}: {
  sessions: Loaded<RecentSession[]>;
  onSelect: (id: Id) => void;
  onReconnect: (id: Id) => void;
}) {
  const { t } = useI18n();
  if (sessions.state === "loading") {
    return (
      <p className="sr-only" role="status">
        {t("hosts.recentSessions.loading")}
      </p>
    );
  }
  if (sessions.state === "failed") {
    return (
      <p role="alert" className="px-3 py-3 text-sm text-attention">
        {t("failed.body")} {t("startup.error.reference", { reference: sessions.reference })}
      </p>
    );
  }
  return (
    <section aria-labelledby="recent-sessions-title" className="border-b border-border px-3 pt-1 pb-4">
      <h2 id="recent-sessions-title" className="px-3 pb-2 text-xs font-medium text-muted-foreground">
        {t("hosts.recentSessions")}
      </h2>
      {sessions.data.length === 0 ? (
        <p className="px-3 text-sm text-muted-foreground">{t("hosts.recentSessions.empty")}</p>
      ) : (
        <ul aria-label={t("hosts.recentSessions")} className="m-0 flex list-none flex-col p-0">
          {sessions.data.map((session) => (
            <RecentSessionRow key={session.id} session={session} onSelect={onSelect} onReconnect={onReconnect} />
          ))}
        </ul>
      )}
    </section>
  );
}

function RecentSessionRow({
  session,
  onSelect,
  onReconnect,
}: {
  session: RecentSession;
  onSelect: (id: Id) => void;
  onReconnect: (id: Id) => void;
}) {
  const { t } = useI18n();
  const status = session.outcome === null ? "open" : session.outcome;
  const date = new Date(Number(session.startedAt));
  const formatted = Number.isNaN(date.getTime()) ? "" : date.toLocaleString();
  return (
    <li className="group flex items-center rounded-md hover:bg-inset/60">
      <button
        type="button"
        onClick={() => onSelect(session.hostId)}
        className="flex min-w-0 flex-1 flex-col gap-0.5 rounded-md px-3 py-2 text-left focus-visible:outline-offset-0"
      >
        <span className="flex min-w-0 items-center justify-between gap-3">
          <span className="truncate text-sm font-medium text-foreground">{session.hostName}</span>
          <span className="shrink-0 text-xs text-muted-foreground">
            {t(`hosts.recentSessions.kind.${session.kind}`)}
          </span>
        </span>
        <span className="flex min-w-0 items-center justify-between gap-3 text-xs text-muted-foreground">
          <span className="truncate font-mono">
            {session.username ? `${session.username}@` : ""}
            {session.address}
            {session.port !== 22 ? `:${session.port}` : ""}
          </span>
          <span className={status === "failed" ? "shrink-0 text-attention" : "shrink-0"}>
            {t(`hosts.recentSessions.${status}`)}
          </span>
        </span>
        {formatted ? (
          <time dateTime={date.toISOString()} className="text-xs text-muted-foreground">
            {formatted}
          </time>
        ) : null}
      </button>
      {session.outcome !== null ? (
        <IconButton
          label={t("hosts.recentSessions.reconnect", { name: session.hostName })}
          onClick={() => onReconnect(session.id)}
          className="mr-1 opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100"
        >
          <RotateCcw aria-hidden="true" className="size-4" />
        </IconButton>
      ) : null}
    </li>
  );
}

function Sidebar({
  source,
  onSource,
  favorites,
  recent,
  groups,
  keys,
  routes,
  onOpenObjects,
}: {
  source: HostSource;
  onSource: (source: HostSource) => void;
  favorites: boolean;
  recent: boolean;
  groups: GroupSummary[];
  keys: boolean;
  routes: boolean;
  onOpenObjects?: (section: ObjectSection) => void;
}) {
  const { t } = useI18n();
  const is = (other: HostSource) =>
    other.kind === source.kind && (other.kind !== "group" || (source.kind === "group" && source.id === other.id));
  const item = (label: string, target: HostSource, depth = 0) => (
    <li key={target.kind === "group" ? target.id : target.kind}>
      <button
        type="button"
        aria-current={is(target) ? "page" : undefined}
        onClick={() => onSource(target)}
        style={{ paddingLeft: `${0.75 + depth * 0.875}rem` }}
        className="flex h-8 w-full items-center truncate rounded-md pr-3 text-left text-sm text-muted-foreground hover:bg-inset hover:text-foreground aria-[current=page]:bg-inset aria-[current=page]:font-medium aria-[current=page]:text-foreground"
      >
        <span className="truncate">{label}</span>
      </button>
    </li>
  );

  // Groups nested under their parents, depth first.
  const ordered: { group: GroupSummary; depth: number }[] = [];
  const visit = (parent: Id | null, depth: number) => {
    for (const group of groups.filter((g) => g.parent === parent).sort((a, b) => a.name.localeCompare(b.name))) {
      ordered.push({ group, depth });
      visit(group.id, depth + 1);
    }
  };
  visit(null, 0);

  return (
    <nav
      aria-label={t("hosts.sidebar")}
      className="hidden w-56 shrink-0 flex-col gap-5 overflow-y-auto border-r border-border px-3 py-4 md:flex"
    >
      <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
        {item(t("hosts.title"), { kind: "all" })}
        {favorites ? item(t("hosts.favorites"), { kind: "favorites" }) : null}
        {recent ? item(t("hosts.recent"), { kind: "recent" }) : null}
      </ul>
      {ordered.length > 0 ? (
        <div>
          <h2 className="mb-1 px-3 text-xs font-medium text-muted-foreground">{t("hosts.groups")}</h2>
          <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
            {ordered.map(({ group, depth }) => item(group.name, { kind: "group", id: group.id }, depth))}
          </ul>
        </div>
      ) : null}
      {onOpenObjects ? (
        <div>
          <h2 className="mb-1 px-3 text-xs font-medium text-muted-foreground">{t("hosts.manage")}</h2>
          <ul className="m-0 flex list-none flex-col gap-0.5 p-0">
            <li>
              <button
                type="button"
                onClick={() => onOpenObjects("logins")}
                className="flex h-8 w-full items-center truncate rounded-md px-3 text-left text-sm text-muted-foreground hover:bg-inset hover:text-foreground"
              >
                {t("frame.logins")}
              </button>
            </li>
            {keys ? (
              <li>
                <button
                  type="button"
                  onClick={() => onOpenObjects("keys")}
                  className="flex h-8 w-full items-center truncate rounded-md px-3 text-left text-sm text-muted-foreground hover:bg-inset hover:text-foreground"
                >
                  {t("frame.keys")}
                </button>
              </li>
            ) : null}
            {routes ? (
              <li>
                <button
                  type="button"
                  onClick={() => onOpenObjects("routes")}
                  className="flex h-8 w-full items-center truncate rounded-md px-3 text-left text-sm text-muted-foreground hover:bg-inset hover:text-foreground"
                >
                  {t("frame.routes")}
                </button>
              </li>
            ) : null}
          </ul>
        </div>
      ) : null}
    </nav>
  );
}

function HostList({
  hosts,
  selected,
  onSelect,
  onFavorite,
  empty,
}: {
  hosts: HostSummary[];
  selected: Id | null;
  onSelect: (id: Id) => void;
  onFavorite: (host: HostSummary) => void;
  empty: React.ReactNode;
}) {
  const { t } = useI18n();
  const list = useRef<HTMLUListElement>(null);

  if (hosts.length === 0) return <>{empty}</>;

  // Up and down move between hosts, selecting as they go.
  const move = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const target =
      event.key === "ArrowDown"
        ? index + 1
        : event.key === "ArrowUp"
          ? index - 1
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? hosts.length - 1
              : null;
    if (target === null || target < 0 || target >= hosts.length) return;
    event.preventDefault();
    const buttons = list.current?.querySelectorAll<HTMLButtonElement>("[data-host]");
    buttons?.[target]?.focus();
    onSelect(hosts[target]!.id);
  };

  return (
    <ul ref={list} aria-label={t("hosts.list")} className="m-0 flex list-none flex-col p-0">
      {hosts.map((host, index) => {
        const isSelected = host.id === selected;
        const tags = Object.entries(host.tags);
        return (
          <li key={host.id} className="group relative flex items-center rounded-md hover:bg-inset/60">
            {isSelected ? <ScopeMark /> : null}
            <button
              type="button"
              data-host=""
              aria-current={isSelected ? "true" : undefined}
              onClick={() => onSelect(host.id)}
              onKeyDown={(event) => move(event, index)}
              className="flex min-w-0 flex-1 flex-col gap-0.5 rounded-md py-2 pr-2 pl-3 text-left focus-visible:outline-offset-0"
            >
              <span className="flex items-center gap-2">
                <span className="truncate text-sm font-medium text-foreground">{host.name}</span>
                {host.restored ? <span className="shrink-0 text-xs text-primary">{t("hosts.restored")}</span> : null}
              </span>
              <span className="flex min-w-0 items-center gap-1.5 text-xs text-muted-foreground">
                <span className="truncate font-mono">
                  {host.username ? `${host.username}@` : ""}
                  {host.address}
                  {host.port !== 22 ? `:${host.port}` : ""}
                </span>
                {host.route.kind !== "direct" ? (
                  <span className="shrink-0 truncate">· {routeText(t, host.route)}</span>
                ) : null}
              </span>
            </button>
            {tags.length > 0 ? (
              <span className="mr-1 hidden shrink-0 items-center gap-1 @2xl:flex">
                {tags.slice(0, 2).map(([key, value]) => (
                  <Tag key={key}>{value ? `${key}: ${value}` : key}</Tag>
                ))}
                {tags.length > 2 ? <Tag>+{tags.length - 2}</Tag> : null}
              </span>
            ) : null}
            <IconButton
              label={
                host.favorite
                  ? t("hosts.favorite.remove", { name: host.name })
                  : t("hosts.favorite.add", { name: host.name })
              }
              pressed={host.favorite}
              onClick={() => onFavorite(host)}
              className={`mr-1 ${host.favorite ? "" : "opacity-0 group-focus-within:opacity-100 group-hover:opacity-100 focus-visible:opacity-100"}`}
            >
              <Star aria-hidden="true" className={`size-4 ${host.favorite ? "fill-current" : ""}`} />
            </IconButton>
          </li>
        );
      })}
    </ul>
  );
}
