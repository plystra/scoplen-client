// SPDX-License-Identifier: Apache-2.0
import { Button, Dialog } from "@scoplen/ui";
import { useCallback, useEffect, useRef, useState } from "react";
import { useI18n } from "./i18n";
import { commands, type HostKeyDetails } from "./ipc/bindings";
import { frameChannel } from "./ipc/frames";

export type SessionState = "verifying" | "trusting" | "connecting" | "connected" | "closed" | "failed";

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
  const [state, setState] = useState<SessionState>("verifying");
  const [output, setOutput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [hostKey, setHostKey] = useState<HostKeyDetails | null>(null);
  const [trustError, setTrustError] = useState<string | null>(null);
  const [trustBusy, setTrustBusy] = useState(false);
  const mounted = useRef(true);

  const connectSession = useCallback(async () => {
    if (!mounted.current) return;
    setState("connecting");
    const decoder = new TextDecoder();
    const frames = frameChannel((frame) => {
      if (!mounted.current) return;
      setOutput((current) => current + decoder.decode(frame, { stream: true }));
    });
    try {
      setState("connected");
      const result = await commands.sessionConnect(profileId, null, true, frames);
      if (!mounted.current) return;
      const tail = decoder.decode();
      if (tail) setOutput((current) => current + tail);
      if (result.status === "ok") {
        setState("closed");
      } else {
        setState("failed");
        setError(result.error);
      }
    } catch (cause) {
      if (!mounted.current) return;
      setState("failed");
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  }, [profileId]);

  useEffect(() => {
    mounted.current = true;

    void (async () => {
      try {
        await Promise.resolve();
        if (!mounted.current) return;
        setState("verifying");
        setError(null);
        setHostKey(null);
        setTrustError(null);
        const result = await commands.sessionHostKey(profileId);
        if (!mounted.current) return;
        if (result.status === "error") {
          setState("failed");
          setError(result.error);
          return;
        }
        setHostKey(result.data);
        if (result.data.status === "trusted") void connectSession();
        else setState("trusting");
      } catch (cause) {
        if (!mounted.current) return;
        setState("failed");
        setError(cause instanceof Error ? cause.message : String(cause));
      }
    })();

    return () => {
      mounted.current = false;
    };
  }, [connectSession, profileId]);

  const cancelTrust = () => {
    if (trustBusy) return;
    setHostKey(null);
    setState("failed");
    setError(t("session.trust.cancelled"));
  };

  const trustAndConnect = async () => {
    if (!hostKey || trustBusy) return;
    setTrustBusy(true);
    setTrustError(null);
    const result = await commands.sessionTrustHostKey(profileId, hostKey.algorithm, hostKey.key, hostKey.fingerprint);
    if (!mounted.current) return;
    setTrustBusy(false);
    if (result.status === "error") {
      setTrustError(result.error);
      return;
    }
    setHostKey(null);
    void connectSession();
  };

  const status =
    state === "verifying"
      ? t("session.verifying")
      : state === "trusting"
        ? t("session.firstConnection")
        : state === "connecting"
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
      <Dialog
        open={hostKey !== null}
        onOpenChange={(open) => {
          if (!open) cancelTrust();
        }}
        title={hostKey?.status === "changed" ? t("session.trust.changedTitle") : t("session.trust.title")}
        description={hostKey?.status === "changed" ? t("session.trust.changedBody") : t("session.trust.firstBody")}
        closeLabel={t("session.trust.cancel")}
        footer={
          <>
            <Button variant="ghost" onClick={cancelTrust} disabled={trustBusy}>
              {t("session.trust.cancel")}
            </Button>
            <Button variant="primary" busy={trustBusy} onClick={() => void trustAndConnect()}>
              {t("session.trust.connect")}
            </Button>
          </>
        }
      >
        {hostKey ? (
          <div className="space-y-4 text-sm">
            <dl className="grid gap-3">
              <div>
                <dt className="text-muted-foreground">{t("session.trust.algorithm")}</dt>
                <dd className="m-0 mt-1 font-mono">{hostKey.algorithm}</dd>
              </div>
              <div>
                <dt className="text-muted-foreground">{t("session.trust.fingerprint")}</dt>
                <dd className="m-0 mt-1 break-all font-mono">{hostKey.fingerprint}</dd>
              </div>
              <div>
                <dt className="text-muted-foreground">{t("session.trust.publicKey")}</dt>
                <dd className="m-0 mt-1 break-all rounded-md bg-inset p-3 font-mono text-xs leading-5">
                  {hostKey.key}
                </dd>
              </div>
            </dl>
            {trustError ? (
              <p role="alert" className="m-0 text-sm text-attention">
                {trustError}
              </p>
            ) : null}
          </div>
        ) : null}
      </Dialog>
    </main>
  );
}
