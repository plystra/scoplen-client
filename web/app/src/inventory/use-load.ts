// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from "react";
import { useInventory, type Failure, type Outcome } from "./api";

export type Loaded<T> = { state: "loading" } | { state: "ready"; data: T } | { state: "failed"; reference: string };

/**
 * Loads data from the core, and loads it again whenever the inventory
 * changes or `key` changes. `key` names everything `load` depends on.
 * Earlier answers that arrive late are ignored.
 */
export function useLoad<T>(load: () => Promise<Outcome<T, Failure>>, key: string): Loaded<T> {
  const api = useInventory();
  const [result, setResult] = useState<Loaded<T>>({ state: "loading" });
  const [revision, setRevision] = useState(0);
  const latest = useRef(load);
  useEffect(() => {
    latest.current = load;
  });

  useEffect(() => api.onChange(() => setRevision((n) => n + 1)), [api]);

  useEffect(() => {
    let current = true;
    latest.current().then(
      (outcome) => {
        if (!current) return;
        setResult(
          outcome.status === "ok"
            ? { state: "ready", data: outcome.data }
            : { state: "failed", reference: outcome.error.reference },
        );
      },
      (error: unknown) => {
        if (current) setResult({ state: "failed", reference: String(error) });
      },
    );
    return () => {
      current = false;
    };
  }, [key, revision, api]);

  return result;
}
