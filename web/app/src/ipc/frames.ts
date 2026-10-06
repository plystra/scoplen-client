// SPDX-License-Identifier: Apache-2.0
import { Channel } from "@tauri-apps/api/core";

/**
 * A channel for a binary stream from the core (src-tauri/src/frames.rs).
 * Each frame arrives as raw bytes, in the order it was sent, and is handed to
 * `onFrame` as a `Uint8Array`. Pass the channel as a command argument.
 */
export function frameChannel(onFrame: (frame: Uint8Array) => void): Channel<ArrayBuffer> {
  return new Channel<ArrayBuffer>((message) => onFrame(new Uint8Array(message)));
}
