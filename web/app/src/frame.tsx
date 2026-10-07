// SPDX-License-Identifier: Apache-2.0
import { IconButton } from "@scoplen/ui";
import { Settings } from "lucide-react";
import type { ReactNode } from "react";
import { useI18n } from "./i18n";
import type { Platform } from "./ipc/bindings";

export interface FrameTab {
  id: string;
  label: string;
}

/**
 * The window: a strip of tabs above the content. The first tab is always
 * Hosts; each session will add a tab after it. On macOS the strip is the
 * title bar, with room left for the window controls, and dragging its empty
 * space moves the window.
 */
export function Frame({
  platform,
  tabs,
  active,
  onTab,
  onSettings,
  settingsActive,
  children,
}: {
  platform: Platform;
  tabs: FrameTab[];
  active: string;
  onTab: (id: string) => void;
  onSettings: () => void;
  settingsActive: boolean;
  children: ReactNode;
}) {
  const { t } = useI18n();
  return (
    <div className="flex h-screen flex-col bg-background">
      <header
        data-tauri-drag-region
        className={`flex h-11 shrink-0 items-end gap-1 border-b border-border bg-inset pr-2 ${platform === "macos" ? "pl-[5.25rem]" : "pl-2"}`}
      >
        <div role="tablist" aria-label={t("frame.tabs")} className="flex min-w-0 items-end gap-1">
          {tabs.map((tab) => {
            const selected = tab.id === active && !settingsActive;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={selected}
                onClick={() => onTab(tab.id)}
                className="relative -mb-px h-8 max-w-56 truncate rounded-t-md border border-b-0 border-transparent px-4 text-sm text-muted-foreground hover:text-foreground aria-selected:border-border aria-selected:bg-background aria-selected:text-foreground"
              >
                {tab.label}
              </button>
            );
          })}
        </div>
        <div data-tauri-drag-region className="h-full flex-1" />
        <IconButton label={t("frame.settings")} pressed={settingsActive} onClick={onSettings} className="mb-1.5">
          <Settings aria-hidden="true" className="size-4" />
        </IconButton>
      </header>
      <div className="relative min-h-0 flex-1">{children}</div>
    </div>
  );
}
