// SPDX-License-Identifier: Apache-2.0
import type { ReactNode } from "react";

/** Content for assistive technology only. */
export function VisuallyHidden({ children }: { children: ReactNode }) {
  return <span className="sr-only">{children}</span>;
}

/** A link to the main content, the first stop for keyboard users. */
export function SkipLink({ target, children }: { target: string; children: ReactNode }) {
  return (
    <a
      href={`#${target}`}
      className="sr-only rounded-md border border-border-strong bg-raised text-sm text-foreground focus:not-sr-only focus:fixed focus:top-2 focus:left-2 focus:z-50 focus:px-3 focus:py-2"
    >
      {children}
    </a>
  );
}
