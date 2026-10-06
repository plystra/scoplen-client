// SPDX-License-Identifier: Apache-2.0
import { useLayoutEffect, useRef, type KeyboardEvent, type ReactNode } from "react";

export interface ToolbarProps {
  /** Accessible name of the group of controls. */
  label: string;
  orientation?: "horizontal" | "vertical";
  children: ReactNode;
  className?: string;
}

const enabled = (el: Element): el is HTMLElement =>
  el instanceof HTMLElement && !el.hasAttribute("disabled") && el.getAttribute("aria-disabled") !== "true";

/**
 * A group of controls reached with one Tab stop and moved through with the
 * arrow keys, Home, and End (WAI-ARIA toolbar pattern). The control that last
 * had focus keeps the Tab stop.
 */
export function Toolbar({ label, orientation = "horizontal", children, className }: ToolbarProps) {
  const root = useRef<HTMLDivElement>(null);

  const items = () =>
    Array.from(root.current?.querySelectorAll("button, a[href], [role=button], input") ?? []).filter(enabled);

  const makeStop = (target: HTMLElement) => {
    for (const item of items()) {
      item.tabIndex = item === target ? 0 : -1;
      if (item === target) item.dataset.rovingStop = "";
      else delete item.dataset.rovingStop;
    }
  };

  const focusAt = (target: HTMLElement) => {
    makeStop(target);
    target.focus();
  };

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const list = items();
    const index = list.indexOf(document.activeElement as HTMLElement);
    if (index < 0 || list.length === 0) return;
    const next = orientation === "horizontal" ? "ArrowRight" : "ArrowDown";
    const previous = orientation === "horizontal" ? "ArrowLeft" : "ArrowUp";
    let target: HTMLElement | undefined;
    if (event.key === next) target = list[(index + 1) % list.length];
    else if (event.key === previous) target = list[(index - 1 + list.length) % list.length];
    else if (event.key === "Home") target = list[0];
    else if (event.key === "End") target = list[list.length - 1];
    if (target) {
      event.preventDefault();
      focusAt(target);
    }
  };

  // Exactly one control is a Tab stop: the one that last had focus, or the
  // first when that one is gone or disabled.
  useLayoutEffect(() => {
    const list = items();
    const stop = list.find((item) => item.tabIndex === 0 && item.dataset.rovingStop !== undefined) ?? list[0];
    for (const item of list) {
      item.tabIndex = item === stop ? 0 : -1;
      if (item === stop) item.dataset.rovingStop = "";
      else delete item.dataset.rovingStop;
    }
  });

  return (
    <div
      ref={root}
      role="toolbar"
      aria-label={label}
      aria-orientation={orientation}
      onKeyDown={onKeyDown}
      onFocus={(event) => {
        if (enabled(event.target) && items().includes(event.target)) makeStop(event.target);
      }}
      className={className}
    >
      {children}
    </div>
  );
}
