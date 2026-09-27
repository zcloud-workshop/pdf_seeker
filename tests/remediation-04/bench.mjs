#!/usr/bin/env node
/**
 * Remediation 04 — diff budget/performance record.
 *
 * Prints measured input sizes, step counts and wall time for the diff core
 * on long and extremely unbalanced inputs. The numbers feed the evidence
 * document (docs/remediation/evidence/04-comparison.md); they are
 * measurements on one machine, not promises.
 *
 * Usage: node tests/remediation-04/bench.mjs
 */
import { build } from "esbuild";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = await mkdtemp(join(tmpdir(), "pdf-seeker-bench-04-"));
try {
  const outfile = join(outDir, "core.mjs");
  await build({
    entryPoints: [join(here, "../../src/lib/comparison/diff-core.ts")],
    outfile,
    bundle: true,
    format: "esm",
    platform: "node",
    logLevel: "silent",
  });
  const { diffLinesCounted, DiffBudgetError } = await import(
    `file://${outfile}`
  );

  const line = (i) => `line ${i}: the quick brown fox jumps over the lazy dog`;
  const scenarios = [
    [
      "identical 20k lines",
      () => {
        const a = Array.from({ length: 20_000 }, (_, i) => line(i));
        return [a, a.map((x) => x)];
      },
      8_000_000,
    ],
    [
      "20k lines, one changed line",
      () => {
        const a = Array.from({ length: 20_000 }, (_, i) => line(i));
        const b = [...a];
        b[10_000] = "CHANGED";
        return [a, b];
      },
      8_000_000,
    ],
    [
      "5k vs 5k fully different lines",
      () => {
        const a = Array.from({ length: 5_000 }, (_, i) => `a-${i}-alpha`);
        const b = Array.from({ length: 5_000 }, (_, i) => `b-${i}-beta`);
        return [a, b];
      },
      8_000_000,
    ],
    [
      "60k vs 3 lines (extremely unbalanced)",
      () => {
        const a = Array.from({ length: 60_000 }, (_, i) => `only-a-${i}`);
        return [a, ["x", "y", "z"]];
      },
      8_000_000,
    ],
    [
      "interleaved random 2k lines (3-symbol alphabet)",
      () => {
        let t = 99;
        const rnd = () => ((t = (t * 1103515245 + 12345) >>> 0) / 4294967296);
        const mk = (p) =>
          Array.from({ length: 2_000 }, () =>
            ["l0", "l1", "l2"][Math.floor(rnd() * 3)] + (rnd() < 0.02 ? p : ""),
          );
        return [mk("a"), mk("b")];
      },
      8_000_000,
    ],
  ];

  console.log("scenario | linesA | linesB | steps | ms (single run)");
  for (const [name, make, budget] of scenarios) {
    const [a, b] = make();
    const t0 = process.hrtime.bigint();
    let result;
    try {
      result = diffLinesCounted(a, b, budget);
    } catch (e) {
      if (e instanceof DiffBudgetError) {
        const ms = Number(process.hrtime.bigint() - t0) / 1e6;
        console.log(`${name} | ${a.length} | ${b.length} | BUDGET | ${ms.toFixed(1)}`);
        continue;
      }
      throw e;
    }
    const ms = Number(process.hrtime.bigint() - t0) / 1e6;
    const edits = result.ops.filter((op) => op.type !== "same").length;
    console.log(
      `${name} | ${a.length} | ${b.length} | ${result.steps} | ${ms.toFixed(1)} (edits: ${edits})`,
    );
  }

  // Memory shape: the algorithm allocates two Int32Arrays of 2*ceil((n+m)/2)
  // per bisection level (transient), i.e. O(n+m) — no n*m matrix exists.
  console.log(
    "memory: 2 x Int32Array(2*ceil((n+m)/2)) per level = O(n+m) transient; no O(n*m) allocation",
  );
} finally {
  await rm(outDir, { recursive: true, force: true });
}
