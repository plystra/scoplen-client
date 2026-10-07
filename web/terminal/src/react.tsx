// SPDX-License-Identifier: Apache-2.0
import { forwardRef, useImperativeHandle, useLayoutEffect, useRef, type HTMLAttributes, type Ref } from "react";
import {
  DEFAULT_MAX_INPUT_BYTES,
  DEFAULT_MAX_OUTPUT_BYTES,
  DEFAULT_MAX_PENDING_WRITES,
  ScoplenTerminalElement,
  defineTerminalElement,
} from "./terminal-element";
import type { TerminalBytes, TerminalError, TerminalSink, TerminalSource } from "./types";

export interface TerminalProps extends Omit<HTMLAttributes<ScoplenTerminalElement>, "onError" | "onInput"> {
  source?: TerminalSource | null;
  sink?: TerminalSink | null;
  maxOutputBytes?: number;
  maxInputBytes?: number;
  maxPendingWrites?: number;
  onTerminalInput?: (bytes: TerminalBytes) => void;
  onTerminalOutput?: (bytes: TerminalBytes) => void;
  onTerminalError?: (error: TerminalError) => void;
  onTerminalComplete?: () => void;
}

function assignRef<T>(ref: Ref<T> | undefined, value: T | null): void {
  if (typeof ref === "function") {
    ref(value);
  } else if (ref) {
    ref.current = value;
  }
}

/** React binding for the binary custom element; it contains no IPC or client state. */
export const Terminal = forwardRef<ScoplenTerminalElement, TerminalProps>(function Terminal(
  {
    source = null,
    sink = null,
    maxOutputBytes = DEFAULT_MAX_OUTPUT_BYTES,
    maxInputBytes = DEFAULT_MAX_INPUT_BYTES,
    maxPendingWrites = DEFAULT_MAX_PENDING_WRITES,
    onTerminalInput,
    onTerminalOutput,
    onTerminalError,
    onTerminalComplete,
    children,
    ...attributes
  },
  forwardedRef,
) {
  const elementRef = useRef<ScoplenTerminalElement | null>(null);
  useImperativeHandle(forwardedRef, () => elementRef.current as ScoplenTerminalElement, []);

  useLayoutEffect(() => {
    const element = elementRef.current;
    if (!element) {
      return;
    }
    const input = (event: Event) => onTerminalInput?.((event as CustomEvent<TerminalBytes>).detail.slice());
    const output = (event: Event) => onTerminalOutput?.((event as CustomEvent<TerminalBytes>).detail.slice());
    const error = (event: Event) => onTerminalError?.((event as CustomEvent<TerminalError>).detail);
    const complete = () => onTerminalComplete?.();
    element.addEventListener("terminal-input", input);
    element.addEventListener("terminal-output", output);
    element.addEventListener("terminal-error", error);
    element.addEventListener("terminal-complete", complete);
    return () => {
      element.removeEventListener("terminal-input", input);
      element.removeEventListener("terminal-output", output);
      element.removeEventListener("terminal-error", error);
      element.removeEventListener("terminal-complete", complete);
    };
  }, [onTerminalInput, onTerminalOutput, onTerminalError, onTerminalComplete]);

  useLayoutEffect(() => {
    defineTerminalElement();
    const element = elementRef.current;
    if (!element) {
      return;
    }
    element.source = source;
    element.sink = sink;
    element.maxOutputBytes = maxOutputBytes;
    element.maxInputBytes = maxInputBytes;
    element.maxPendingWrites = maxPendingWrites;
  }, [source, sink, maxOutputBytes, maxInputBytes, maxPendingWrites]);

  return (
    <scoplen-terminal
      {...attributes}
      ref={(element: ScoplenTerminalElement | null) => {
        elementRef.current = element;
        assignRef(forwardedRef, element);
      }}
    >
      {children}
    </scoplen-terminal>
  );
});

declare module "react" {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace JSX {
    interface IntrinsicElements {
      "scoplen-terminal": import("react").HTMLAttributes<ScoplenTerminalElement> &
        import("react").RefAttributes<ScoplenTerminalElement>;
    }
  }
}

export type { TerminalBytes, TerminalError, TerminalSink, TerminalSource };
