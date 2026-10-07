// SPDX-License-Identifier: Apache-2.0

export type TerminalSplitDirection = "horizontal" | "vertical";

export interface TerminalTab {
  id: string;
  title: string;
}

export interface TerminalPane {
  id: string;
  tabId: string;
}

export type TerminalLayoutNode =
  | { type: "pane"; paneId: string }
  | { type: "split"; direction: TerminalSplitDirection; children: TerminalLayoutNode[] };

export interface TerminalWorkspaceSnapshot {
  tabs: TerminalTab[];
  /** All panes, including panes belonging to inactive tabs. */
  panes: TerminalPane[];
  /** The active tab's layout, or null when no tab is open. */
  layout: TerminalLayoutNode | null;
  activeTabId: string | null;
}

export type TerminalWorkspaceListener = (snapshot: TerminalWorkspaceSnapshot) => void;

interface WorkspaceTab {
  tab: TerminalTab;
  layout: TerminalLayoutNode;
}

function cloneNode(node: TerminalLayoutNode): TerminalLayoutNode {
  return node.type === "pane"
    ? { type: "pane", paneId: node.paneId }
    : { type: "split", direction: node.direction, children: node.children.map(cloneNode) };
}

function requireId(id: string, name: string): string {
  if (!id.trim()) {
    throw new TypeError(`${name} must not be empty`);
  }
  return id;
}

/**
 * Session-agnostic tabs and split layout state. It deliberately stores no
 * connection, transport, or credential; callers attach a terminal element to
 * each pane when a session exists.
 */
export class TerminalWorkspace {
  private tabs: WorkspaceTab[] = [];
  private panes: TerminalPane[] = [];
  private activeTabId: string | null = null;
  private listeners = new Set<TerminalWorkspaceListener>();

  snapshot(): TerminalWorkspaceSnapshot {
    const active = this.activeTabId ? this.tabs.find((entry) => entry.tab.id === this.activeTabId) : undefined;
    return {
      tabs: this.tabs.map(({ tab }) => ({ ...tab })),
      panes: this.panes.map((pane) => ({ ...pane })),
      layout: active ? cloneNode(active.layout) : null,
      activeTabId: this.activeTabId,
    };
  }

  subscribe(listener: TerminalWorkspaceListener): () => void {
    this.listeners.add(listener);
    listener(this.snapshot());
    return () => this.listeners.delete(listener);
  }

  openTab(tab: TerminalTab): string {
    const id = requireId(tab.id, "tab.id");
    const title = requireId(tab.title, "tab.title");
    if (this.tabs.some((entry) => entry.tab.id === id)) {
      throw new Error(`terminal tab already exists: ${id}`);
    }
    const paneId = this.paneId(id);
    this.tabs.push({ tab: { id, title }, layout: { type: "pane", paneId } });
    this.panes.push({ id: paneId, tabId: id });
    this.activeTabId = id;
    this.notify();
    return paneId;
  }

  splitPane(paneId: string, direction: TerminalSplitDirection, newPaneId: string): string {
    requireId(paneId, "paneId");
    const owner = this.tabs.find((entry) => this.containsPane(entry.layout, paneId));
    if (!owner) {
      throw new Error(`terminal pane does not exist: ${paneId}`);
    }
    if (direction !== "horizontal" && direction !== "vertical") {
      throw new TypeError("terminal split direction must be horizontal or vertical");
    }
    const id = requireId(newPaneId, "newPaneId");
    if (this.panes.some((entry) => entry.id === id)) {
      throw new Error(`terminal pane already exists: ${id}`);
    }
    const nextPaneId = id;
    owner.layout = this.replacePane(owner.layout, paneId, {
      type: "split",
      direction,
      children: [
        { type: "pane", paneId },
        { type: "pane", paneId: nextPaneId },
      ],
    });
    this.panes.push({ id: nextPaneId, tabId: owner.tab.id });
    this.notify();
    return nextPaneId;
  }

  setActiveTab(tabId: string): void {
    requireId(tabId, "tabId");
    if (!this.tabs.some((entry) => entry.tab.id === tabId)) {
      throw new Error(`terminal tab does not exist: ${tabId}`);
    }
    if (this.activeTabId === tabId) {
      return;
    }
    this.activeTabId = tabId;
    this.notify();
  }

  closeTab(tabId: string): boolean {
    requireId(tabId, "tabId");
    const tabIndex = this.tabs.findIndex((entry) => entry.tab.id === tabId);
    if (tabIndex < 0) {
      return false;
    }
    const paneIds = this.panes.filter((pane) => pane.tabId === tabId).map((pane) => pane.id);
    this.tabs.splice(tabIndex, 1);
    this.panes = this.panes.filter((pane) => !paneIds.includes(pane.id));
    if (this.activeTabId === tabId) {
      this.activeTabId = this.tabs[tabIndex]?.tab.id ?? this.tabs[tabIndex - 1]?.tab.id ?? null;
    }
    this.notify();
    return true;
  }

  paneForTab(tabId: string): TerminalPane | undefined {
    return this.panes.find((pane) => pane.tabId === tabId);
  }

  private paneId(tabId: string): string {
    return `pane:${tabId}`;
  }

  private replacePane(node: TerminalLayoutNode, paneId: string, replacement: TerminalLayoutNode): TerminalLayoutNode {
    if (node.type === "pane") {
      if (node.paneId !== paneId) {
        throw new Error(`terminal pane does not exist: ${paneId}`);
      }
      return replacement;
    }
    if (!node.children.some((child) => this.containsPane(child, paneId))) {
      throw new Error(`terminal pane does not exist: ${paneId}`);
    }
    return {
      ...node,
      children: node.children.map((child) =>
        this.containsPane(child, paneId) ? this.replacePane(child, paneId, replacement) : child,
      ),
    };
  }

  private containsPane(node: TerminalLayoutNode, paneId: string): boolean {
    return node.type === "pane"
      ? node.paneId === paneId
      : node.children.some((child) => this.containsPane(child, paneId));
  }

  private notify(): void {
    const snapshot = this.snapshot();
    for (const listener of this.listeners) {
      listener(snapshot);
    }
  }
}
