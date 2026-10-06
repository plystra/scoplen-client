// SPDX-License-Identifier: Apache-2.0
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { frameChannel } from "../src/ipc/frames";

afterEach(() => clearMocks());

type Internals = { runCallback: (id: number, data: unknown) => void };

describe("frameChannel", () => {
  it("delivers raw frames as bytes in sending order", () => {
    mockIPC(() => undefined);
    const frames: number[][] = [];
    const channel = frameChannel((frame) => frames.push([...frame]));
    const internals = (window as unknown as { __TAURI_INTERNALS__: Internals }).__TAURI_INTERNALS__;
    // The second frame arrives first; the channel restores order.
    internals.runCallback(channel.id, { index: 1, message: new Uint8Array([0, 255]).buffer });
    internals.runCallback(channel.id, { index: 0, message: new TextEncoder().encode("\u001b[1m").buffer });
    expect(frames).toEqual([
      [27, 91, 49, 109],
      [0, 255],
    ]);
  });
});
