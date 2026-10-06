// SPDX-License-Identifier: Apache-2.0
import { Lockup } from "@scoplen/ui";
import { useState } from "react";
import { useI18n } from "./i18n";
import type { AppInfo, Status } from "./ipc/bindings";
import { PassphraseSettings } from "./local-data";

type View = "about" | "settings";

/** The application once its local data is open. */
export function Shell({
  info,
  status,
  onStatus,
}: {
  info: AppInfo;
  status: Status & { state: "open" };
  onStatus: (status: Status) => void;
}) {
  const { t } = useI18n();
  const [view, setView] = useState<View>("about");
  const views: View[] = ["about", "settings"];

  return (
    <>
      <header className="flex h-12 items-center gap-6 border-b border-border px-4">
        <Lockup size="1.0625rem" />
        <nav aria-label={t("shell.nav")}>
          <ul className="flex gap-1">
            {views.map((name) => (
              <li key={name}>
                <button
                  type="button"
                  aria-current={view === name ? "page" : undefined}
                  onClick={() => setView(name)}
                  className="rounded-md px-3 py-1.5 text-sm text-muted-foreground hover:text-foreground aria-[current=page]:bg-inset aria-[current=page]:text-foreground"
                >
                  {t(`shell.nav.${name}`)}
                </button>
              </li>
            ))}
          </ul>
        </nav>
      </header>
      <main id="main" tabIndex={-1} className="mx-auto max-w-xl px-6 py-16 focus:outline-none">
        {view === "about" ? <About info={info} /> : <Settings status={status} onStatus={onStatus} />}
      </main>
    </>
  );
}

function About({ info }: { info: AppInfo }) {
  const { t } = useI18n();
  return (
    <>
      <h1 className="font-serif text-2xl font-medium">{t("about.title")}</h1>
      <p className="mt-3 text-base">{t("about.summary")}</p>
      <div className="mt-8 space-y-2 text-sm text-muted-foreground">
        <p>{t("about.version", { version: info.version, platform: info.platform })}</p>
        <p>{t("about.maturity")}</p>
        <p>{t("about.project")}</p>
        <p>{t("about.license")}</p>
      </div>
    </>
  );
}

function Settings({ status, onStatus }: { status: Status & { state: "open" }; onStatus: (status: Status) => void }) {
  const { t } = useI18n();
  return (
    <>
      <h1 className="font-serif text-2xl font-medium">{t("settings.title")}</h1>
      <PassphraseSettings status={status} onStatus={onStatus} />
    </>
  );
}
