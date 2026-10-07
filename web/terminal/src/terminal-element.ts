// SPDX-License-Identifier: Apache-2.0
/* eslint-disable @typescript-eslint/no-invalid-void-type */
import {
  TerminalBackpressureError,
  TerminalBoundsError,
  TerminalDisconnectedError,
  TerminalError,
  type TerminalBytes,
  type TerminalElementEventMap,
  type TerminalObjectSink,
  type TerminalSink,
  type TerminalSource,
  type TerminalSubscription,
} from "./types";

export const TERMINAL_TAG_NAME = "scoplen-terminal";
export const DEFAULT_MAX_OUTPUT_BYTES = 1024 * 1024;
export const DEFAULT_MAX_INPUT_BYTES = 64 * 1024;
export const DEFAULT_MAX_PENDING_WRITES = 32;

// eslint-disable-next-line @typescript-eslint/no-extraneous-class
const HTMLElementBase = (typeof HTMLElement === "undefined" ? class {} : HTMLElement) as typeof HTMLElement;

function isBytes(value: unknown): value is TerminalBytes {
  return value instanceof Uint8Array || Object.prototype.toString.call(value) === "[object Uint8Array]";
}

function cloneBytes(value: TerminalBytes): TerminalBytes {
  return new Uint8Array(value);
}

function normalizeUnsubscribe(value: TerminalSubscription | (() => void) | void): () => void {
  if (typeof value === "function") {
    return value;
  }
  if (value && typeof value.unsubscribe === "function") {
    return () => value.unsubscribe();
  }
  return () => undefined;
}

function positiveLimit(value: number, name: string): number {
  if (!Number.isSafeInteger(value) || value < 1) {
    throw new RangeError(`${name} must be a positive safe integer`);
  }
  return value;
}

function isReadableStream(source: TerminalSource): source is ReadableStream<TerminalBytes> {
  return typeof ReadableStream !== "undefined" && source instanceof ReadableStream;
}

function isAsyncIterable(source: TerminalSource): source is AsyncIterable<TerminalBytes> {
  return typeof (source as AsyncIterable<TerminalBytes>)[Symbol.asyncIterator] === "function";
}

function isWritableStream(sink: TerminalSink): sink is WritableStream<TerminalBytes> {
  return typeof WritableStream !== "undefined" && sink instanceof WritableStream;
}

function isObjectSink(sink: TerminalSink): sink is TerminalObjectSink {
  return typeof sink === "object" && sink !== null && "write" in sink && typeof sink.write === "function";
}

/**
 * A small binary terminal surface. It deliberately renders decoded text only;
 * terminal emulation and xterm.js integrations remain separate work.
 */
export class ScoplenTerminalElement extends HTMLElementBase {
  private _source: TerminalSource | null = null;
  private _sink: TerminalSink | null = null;
  private _maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES;
  private _maxInputBytes = DEFAULT_MAX_INPUT_BYTES;
  private _maxPendingWrites = DEFAULT_MAX_PENDING_WRITES;
  private outputBytes = new Uint8Array();
  private outputNode: HTMLElement | null = null;
  private sourceAbort: AbortController | null = null;
  private sourceUnsubscribe: (() => void) | null = null;
  private sinkWriter: WritableStreamDefaultWriter<TerminalBytes> | null = null;
  private writeTail: Promise<void> = Promise.resolve();
  private pendingWrites = 0;
  private lifecycleGeneration = 0;
  private connected = false;
  private listenersAttached = false;

  constructor() {
    super();
    if (typeof this.attachShadow === "function") {
      const shadow = this.attachShadow({ mode: "open" });
      const style = document.createElement("style");
      style.textContent =
        ":host{display:block;contain:content;overflow:auto;background:#111;color:#f3f4f6;font:14px/1.45 ui-monospace,SFMono-Regular,Consolas,monospace}pre{margin:0;min-height:1.45em;white-space:pre-wrap;overflow-wrap:anywhere;padding:0.75rem;outline:none}";
      this.outputNode = document.createElement("pre");
      this.outputNode.setAttribute("part", "output");
      this.outputNode.setAttribute("role", "log");
      this.outputNode.setAttribute("aria-live", "polite");
      shadow.append(style, this.outputNode);
    }
  }

  get source(): TerminalSource | null {
    return this._source;
  }

  set source(source: TerminalSource | null) {
    this._source = source;
    if (this.connected) {
      this.attachSource();
    }
  }

  get sink(): TerminalSink | null {
    return this._sink;
  }

  set sink(sink: TerminalSink | null) {
    this.releaseSinkWriter();
    this._sink = sink;
  }

  get maxOutputBytes(): number {
    return this._maxOutputBytes;
  }

  set maxOutputBytes(value: number) {
    this._maxOutputBytes = positiveLimit(value, "maxOutputBytes");
    if (this.outputBytes.byteLength > this._maxOutputBytes) {
      this.outputBytes = this.outputBytes.slice(-this._maxOutputBytes);
      this.renderOutput();
    }
  }

  get maxInputBytes(): number {
    return this._maxInputBytes;
  }

  set maxInputBytes(value: number) {
    this._maxInputBytes = positiveLimit(value, "maxInputBytes");
  }

  get maxPendingWrites(): number {
    return this._maxPendingWrites;
  }

  set maxPendingWrites(value: number) {
    this._maxPendingWrites = positiveLimit(value, "maxPendingWrites");
  }

  connectedCallback(): void {
    if (this.connected) {
      return;
    }
    this.connected = true;
    this.lifecycleGeneration += 1;
    this.setAttribute("role", "application");
    this.tabIndex = 0;
    if (!this.listenersAttached) {
      this.addEventListener("keydown", this.onKeyDown);
      this.listenersAttached = true;
    }
    this.attachSource();
  }

  disconnectedCallback(): void {
    if (!this.connected) {
      return;
    }
    this.connected = false;
    this.lifecycleGeneration += 1;
    this.detachSource();
    // A transport write cannot be interrupted portably. Every queued write
    // checks the generation before touching the sink and rejects after detach.
    this.writeTail = this.writeTail.catch(() => undefined);
  }

  /** Stop the source, clear rendered bytes, and optionally close an owned sink. */
  async dispose(options: { closeSink?: boolean } = {}): Promise<void> {
    this.detachSource();
    this.clearOutput();
    if (options.closeSink) {
      await this.closeSink();
    }
  }

  async closeSink(): Promise<void> {
    const sink = this._sink;
    if (sink === null) {
      return;
    }
    if (isWritableStream(sink)) {
      const writer = this.getSinkWriter(sink);
      await writer.close();
      return;
    }
    if (isObjectSink(sink) && sink.close) {
      await sink.close();
    }
  }

  /** Send one binary input frame to the configured sink. */
  send(chunk: TerminalBytes): Promise<void> {
    if (!isBytes(chunk)) {
      return Promise.reject(new TypeError("terminal input must be a Uint8Array"));
    }
    if (chunk.byteLength > this._maxInputBytes) {
      const error = new TerminalBoundsError("input", this._maxInputBytes, chunk.byteLength);
      this.reportError(error);
      return Promise.reject(error);
    }
    if (this.pendingWrites >= this._maxPendingWrites) {
      const error = new TerminalBackpressureError(this._maxPendingWrites);
      this.reportError(error);
      return Promise.reject(error);
    }

    const bytes = cloneBytes(chunk);
    const generation = this.lifecycleGeneration;
    this.pendingWrites += 1;
    const task = this.writeTail.then(async () => {
      if (!this.connected || generation !== this.lifecycleGeneration) {
        throw new TerminalDisconnectedError();
      }
      const sink = this._sink;
      if (sink === null) {
        this.dispatchInput(bytes);
      } else if (typeof sink === "function") {
        await sink(bytes);
      } else if (isWritableStream(sink)) {
        await this.getSinkWriter(sink).write(bytes);
      } else {
        await sink.write(bytes);
      }
    });
    this.writeTail = task.catch(() => undefined);
    return task.finally(() => {
      this.pendingWrites -= 1;
    });
  }

  /** Return the bounded raw output retained by this element. */
  getOutput(): TerminalBytes {
    return cloneBytes(this.outputBytes);
  }

  clearOutput(): void {
    this.outputBytes = new Uint8Array();
    this.renderOutput();
  }

  private attachSource(): void {
    this.detachSource();
    const source = this._source;
    if (!source || !this.connected) {
      return;
    }
    const abort = new AbortController();
    this.sourceAbort = abort;
    try {
      if (isReadableStream(source)) {
        const reader = source.getReader();
        this.sourceUnsubscribe = () => {
          abort.abort();
          void reader.cancel();
          reader.releaseLock();
        };
        void this.consumeReader(reader, abort.signal);
      } else if (isAsyncIterable(source)) {
        const iterator = source[Symbol.asyncIterator]();
        this.sourceUnsubscribe = () => {
          abort.abort();
          if (iterator.return) {
            void iterator.return();
          }
        };
        void this.consumeIterator(iterator, abort.signal);
      } else {
        this.sourceUnsubscribe = normalizeUnsubscribe(
          source.subscribe((chunk) => this.receiveOutput(chunk), { signal: abort.signal }),
        );
      }
    } catch (error) {
      this.reportError(error);
      this.detachSource();
    }
  }

  private detachSource(): void {
    this.sourceAbort?.abort();
    this.sourceAbort = null;
    const unsubscribe = this.sourceUnsubscribe;
    this.sourceUnsubscribe = null;
    if (unsubscribe) {
      try {
        unsubscribe();
      } catch (error) {
        this.reportError(error);
      }
    }
  }

  private async consumeReader(reader: ReadableStreamDefaultReader<TerminalBytes>, signal: AbortSignal): Promise<void> {
    try {
      while (!signal.aborted) {
        const result = await reader.read();
        if (result.done) {
          this.dispatchEvent(new Event("terminal-complete"));
          break;
        }
        this.receiveOutput(result.value);
      }
    } catch (error) {
      if (!signal.aborted) {
        this.reportError(error);
      }
    } finally {
      reader.releaseLock();
    }
  }

  private async consumeIterator(iterator: AsyncIterator<TerminalBytes>, signal: AbortSignal): Promise<void> {
    try {
      while (!signal.aborted) {
        const result = await iterator.next();
        if (result.done) {
          this.dispatchEvent(new Event("terminal-complete"));
          break;
        }
        this.receiveOutput(result.value);
      }
    } catch (error) {
      if (!signal.aborted) {
        this.reportError(error);
      }
    }
  }

  private receiveOutput(chunk: TerminalBytes): void {
    if (!this.connected) {
      return;
    }
    if (!isBytes(chunk)) {
      this.reportError(new TypeError("terminal output must be a Uint8Array"));
      return;
    }
    const output = cloneBytes(chunk);
    this.dispatchEvent(new CustomEvent<TerminalBytes>("terminal-output", { detail: cloneBytes(output) }));
    const actualOutputBytes = this.outputBytes.byteLength + output.byteLength;
    const exceeded = actualOutputBytes > this._maxOutputBytes;
    if (output.byteLength >= this._maxOutputBytes) {
      this.outputBytes = output.slice(-this._maxOutputBytes);
    } else {
      const merged = new Uint8Array(this.outputBytes.byteLength + output.byteLength);
      merged.set(this.outputBytes);
      merged.set(output, this.outputBytes.byteLength);
      this.outputBytes = merged.byteLength > this._maxOutputBytes ? merged.slice(-this._maxOutputBytes) : merged;
    }
    if (exceeded) {
      this.reportError(new TerminalBoundsError("output", this._maxOutputBytes, actualOutputBytes));
    }
    this.renderOutput();
  }

  private renderOutput(): void {
    if (!this.outputNode) {
      return;
    }
    this.outputNode.textContent = new TextDecoder().decode(this.outputBytes);
  }

  private dispatchInput(bytes: TerminalBytes): void {
    this.dispatchEvent(new CustomEvent<TerminalBytes>("terminal-input", { detail: cloneBytes(bytes) }));
  }

  private reportError(error: unknown): void {
    const normalized = error instanceof TerminalError ? error : new TerminalError("stream", String(error));
    this.dispatchEvent(new CustomEvent<TerminalError>("terminal-error", { detail: normalized }));
  }

  private getSinkWriter(stream: WritableStream<TerminalBytes>): WritableStreamDefaultWriter<TerminalBytes> {
    return (this.sinkWriter ??= stream.getWriter());
  }

  private releaseSinkWriter(): void {
    this.sinkWriter?.releaseLock();
    this.sinkWriter = null;
  }

  private readonly onKeyDown = (event: KeyboardEvent): void => {
    if (event.isComposing || event.metaKey) {
      return;
    }
    const bytes = keyBytes(event);
    if (!bytes) {
      return;
    }
    event.preventDefault();
    void this.send(bytes).catch(() => undefined);
  };
}

function keyBytes(event: KeyboardEvent): TerminalBytes | null {
  const named: Record<string, string> = {
    Enter: "\r",
    Backspace: "\x7f",
    Tab: "\t",
    Escape: "\x1b",
    ArrowUp: "\x1b[A",
    ArrowDown: "\x1b[B",
    ArrowRight: "\x1b[C",
    ArrowLeft: "\x1b[D",
    Home: "\x1b[H",
    End: "\x1b[F",
    Delete: "\x1b[3~",
    PageUp: "\x1b[5~",
    PageDown: "\x1b[6~",
  };
  let value = named[event.key] ?? (event.key.length === 1 && !event.ctrlKey && !event.altKey ? event.key : null);
  if (event.ctrlKey && event.key.length === 1) {
    const code = event.key.toUpperCase().charCodeAt(0);
    if (code >= 64 && code <= 95) {
      value = String.fromCharCode(code & 31);
    }
  }
  if (event.altKey && event.key.length === 1) {
    value = `\x1b${event.key}`;
  }
  return value === null ? null : new TextEncoder().encode(value);
}

export function defineTerminalElement(tagName = TERMINAL_TAG_NAME): typeof ScoplenTerminalElement | undefined {
  if (typeof customElements === "undefined") {
    return undefined;
  }
  const existing = customElements.get(tagName);
  if (existing) {
    return existing as typeof ScoplenTerminalElement;
  }
  customElements.define(tagName, ScoplenTerminalElement);
  return ScoplenTerminalElement;
}

if (typeof customElements !== "undefined" && !customElements.get(TERMINAL_TAG_NAME)) {
  customElements.define(TERMINAL_TAG_NAME, ScoplenTerminalElement);
}

declare global {
  // eslint-disable-next-line @typescript-eslint/no-empty-object-type
  interface HTMLElementEventMap extends TerminalElementEventMap {}
}
