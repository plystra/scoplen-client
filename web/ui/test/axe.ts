// SPDX-License-Identifier: Apache-2.0
import axe from "axe-core";

/** Runs axe on a rendered container and returns the violations. */
export async function violations(container: Element) {
  const result = await axe.run(container, {
    // jsdom does not lay out or paint, so contrast is checked from the tokens
    // instead (tokens.test.ts).
    rules: { "color-contrast": { enabled: false } },
  });
  return result.violations.map((v) => `${v.id}: ${v.help}`);
}
