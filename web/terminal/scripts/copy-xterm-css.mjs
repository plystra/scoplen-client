// SPDX-License-Identifier: Apache-2.0
import { copyFileSync } from "node:fs";
import { createRequire } from "node:module";
import { URL, fileURLToPath } from "node:url";

const require = createRequire(import.meta.url);
const source = require.resolve("@xterm/xterm/css/xterm.css");
const destination = fileURLToPath(new URL("../dist/xterm.css", import.meta.url));
copyFileSync(source, destination);
