# @scoplen/terminal

`@scoplen/terminal` is the byte boundary shared by the Scoplen desktop client and
the server web console. It has no Tauri, IPC, client store, network, or
cryptography dependency.

The package currently provides a small framework-agnostic custom element and a
React binding. It keeps output as bounded `Uint8Array` data, decodes that data
for a plain text preview, and sends keyboard bytes to a caller-owned sink.

## Direct custom element

```ts
import { defineTerminalElement, type ScoplenTerminalElement } from "@scoplen/terminal";

defineTerminalElement();
const terminal = document.querySelector("scoplen-terminal") as ScoplenTerminalElement;

terminal.source = {
  subscribe(onChunk, { signal }) {
    const stop = transport.onBytes(onChunk);
    signal.addEventListener("abort", stop, { once: true });
    return stop;
  },
};
terminal.sink = {
  async write(bytes) {
    await transport.send(bytes);
  },
};
```

`source` accepts a subscription object, `ReadableStream<Uint8Array>`, or
`AsyncIterable<Uint8Array>`. A subscription returns `unsubscribe()` or a
cleanup function and receives an `AbortSignal`. `sink` accepts a `write`
object, `WritableStream<Uint8Array>`, or a function. `send()` serializes sink
writes and returns a promise; `maxPendingWrites` rejects new input with a
`TerminalBackpressureError` when the sink cannot keep up.

The element emits `terminal-output`, `terminal-input`, `terminal-error`, and
`terminal-complete` events. Event `detail` values are `Uint8Array` (or a
`TerminalError` for `terminal-error`). The element unsubscribes its source and
rejects queued writes when disconnected. It does not close a caller-owned sink
on disconnect; call `closeSink()` or `dispose({ closeSink: true })` when that
ownership is explicit.

Output is retained as the most recent `maxOutputBytes` bytes (1 MiB by
default); input frames are limited to 64 KiB and pending sink writes to 32.
These limits make a stalled or untrusted transport observable instead of
allowing unbounded memory growth. The preview is plain text and is not a
terminal emulator.

## React

```tsx
import { Terminal } from "@scoplen/terminal/react";

<Terminal
  source={source}
  sink={sink}
  onTerminalOutput={(bytes) => recordOutput(bytes)}
  onTerminalError={(error) => report(error)}
/>
```

The binding only assigns element properties and subscribes to element events.
It does not know about Tauri or any Scoplen client state, and removes its event
listeners when the component unmounts.

## Explicitly pending terminal work

The C4 package boundary does not yet claim xterm.js emulation, WebGL rendering,
Unicode 15 width handling, search, serialization, SIXEL/iTerm2 images, IME
composition, broadcast input, profiles, `vttest`/`esctest`, or compatibility
coverage for full-screen programs. Those capabilities remain later C4 work and
are not hidden behind this plain preview.

## Development

```bash
pnpm --dir web/terminal typecheck
pnpm --dir web/terminal test
pnpm --dir web/terminal build
```
