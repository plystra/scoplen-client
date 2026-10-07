// SPDX-License-Identifier: Apache-2.0

/** Receives text only after an IME composition has been committed. */
export type TerminalImeCommit = (text: string) => void;

/**
 * Keeps composition text out of the terminal sink until the IME commits it.
 *
 * This class deliberately has no DOM dependency. Hosts can feed it browser
 * composition events, native-platform adapters, or deterministic tests. A
 * blur/cancel clears the pending composition so a late compositionend cannot
 * write text after focus or ownership has moved elsewhere.
 */
export class TerminalImeCompositionBridge {
  private composing = false;

  public constructor(private readonly commit: TerminalImeCommit) {}

  public get isComposing(): boolean {
    return this.composing;
  }

  /** Begin a new composition. Any previous pending composition is discarded. */
  public start(data = ""): void {
    void data;
    this.composing = true;
  }

  /** Observe an in-progress composition without sending it to the sink. */
  public update(data: string): void {
    void data;
    // Composition updates are intentionally not retained or emitted. The
    // compositionend event is the only source of committed terminal input.
  }

  /** Commit the final composition text, if the composition is still active. */
  public end(data: string): boolean {
    if (!this.composing) {
      return false;
    }
    this.composing = false;
    if (data.length === 0) {
      return false;
    }
    this.commit(data);
    return true;
  }

  /** Cancel a composition without writing its intermediate text. */
  public cancel(): boolean {
    const wasComposing = this.composing;
    this.composing = false;
    return wasComposing;
  }

  /** Treat focus loss as cancellation to prevent a late commit. */
  public blur(): boolean {
    return this.cancel();
  }
}
