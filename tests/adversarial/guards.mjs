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
  const { satisfies } = await import("../../scripts/check-npm-licenses.mjs");
  assert.equal(
    satisfies("0BSD"),
    false,
    "C01-R1-01: 0BSD is outside the approved allow-list",
  );
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
