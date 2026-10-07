// SPDX-License-Identifier: Apache-2.0
/* eslint-disable @typescript-eslint/no-invalid-void-type */

/** A chunk crossing the terminal boundary is always binary. */
export type TerminalBytes = Uint8Array;

export interface TerminalSubscription {
  unsubscribe(): void;
}

export interface TerminalSourceOptions {
  signal: AbortSignal;
}

/**
 * A callback source is deliberately smaller than a UI framework observable.
 * The source owns the transport and returns the one operation needed to stop
 * it. A source may also be a ReadableStream or AsyncIterable of Uint8Arrays.
 */
export interface TerminalSubscriptionSource {
  subscribe(
    onChunk: (chunk: TerminalBytes) => void,
    options: TerminalSourceOptions,
  ): TerminalSubscription | (() => void) | void;
}

export type TerminalSource = TerminalSubscriptionSource | ReadableStream<TerminalBytes> | AsyncIterable<TerminalBytes>;

export interface TerminalObjectSink {
  write(chunk: TerminalBytes): void | Promise<void>;
  close?(): void | Promise<void>;
}

export type TerminalSink =
  TerminalObjectSink | WritableStream<TerminalBytes> | ((chunk: TerminalBytes) => void | Promise<void>);

export interface TerminalElementEventMap {
  "terminal-input": CustomEvent<TerminalBytes>;
  "terminal-output": CustomEvent<TerminalBytes>;
  "terminal-error": CustomEvent<TerminalError>;
  "terminal-complete": Event;
}

export class TerminalError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = "TerminalError";
    this.code = code;
  }
}

export class TerminalBoundsError extends TerminalError {
  readonly limit: number;
  readonly actual: number;

  constructor(kind: "input" | "output", limit: number, actual: number) {
    super("bounds", `${kind} chunk exceeds the configured ${kind} byte limit`);
    this.name = "TerminalBoundsError";
    this.limit = limit;
    this.actual = actual;
  }
}

export class TerminalBackpressureError extends TerminalError {
  readonly limit: number;

  constructor(limit: number) {
    super("backpressure", "terminal input is waiting for the sink to catch up");
    this.name = "TerminalBackpressureError";
    this.limit = limit;
  }
}

export class TerminalDisconnectedError extends TerminalError {
  constructor() {
    super("disconnected", "terminal input was cancelled because the element is disconnected");
    this.name = "TerminalDisconnectedError";
  }
}

export type TerminalEventName = keyof TerminalElementEventMap;
