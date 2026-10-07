// SPDX-License-Identifier: Apache-2.0
import { Button, Confirm, IconButton, Notice, ScopeMark, SearchField, Tag } from "@scoplen/ui";
import { Plus, Star } from "lucide-react";
import { useRef, useState, type KeyboardEvent } from "react";
import { useI18n } from "../i18n";
import { AddHostDialog } from "./add-host";
import { useInventory, type GroupSummary, type HostSource, type HostSummary, type Id } from "./api";
import { HostDetailsPanel } from "./host-details";
import { routeText } from "./labels";
import { useLoad } from "./use-load";

type Toast = { message: string; undo?: string } | null;

/** The Hosts tab: where the inventory is, and the way into every session. */
export function HostsHome() {
  const { t } = useI18n();
  const api = useInventory();
  const [source, setSource] = useState<HostSource>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState<Id | null>(null);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<HostSummary | null>(null);
  const [deleteBusy, setDeleteBusy] = useState(false);
  const [toast, setToast] = useState<Toast>(null);
  const search = useRef<HTMLInputElement>(null);

  const sourceKey = source.kind === "group" ? `group:${source.id}` : source.kind;
  const areas = useLoad(() => api.areas(), "areas");
  const groups = useLoad(() => api.groups(), "groups");
  const hosts = useLoad(() => api.hosts(source, query.trim()), `${sourceKey}|${query.trim()}`);

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

  const onListKeyDown = (event: KeyboardEvent) => {
    // "/" focuses the search, as in many lists, unless the user is typing.
    if (event.key === "/" && !(event.target instanceof HTMLInputElement)) {
      event.preventDefault();
      search.current?.focus();
    }
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
                    <Button variant="primary" className="mt-5" onClick={() => setAdding(true)}>
                      {t("hosts.add")}
                    </Button>
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

function Empty({ children }: { children: React.ReactNode }) {
  return (
    <div className="flex flex-col items-center px-6 py-20 text-center text-sm text-muted-foreground">{children}</div>
  );
}

function Sidebar({
  source,
  onSource,
  favorites,
  recent,
  groups,
}: {
  source: HostSource;
  onSource: (source: HostSource) => void;
  favorites: boolean;
  recent: boolean;
  groups: GroupSummary[];
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
