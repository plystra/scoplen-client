// SPDX-License-Identifier: Apache-2.0
import { ScoplenTerminalElement, TerminalError, type TerminalBytes } from "../src";

function mount(options: { source?: ScoplenTerminalElement["source"]; sink?: ScoplenTerminalElement["sink"] } = {}) {
  const element = document.createElement("scoplen-terminal") as ScoplenTerminalElement;
  element.source = options.source ?? null;
  element.sink = options.sink ?? null;
  document.body.append(element);
  return element;
}

const flush = async () => {
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
};

afterEach(() => {
  document.body.replaceChildren();
});

describe("terminal experience adapter", () => {
  it("does not write xterm constructor-only dimensions when applying a profile", () => {
    const element = mount();
    let assigned: Record<string, unknown> | undefined;
    const terminal = {
      options: { cols: 80, rows: 24, cursorBlink: false },
      dispose: () => undefined,
    } as unknown as { options: Record<string, unknown> };
    Object.defineProperty(terminal, "options", {
      configurable: true,
      get: () => ({ cols: 80, rows: 24, cursorBlink: false }),
      set: (value: Record<string, unknown>) => {
        assigned = value;
      },
    });
    (element as unknown as { terminal: typeof terminal }).terminal = terminal;

    element.profile = { cursorBlink: true, scrollback: 8 };

    expect(assigned).toMatchObject({ cursorBlink: true, scrollback: 8 });
    expect(assigned).not.toHaveProperty("cols");
    expect(assigned).not.toHaveProperty("rows");
  });

  it("uses xterm scrollback and search while retaining the bounded byte history", async () => {
    let emit: ((chunk: TerminalBytes) => void) | undefined;
    const element = mount({
      source: {
        subscribe(onChunk) {
          emit = onChunk;
          return () => undefined;
        },
      },
    });
    element.profile = { scrollback: 8, fontSize: 15, cursorStyle: "underline" };
    emit?.(new TextEncoder().encode("first line\r\nsecond line\r\nthird line"));
    await flush();

    expect(element.profile.scrollback).toBe(8);
    expect(element.findNext("second line")).toBe(true);
    expect(element.findPrevious("first line")).toBe(true);
    expect([...element.getOutput()]).toEqual([...new TextEncoder().encode("first line\r\nsecond line\r\nthird line")]);
    expect(element.getAttribute("role")).toBe("application");
    expect(element.getAttribute("aria-label")).toBe("Terminal");
  });

  it("copies all retained output, pastes through the binary sink, and reports clipboard failures", async () => {
    let emit: ((chunk: TerminalBytes) => void) | undefined;
    const writes: number[][] = [];
    const clipboard = {
      readText: async () => "paste me",
      writeText: async (text: string) => {
        if (text.length === 0) {
          throw new Error("empty clipboard write");
        }
      },
    };
    const element = mount({
      source: {
        subscribe(onChunk) {
          emit = onChunk;
          return () => undefined;
        },
      },
      sink: (bytes) => {
        writes.push([...bytes]);
      },
    });
    element.clipboard = clipboard;
    emit?.(new TextEncoder().encode("copy me"));
    await flush();
    expect(await element.copyAll()).toBe(true);
    expect(await element.paste()).toBe(true);
    await flush();
    expect(new TextDecoder().decode(Uint8Array.from(writes[0] ?? []))).toBe("paste me");

    const errors: TerminalError[] = [];
    element.addEventListener("terminal-error", (event) => errors.push((event as CustomEvent<TerminalError>).detail));
    element.clipboard = { writeText: async () => Promise.reject(new Error("permission denied")) };
    expect(await element.copyAll()).toBe(false);
    expect(errors).toHaveLength(1);
    expect(errors[0].code).toBe("clipboard");
  });

  it("requires confirmation before enabling broadcast input and exposes the visible state", async () => {
    const primary: number[][] = [];
    const broadcast: number[][] = [];
    const element = mount({
      sink: (bytes) => {
        primary.push([...bytes]);
      },
    });
    element.broadcastSink = (bytes) => {
      broadcast.push([...bytes]);
    };
    expect(await element.setBroadcastInput(true)).toBe(false);
    expect(element.broadcastInput).toBe(false);
    expect(await element.setBroadcastInput(true, () => false)).toBe(false);
    expect(await element.setBroadcastInput(true, () => true)).toBe(true);
    const indicator = element.shadowRoot?.querySelector("[part='broadcast-indicator']") as HTMLElement;
    expect(element.broadcastInput).toBe(true);
    expect(indicator.hidden).toBe(false);

    const events: number[][] = [];
    element.addEventListener("terminal-broadcast-input", (event) =>
      events.push([...(event as CustomEvent<TerminalBytes>).detail]),
    );
    await element.send(new Uint8Array([65]));
    expect(primary).toEqual([[65]]);
    expect(broadcast).toEqual([[65]]);
    expect(events).toEqual([[65]]);
    await element.setBroadcastInput(false);
    expect(indicator.hidden).toBe(true);
  });

  it("reports a primary sink failure while keeping the write rejection visible", async () => {
    const element = mount({
      sink: {
        write: async () => {
          throw new Error("transport closed");
        },
      },
    });
    const errors: TerminalError[] = [];
    element.addEventListener("terminal-error", (event) => errors.push((event as CustomEvent<TerminalError>).detail));
    await expect(element.send(new Uint8Array([1]))).rejects.toThrow("transport closed");
    expect(errors).toHaveLength(1);
    expect(errors[0].code).toBe("stream");
  });

  it("cleans xterm and source subscriptions on disconnect and restores the fallback on reconnect", async () => {
    let cancelled = 0;
    let emit: ((chunk: TerminalBytes) => void) | undefined;
    const element = mount({
      source: {
        subscribe(onChunk) {
          emit = onChunk;
          return () => {
            cancelled += 1;
          };
        },
      },
    });
    emit?.(new Uint8Array([120]));
    await flush();
    element.remove();
    expect(cancelled).toBe(1);
    document.body.append(element);
    expect(cancelled).toBe(1);
    expect(element.getOutput()).toEqual(new Uint8Array([120]));
  });
});
