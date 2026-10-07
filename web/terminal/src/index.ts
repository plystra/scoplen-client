// SPDX-License-Identifier: Apache-2.0
export {
  DEFAULT_MAX_INPUT_BYTES,
  DEFAULT_MAX_OUTPUT_BYTES,
  DEFAULT_MAX_PENDING_WRITES,
  TERMINAL_TAG_NAME,
  ScoplenTerminalElement,
  defineTerminalElement,
} from "./terminal-element";
export { TerminalBackpressureError, TerminalBoundsError, TerminalDisconnectedError, TerminalError } from "./types";
export type {
  TerminalBytes,
  TerminalElementEventMap,
  TerminalObjectSink,
  TerminalSink,
  TerminalSource,
  TerminalSubscription,
  TerminalSubscriptionSource,
  TerminalEventName,
} from "./types";
