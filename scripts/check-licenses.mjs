// SPDX-License-Identifier: Apache-2.0
// Fails when a dependency shipped in the application carries a license that
// is not compatible with Apache-2.0 distribution (`scoplen-docs/02-workspace-and-repositories.md` §5).
// Fonts are OFL-1.1, which permits bundling with software.
import { execFileSync } from "node:child_process";

const allowed = new Set(["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "0BSD", "OFL-1.1"]);

const satisfies = (expression) =>
  expression
    .replace(/[()]/g, "")
    .split(/\s+OR\s+/)
    .some((alternative) => alternative.split(/\s+AND\s+/).every((id) => allowed.has(id.trim())));

const pnpmArgs = ["licenses", "list", "--json", "--prod"];
const command =
  process.platform === "win32"
    ? [process.env.ComSpec ?? "cmd.exe", ["/d", "/s", "/c", `pnpm.cmd ${pnpmArgs.join(" ")}`]]
    : ["pnpm", pnpmArgs];
const report = JSON.parse(execFileSync(command[0], command[1], { encoding: "utf8" }));
const rejected = Object.entries(report)
  .filter(([license]) => !satisfies(license))
  .flatMap(([license, packages]) => packages.map((p) => `${p.name}@${p.versions.join(",")}: ${license}`));

if (rejected.length > 0) {
  console.error(`Dependencies with licenses not allowed in the client:\n${rejected.join("\n")}`);
  process.exit(1);
}
console.log(`${Object.values(report).flat().length} shipped packages, all with allowed licenses`);
