#!/usr/bin/env node
/**
 * Remediation 04 test entry.
 *
 * The shared frontend test entry from remediation-01 has not landed yet
 * (package.json is owned by 01); this runner uses esbuild — already present
 * in node_modules via vite — to bundle each *.test.ts and executes it with
 * node's built-in test runner. It adds no dependencies and no lockfile
 * changes. Once 01 lands a real test entry, these files should move under
 * it; the prerequisite is recorded in the evidence document.
 *
 * Usage: node tests/remediation-04/run.mjs
 */
import { build } from "esbuild";
import { mkdtemp, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";

const here = dirname(fileURLToPath(import.meta.url));
const files = (await readdir(here)).filter((f) => f.endsWith(".test.ts")).sort();
if (files.length === 0) {
  console.error("no test files found");
  process.exit(1);
}

const outDir = await mkdtemp(join(tmpdir(), "pdf-seeker-remediation-04-"));
let failed = 0;
try {
  for (const f of files) {
    const outfile = join(outDir, f.replace(/\.test\.ts$/, ".mjs"));
    await build({
      entryPoints: [join(here, f)],
      outfile,
      bundle: true,
      format: "esm",
      platform: "node",
      sourcemap: "inline",
      logLevel: "silent",
    });
    const res = spawnSync(process.execPath, [outfile], { stdio: "inherit" });
    if (res.status !== 0) {
      failed++;
      console.error(`FAIL ${f}`);
    } else {
      console.error(`PASS ${f}`);
    }
  }
} finally {
  await rm(outDir, { recursive: true, force: true });
}
process.exit(failed ? 1 : 0);
