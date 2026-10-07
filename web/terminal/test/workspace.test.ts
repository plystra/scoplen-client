// SPDX-License-Identifier: Apache-2.0
import { TerminalWorkspace } from "../src";

describe("TerminalWorkspace", () => {
  it("keeps tabs independent and splits panes inside the active tab", () => {
    const workspace = new TerminalWorkspace();
    const firstPane = workspace.openTab({ id: "one", title: "One" });
    const secondPane = workspace.splitPane(firstPane, "vertical", "pane:logs");
    const secondTabPane = workspace.openTab({ id: "two", title: "Two" });

    expect(workspace.snapshot()).toMatchObject({
      activeTabId: "two",
      tabs: [
        { id: "one", title: "One" },
        { id: "two", title: "Two" },
      ],
      panes: [
        { id: firstPane, tabId: "one" },
        { id: secondPane, tabId: "one" },
        { id: secondTabPane, tabId: "two" },
      ],
      layout: { type: "pane", paneId: secondTabPane },
    });

    workspace.setActiveTab("one");
    expect(workspace.snapshot().layout).toEqual({
      type: "split",
      direction: "vertical",
      children: [
        { type: "pane", paneId: firstPane },
        { type: "pane", paneId: secondPane },
      ],
    });
    expect(workspace.closeTab("one")).toBe(true);
    expect(workspace.snapshot().activeTabId).toBe("two");
    expect(workspace.snapshot().layout).toEqual({ type: "pane", paneId: secondTabPane });
  });

  it("notifies subscribers with isolated snapshots and rejects duplicate tabs", () => {
    const workspace = new TerminalWorkspace();
    const snapshots: string[] = [];
    const unsubscribe = workspace.subscribe((snapshot) => snapshots.push(snapshot.activeTabId ?? "empty"));
    workspace.openTab({ id: "one", title: "One" });
    expect(snapshots).toEqual(["empty", "one"]);

    expect(() => workspace.openTab({ id: "one", title: "Duplicate" })).toThrow(/already exists/);
    unsubscribe();
    workspace.closeTab("one");
    expect(snapshots).toEqual(["empty", "one"]);
  });

  it("can split a nested pane without rewriting its sibling", () => {
    const workspace = new TerminalWorkspace();
    const firstPane = workspace.openTab({ id: "one", title: "One" });
    const secondPane = workspace.splitPane(firstPane, "horizontal", "pane:two");
    const thirdPane = workspace.splitPane(secondPane, "vertical", "pane:three");

    expect(workspace.snapshot().layout).toEqual({
      type: "split",
      direction: "horizontal",
      children: [
        { type: "pane", paneId: firstPane },
        {
          type: "split",
          direction: "vertical",
          children: [
            { type: "pane", paneId: secondPane },
            { type: "pane", paneId: thirdPane },
          ],
        },
      ],
    });
  });

  it("keeps connection state outside the layout model", () => {
    const workspace = new TerminalWorkspace();
    expect(workspace.snapshot()).toEqual({ tabs: [], panes: [], layout: null, activeTabId: null });
    expect(workspace.paneForTab("missing")).toBeUndefined();
  });
});
