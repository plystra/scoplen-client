// SPDX-License-Identifier: Apache-2.0
import { TerminalImeCompositionBridge } from "../src/ime";

describe("TerminalImeCompositionBridge", () => {
  it("commits only non-empty compositionend data", () => {
    const commits: string[] = [];
    const bridge = new TerminalImeCompositionBridge((text) => commits.push(text));

    bridge.start();
    bridge.update("n");
    bridge.update("ni");
    expect(bridge.isComposing).toBe(true);
    expect(bridge.end("你")).toBe(true);
    expect(commits).toEqual(["你"]);
    expect(bridge.isComposing).toBe(false);
  });

  it("drops pending text on cancel or blur and ignores late end events", () => {
    const commits: string[] = [];
    const bridge = new TerminalImeCompositionBridge((text) => commits.push(text));

    bridge.start();
    bridge.update("候选");
    expect(bridge.cancel()).toBe(true);
    expect(bridge.end("候选")).toBe(false);
    bridge.start();
    bridge.update("失焦");
    expect(bridge.blur()).toBe(true);
    expect(bridge.end("失焦")).toBe(false);
    expect(commits).toEqual([]);
  });

  it("does not commit an empty final composition", () => {
    const commits: string[] = [];
    const bridge = new TerminalImeCompositionBridge((text) => commits.push(text));

    bridge.start();
    expect(bridge.end("")).toBe(false);
    expect(commits).toEqual([]);
  });
});
