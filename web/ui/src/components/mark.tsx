// SPDX-License-Identifier: Apache-2.0
import geometry from "../brand/geometry.json";

export type MarkVariant = "display" | "small";
type Variant = MarkVariant;

/** The two corner forms of the mark as one SVG path, in mark units. */
export function markPath(variant: Variant): string {
  const { start: a, end: b, length: l, vertical: v, horizontal: h, radius: r } = geometry[variant];
  const topLeft = `M${a + r} ${a}H${a + l}V${a + h}H${a + v}V${a + l}H${a}V${a + r}A${r} ${r} 0 0 1 ${a + r} ${a}Z`;
  const bottomRight = `M${b - r} ${b}H${b - l}V${b - h}H${b - v}V${b - l}H${b}V${b - r}A${r} ${r} 0 0 1 ${b - r} ${b}Z`;
  return topLeft + bottomRight;
}

export interface MarkProps {
  /** `display` at 48px and larger, `small` below (`scoplen-docs/18-visual-identity.md`). */
  variant?: MarkVariant;
  /** Rendered height and width in CSS units. */
  size?: string;
  /** Accessible name; omit when the mark is decorative next to the name. */
  title?: string;
  className?: string;
}

/**
 * The Scoplen mark, cropped to its field so that `size` is the visible
 * height. Always one color, taken from the current text color.
 */
export function Mark({ variant = "small", size = "1em", title, className }: MarkProps) {
  const { start, end } = geometry[variant];
  return (
    <svg
      viewBox={`${start} ${start} ${end - start} ${end - start}`}
      width={size}
      height={size}
      className={className}
      fill="currentColor"
      role={title ? "img" : undefined}
      aria-hidden={title ? undefined : true}
      aria-label={title}
      focusable="false"
    >
      <path d={markPath(variant)} />
    </svg>
  );
}
