// SPDX-License-Identifier: Apache-2.0
import { Mark } from "./mark";

export interface LockupProps {
  /** Mark variant for the rendered size; `display` when the mark is 48px or larger. */
  variant?: "display" | "small";
  /** Font size of the wordmark; the mark is sized from it. */
  size?: string;
  className?: string;
}

/**
 * The mark beside the wordmark "Scoplen" in Lora 500. The mark's height is
 * the wordmark's cap height plus one third, and the gap is one third of the
 * mark's height (`scoplen-docs/18-visual-identity.md`). Lora's cap height is
 * 0.7 em, so the mark is 0.933 em and the gap 0.311 em.
 */
export function Lockup({ variant = "small", size = "1.5rem", className }: LockupProps) {
  return (
    <span
      className={["inline-flex items-center font-serif font-medium text-foreground", className]
        .filter(Boolean)
        .join(" ")}
      style={{ fontSize: size, gap: "0.311em", lineHeight: 1 }}
    >
      <Mark variant={variant} size="0.933em" />
      <span lang="en">Scoplen</span>
    </span>
  );
}
