// Executed by the Rust integration suite; no product code is altered.
import assert from "node:assert/strict";
import { writeFileSync, unlinkSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { ESLint } from "../../desktop/node_modules/eslint/lib/api.js";

const root = fileURLToPath(new URL("../../", import.meta.url));

if (process.argv[2] === "licensing") {
  const { satisfies } = await import("../../scripts/check-npm-licenses.mjs");
  assert.equal(
    satisfies("MIT OR GPL-3.0-only"),
    true,
    "positive control: permitted dual licence",
  );
  assert.equal(
    satisfies("MIT AND GPL-3.0-only"),
    false,
    "negative control: mandatory GPL",
  );
  const expression = "(MIT OR Apache-2.0) AND (GPL-3.0-only OR AGPL-3.0-only)";
  assert.equal(
    satisfies(expression),
    false,
    `C01-R1-01: ${expression} requires a forbidden licence`,
  );
} else if (process.argv[2] === "allowlist") {
  // Amended by the builder in round 1 per the orchestrator's C01-R1-01 ruling (2026-10-17): 0BSD is approved, and
  // there must be ONE allow-list for both ecosystems. The original assertion (0BSD rejected) is replaced by a
  // drift check: the JS gate's list must equal deny.toml's [licenses] allow list, must equal the approved set
  // recorded in ADR-0003, and must reject licences outside it.
  const { readFileSync } = await import("node:fs");
  const { ALLOWED, satisfies } = await import("../../scripts/check-npm-licenses.mjs");
  const deny = readFileSync(new URL("../../deny.toml", import.meta.url), "utf8");
  const table = /^\[licenses\][\s\S]*?^allow\s*=\s*\[([\s\S]*?)\]/m.exec(deny);
  assert(table, "deny.toml must have a [licenses] allow list");
  const rust = [...table[1].replace(/#.*$/gm, "").matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  // Amended again by the builder in M1 C1.1 round 1 per the orchestrator's C11-R1-18 ruling (2026-10-18): MIT-0 is
  // approved (borrow-or-share, needed by current jsonschema releases), so it moves from the rejected controls below
  // into the approved set; every other control is unchanged.
  const approved = ["MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0", "Unicode-3.0", "0BSD", "MIT-0"];
  assert.deepEqual([...ALLOWED].sort(), [...rust].sort(), "C01-R1-01: JS and Rust allow-lists drifted");
  assert.deepEqual([...rust].sort(), [...approved].sort(), "C01-R1-01: allow-list differs from the approved set");
  const adr = readFileSync(new URL("../../docs/decisions/ADR-0003-licensing-policy.md", import.meta.url), "utf8");
  for (const id of approved) assert(adr.includes(id), `ADR-0003 must list ${id}`);
  for (const outside of ["Unlicense", "CC0-1.0", "BlueOak-1.0.0", "LGPL-2.1-only", "GPL-3.0-only", "AGPL-3.0-only"]) {
    assert.equal(satisfies(outside), false, `C01-R1-01: ${outside} is outside the approved allow-list`);
  }
} else if (process.argv[2] === "imports") {
  const desktop = fileURLToPath(new URL("../../desktop/", import.meta.url));
  const probe = fileURLToPath(
    new URL("../../desktop/src/review-boundary-probe.ts", import.meta.url),
  );
  const eslint = new ESLint({ cwd: desktop });
  const check = async (text) => {
    writeFileSync(probe, text);
    return (await eslint.lintFiles([probe])).flatMap(
      (result) => result.messages,
    );
  };
  try {
    const direct = await check(
      "import { invoke } from '@tauri-apps/api/core';\nexport const bypass = () => invoke<unknown>('app_info');\n",
    );
    assert(
      direct.some((message) => message.ruleId === "no-restricted-imports"),
      "static import control must fail",
    );
    const dynamic = await check(
      "export async function bypass(): Promise<unknown> {\n  const { invoke } = await import('@tauri-apps/api/core');\n  return invoke<unknown>('app_info');\n}\n",
    );
    assert(
      dynamic.some(
        (message) =>
          message.ruleId?.includes("restricted") ||
          message.message.includes("src/ipc"),
      ),
      `C01-R1-02: dynamic import bypass has zero lint diagnostics (root ${root})`,
    );
  } finally {
    unlinkSync(probe);
  }
} else {
  throw new Error("expected licensing, allowlist, or imports");
}
