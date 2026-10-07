// SPDX-License-Identifier: Apache-2.0
import { Button } from "@scoplen/ui";
import { useEffect, useState } from "react";
import { useI18n } from "./i18n";
import { commands } from "./ipc/bindings";
import { frameChannel } from "./ipc/frames";

export type SessionState = "connecting" | "connected" | "closed" | "failed";

/** A small output surface for the first live SSH session slice. */
export function SessionTerminal({
  profileId,
  label,
  onClose,
}: {
  profileId: string;
  label: string;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [state, setState] = useState<SessionState>("connecting");
  const [output, setOutput] = useState("");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    const decoder = new TextDecoder();
    const frames = frameChannel((frame) => {
      if (!mounted) return;
      setOutput((current) => current + decoder.decode(frame, { stream: true }));
    });

    void (async () => {
      try {
        if (mounted) setState("connected");
        const result = await commands.sessionConnect(profileId, null, true, frames);
        if (!mounted) return;
        const tail = decoder.decode();
        if (tail) setOutput((current) => current + tail);
        if (result.status === "ok") {
          setState("closed");
        } else {
          setState("failed");
          setError(result.error);
        }
      } catch (cause) {
        if (!mounted) return;
        setState("failed");
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    })();

    return () => {
      mounted = false;
    };
  }, [profileId]);

  const status =
    state === "connecting"
      ? t("session.connecting")
      : state === "connected"
        ? t("session.connected")
        : state === "closed"
          ? t("session.closed")
          : t("session.failed");

  return (
    <main className="flex h-full min-h-0 flex-col bg-[#11100f] text-[#f5f0e8]" aria-labelledby="session-title">
      <header className="flex shrink-0 items-center justify-between gap-3 border-b border-[#3b3732] px-5 py-3">
        <div className="min-w-0">
          <h1 id="session-title" className="truncate font-serif text-lg font-medium">
            {label}
          </h1>
          <p className="m-0 text-xs text-[#b6ada3]" role="status">
            {status}
          </p>
        </div>
        <Button variant="ghost" className="shrink-0 text-[#f5f0e8] hover:bg-[#282521]" onClick={onClose}>
          {t("session.close")}
        </Button>
      </header>
      <div className="min-h-0 flex-1 overflow-auto px-5 py-4">
        {error ? (
          <p role="alert" className="mb-3 whitespace-pre-wrap text-sm text-[#f2a39b]">
            {error}
          </p>
        ) : null}
        <pre
          role="log"
          aria-label={t("session.output")}
          className="m-0 min-h-full whitespace-pre-wrap break-words font-mono text-sm leading-6"
        >
          {output || t("session.waiting")}
        </pre>
      </div>
    </main>
  );
}
