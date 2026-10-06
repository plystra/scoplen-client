// SPDX-License-Identifier: Apache-2.0
// @vitest-environment node
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const read = (path: string) => readFileSync(fileURLToPath(new URL(path, import.meta.url)), "utf8");

describe("Chinese heading font", () => {
  it("covers every Chinese character in the catalog; run `pnpm fonts` after editing it", () => {
    const manifest = JSON.parse(read("../../ui/src/fonts/scoplen-serif-sc.json")) as { characters: string };
    const covered = new Set(manifest.characters);
    const missing = [...new Set(read("../src/messages/zh-Hans.ts"))].filter(
      (c) => c.codePointAt(0)! >= 0x2e80 && !covered.has(c),
    );
    expect(missing.join("")).toBe("");
  });
});
