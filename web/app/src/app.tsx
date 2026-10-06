// SPDX-License-Identifier: Apache-2.0
import { Button, Lockup, SkipLink } from "@scoplen/ui";
import { useCallback, useEffect, useState } from "react";
import { I18nProvider, useI18n } from "./i18n";
import { commands, type AppInfo, type Locale } from "./ipc/bindings";

type Startup = { state: "loading" } | { state: "ready"; info: AppInfo } | { state: "failed"; reference: string };

/**
 * The language used only if the core cannot be reached, when its choice
 * (`Locale::negotiate` in scoplen-client-core) is unavailable: the webview's
 * own preference, with Traditional Chinese falling back to English.
 */
function initialLocale(): Locale {
  const tag = navigator.language.toLowerCase();
  return tag.startsWith("zh") && !/hant|tw|hk|mo/.test(tag) ? "zh-Hans" : "en";
}

export function App() {
  const [startup, setStartup] = useState<Startup>({ state: "loading" });

  // Asks the core; state changes only when it answers.
  const request = useCallback(() => {
    commands.appInfo().then(
      (info) => setStartup({ state: "ready", info }),
      (error: unknown) => setStartup({ state: "failed", reference: String(error) }),
    );
  }, []);

  useEffect(request, [request]);

  const retry = () => {
    setStartup({ state: "loading" });
    request();
  };

  const locale = startup.state === "ready" ? startup.info.locale : initialLocale();
  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  useEffect(() => {
    if (startup.state === "ready") void commands.shellReady();
  }, [startup.state]);

  return (
    <I18nProvider locale={locale}>
      {startup.state === "ready" ? <Shell info={startup.info} /> : null}
      {startup.state === "failed" ? <StartupError reference={startup.reference} onRetry={retry} /> : null}
    </I18nProvider>
  );
}

function Shell({ info }: { info: AppInfo }) {
  const { t } = useI18n();
  return (
    <>
      <SkipLink target="main">{t("shell.skip")}</SkipLink>
      <header className="flex h-12 items-center border-b border-border px-4">
        <Lockup size="1.0625rem" />
      </header>
      <main id="main" tabIndex={-1} className="mx-auto max-w-xl px-6 py-16 focus:outline-none">
        <h1 className="font-serif text-2xl font-medium">{t("about.title")}</h1>
        <p className="mt-3 text-base">{t("about.summary")}</p>
        <div className="mt-8 space-y-2 text-sm text-muted-foreground">
          <p>{t("about.version", { version: info.version, platform: info.platform })}</p>
          <p>{t("about.maturity")}</p>
          <p>{t("about.project")}</p>
          <p>{t("about.license")}</p>
        </div>
      </main>
    </>
  );
}

function StartupError({ reference, onRetry }: { reference: string; onRetry: () => void }) {
  const { t } = useI18n();
  return (
    <main id="main" className="mx-auto max-w-xl px-6 py-16">
      <div role="alert">
        <h1 className="font-serif text-xl font-medium">{t("startup.error.title")}</h1>
        <p className="mt-3 text-sm">{t("startup.error.body")}</p>
        <p className="mt-3 font-mono text-xs text-muted-foreground select-all">
          {t("startup.error.reference", { reference })}
        </p>
      </div>
      <Button variant="primary" className="mt-6" onClick={onRetry}>
        {t("startup.error.retry")}
      </Button>
    </main>
  );
}
