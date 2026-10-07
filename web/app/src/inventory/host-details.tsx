// SPDX-License-Identifier: Apache-2.0
import { Button, IconButton, Tag } from "@scoplen/ui";
import { X } from "lucide-react";
import { useState, type ReactNode } from "react";
import { useI18n } from "../i18n";
import {
  useInventory,
  type CredentialLabel,
  type GroupSummary,
  type HostDetails,
  type Id,
  type LoginSummary,
} from "./api";
import { credentialText, routeText } from "./labels";
import { useLoad } from "./use-load";

/** Everything about one host, beside the list. */
export function HostDetailsPanel({
  id,
  groups,
  onClose,
  onDelete,
}: {
  id: Id;
  groups: GroupSummary[];
  onClose: () => void;
  onDelete: (host: HostDetails) => void;
}) {
  const { t } = useI18n();
  const api = useInventory();
  const details = useLoad(() => api.host(id), id);

  return (
    <aside
      aria-labelledby="host-details-title"
      className="flex w-[22rem] shrink-0 flex-col border-l border-border bg-raised max-lg:absolute max-lg:inset-y-0 max-lg:right-0 max-lg:z-20 max-lg:shadow-[-12px_0_32px_-16px_rgba(28,25,23,0.35)]"
    >
      <div className="flex items-start gap-2 px-5 pt-5">
        <div className="min-w-0 flex-1">
          {details.state === "ready" && details.data ? (
            <>
              <h2 id="host-details-title" className="m-0 font-serif text-lg font-medium break-words">
                {details.data.name}
              </h2>
              <p className="mt-0.5 mb-0 font-mono text-xs break-all text-muted-foreground">
                {details.data.address}
                {details.data.port !== 22 ? `:${details.data.port}` : ""}
              </p>
            </>
          ) : (
            <h2 id="host-details-title" className="sr-only">
              {t("hosts.loading")}
            </h2>
          )}
        </div>
        <IconButton label={t("host.close")} onClick={onClose} className="-mr-2">
          <X aria-hidden="true" className="size-4" />
        </IconButton>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5">
        {details.state === "failed" ? (
          <p role="alert" className="text-sm text-attention">
            {t("failed.body")} {t("startup.error.reference", { reference: details.reference })}
          </p>
        ) : null}
        {details.state === "ready" && details.data === null ? (
          <p className="text-sm text-muted-foreground">{t("host.missing")}</p>
        ) : null}
        {details.state === "ready" && details.data ? <Body host={details.data} groups={groups} /> : null}
      </div>

      {details.state === "ready" && details.data ? (
        <div className="border-t border-border px-5 py-3">
          <Button
            variant="ghost"
            className="-ml-3 text-attention hover:bg-inset"
            onClick={() => onDelete(details.data!)}
          >
            {t("host.delete")}
          </Button>
        </div>
      ) : null}
    </aside>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section className="mb-6 last:mb-0">
      <h3 className="mt-0 mb-2 text-xs font-medium text-muted-foreground">{title}</h3>
      {children}
    </section>
  );
}

function Body({ host, groups }: { host: HostDetails; groups: GroupSummary[] }) {
  const { t } = useI18n();
  const tags = Object.entries(host.tags);
  const memberOf = groups.filter((g) => host.groups.includes(g.id));

  return (
    <>
      {host.logins.length === 1 ? (
        <Section title={t("host.login")}>
          <LoginLine login={host.logins[0]!} />
        </Section>
      ) : host.logins.length > 1 ? (
        <Section title={t("host.logins")}>
          <ul className="m-0 flex list-none flex-col gap-3 p-0">
            {host.logins.map((login) => (
              <li key={login.id}>
                <LoginLine login={login} />
              </li>
            ))}
          </ul>
        </Section>
      ) : null}

      {tags.length > 0 ? (
        <Section title={t("host.tags")}>
          <div className="flex flex-wrap gap-1.5">
            {tags.map(([key, value]) => (
              <Tag key={key}>{value ? `${key}: ${value}` : key}</Tag>
            ))}
          </div>
        </Section>
      ) : null}

      {memberOf.length > 0 ? (
        <Section title={t("host.groups")}>
          <p className="m-0 text-sm">{memberOf.map((g) => g.name).join(", ")}</p>
        </Section>
      ) : null}

      {host.notes ? (
        <Section title={t("host.notes")}>
          <p className="m-0 text-sm whitespace-pre-wrap">{host.notes}</p>
        </Section>
      ) : null}
    </>
  );
}

function LoginLine({ login }: { login: LoginSummary }) {
  const { t } = useI18n();
  return (
    <div className="flex flex-col gap-0.5">
      <span className="flex items-center gap-2">
        <span className="font-mono text-sm">{login.name ?? login.username}</span>
        {login.isDefault && login.name ? (
          <span className="text-xs text-muted-foreground">{t("host.default")}</span>
        ) : null}
      </span>
      <span className="text-xs text-muted-foreground">
        {credentialText(t, login.credential)} · {routeText(t, login.route)}
      </span>
      {login.credential?.publicKey ? <CopyPublicKey credential={login.credential} /> : null}
    </div>
  );
}

/**
 * The public half of a key, for adding to a host's `authorized_keys`.
 * Copying is the whole task, so the key itself is not shown.
 */
function CopyPublicKey({ credential }: { credential: CredentialLabel }) {
  const { t } = useI18n();
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      title={t("host.publicKey.hint")}
      onClick={() =>
        void navigator.clipboard.writeText(credential.publicKey ?? "").then(() => {
          setCopied(true);
          setTimeout(() => setCopied(false), 2000);
        })
      }
      className="mt-1 self-start rounded text-xs text-primary hover:underline"
    >
      <span aria-live="polite">{copied ? t("host.publicKey.copied") : t("host.publicKey.copy")}</span>
    </button>
  );
}
