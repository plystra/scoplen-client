// SPDX-License-Identifier: Apache-2.0
import { render, act } from "@testing-library/react";
import { Terminal } from "../src/react";
import type { TerminalBytes } from "../src";

afterEach(() => {
  document.body.replaceChildren();
});

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

describe("Terminal React binding", () => {
  it("passes binary source/sink and cleans up its source on unmount", async () => {
    let cancelled = 0;
    let emit: ((bytes: TerminalBytes) => void) | undefined;
    const source = {
      subscribe(onChunk: (bytes: TerminalBytes) => void) {
        emit = onChunk;
        onChunk(new Uint8Array([7]));
        return () => {
          cancelled += 1;
        };
      },
    };
    const writes: number[][] = [];
    const outputs: number[][] = [];
    const view = render(
      <Terminal
        source={source}
        sink={(bytes) => {
          writes.push([...bytes]);
        }}
        onTerminalOutput={(bytes) => outputs.push([...bytes])}
      />,
    );
    const element = view.container.firstElementChild;
    expect(element).toBeInstanceOf(HTMLElement);
    expect(outputs).toEqual([[7]]);
    emit?.(new Uint8Array([0, 255]));
    (element as HTMLElement).dispatchEvent(new KeyboardEvent("keydown", { key: "x", bubbles: true }));
    await act(flush);
    expect(outputs).toEqual([[7], [0, 255]]);
    expect(writes).toEqual([[120]]);
    view.unmount();
    expect(cancelled).toBe(1);
  });
});
