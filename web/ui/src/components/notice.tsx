// SPDX-License-Identifier: Apache-2.0
import { useEffect, useRef, useState } from "react";

export interface NoticeProps {
  /** What just happened. */
  message: string;
  /** An action such as Undo, if there is one. */
  action?: { label: string; onAction: () => void };
  /** Called when the notice goes away. */
  onDismiss: () => void;
  /** Milliseconds before it goes away; paused while pointed at or focused. */
  duration?: number;
}

/**
 * A brief notice at the bottom of the window, announced politely. It stays
 * while the pointer or keyboard focus is on it, so its action stays reachable.
 */
export function Notice({ message, action, onDismiss, duration = 8000 }: NoticeProps) {
  const [held, setHeld] = useState(false);
  const dismiss = useRef(onDismiss);
  useEffect(() => {
    dismiss.current = onDismiss;
  });
  useEffect(() => {
    if (held) return;
    const timer = setTimeout(() => dismiss.current(), duration);
    return () => clearTimeout(timer);
  }, [held, duration, message]);

  return (
    <div
      role="status"
      onPointerEnter={() => setHeld(true)}
      onPointerLeave={() => setHeld(false)}
      onFocus={() => setHeld(true)}
      onBlur={() => setHeld(false)}
      className="fixed bottom-5 left-1/2 z-40 flex -translate-x-1/2 items-center gap-4 rounded-lg bg-foreground py-2 pr-2 pl-4 text-sm text-background shadow-[0_10px_30px_-10px_rgba(28,25,23,0.5)]"
    >
      <span>{message}</span>
      {action ? (
        <button
          type="button"
          onClick={action.onAction}
          className="rounded-md px-3 py-1.5 font-medium text-[color-mix(in_srgb,var(--primary)_55%,var(--background))] hover:bg-[color-mix(in_srgb,var(--background)_12%,transparent)]"
        >
          {action.label}
        </button>
      ) : null}
    </div>
  );
}
