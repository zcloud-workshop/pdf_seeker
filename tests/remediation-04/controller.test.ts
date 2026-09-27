/**
 * Remediation 04 — controlled request-order tests for the analyze
 * controller.
 *
 * Verifies that a file pair plus request token pin the identity of one
 * analysis: a late-returned older request (or its finally block) can never
 * overwrite the newer pair's texts, errors or analyzing flag, and that
 * dispose voids in-flight requests and releases the diff engine.
 */
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  CompareController,
  initialCompareState,
  type CompareState,
} from "../../src/lib/comparison/analyze-controller";
import type { DiffRunOutcome, PageDiff } from "../../src/lib/comparison/page-diff";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise<void>((r) => setTimeout(r, 0));

function okOutcome(pages: PageDiff[] = [{ index: 1, status: "same", ops: [] }]): DiffRunOutcome {
  return {
    pages,
    stoppedEarly: false,
    skippedCount: 0,
    canceled: false,
    engineFailed: false,
  };
}

class FakeDiff {
  calls: { textsA: string[]; textsB: string[]; resolve: (o: DiffRunOutcome | null) => void }[] =
    [];
  disposed = false;

  run(textsA: string[], textsB: string[]) {
    let resolve!: (o: DiffRunOutcome | null) => void;
    const promise = new Promise<DiffRunOutcome | null>((r) => {
      resolve = r;
    });
    this.calls.push({ textsA, textsB, resolve });
    return promise;
  }

  dispose() {
    this.disposed = true;
    for (const c of this.calls.splice(0)) c.resolve(null);
  }
}

function setup(extractMap: Record<string, ReturnType<typeof deferred<string[]>>>) {
  const states: CompareState[] = [];
  const diff = new FakeDiff();
  const controller = new CompareController({
    extract: (path) => {
      const d = extractMap[path];
      if (!d) return Promise.reject(new Error(`unexpected path ${path}`));
      return d.promise;
    },
    diff,
    onState: (s) => states.push(s),
  });
  return { states, diff, controller };
}

test("A1/B1 returning later than A2/B2 cannot overwrite the newer pair", async () => {
  const a1 = deferred<string[]>();
  const b1 = deferred<string[]>();
  const a2 = deferred<string[]>();
  const b2 = deferred<string[]>();
  const { states, diff, controller } = setup({ A1: a1, B1: b1, A2: a2, B2: b2 });

  controller.run({ a: "A1", b: "B1" });
  controller.run({ a: "A2", b: "B2" });

  // Newer pair resolves first.
  a2.resolve(["page-one-A2"]);
  b2.resolve(["page-one-B2"]);
  await flush();
  assert.equal(diff.calls.length, 1, "old pair must never reach the diff stage");
  assert.deepEqual(diff.calls[0].textsA, ["page-one-A2"]);

  diff.calls[0].resolve(okOutcome());
  await flush();

  // Older pair resolves last — must be fully discarded.
  a1.resolve(["stale-A1"]);
  b1.resolve(["stale-B1"]);
  await flush();
  await flush();

  assert.equal(diff.calls.length, 1, "stale extract must not start a diff");
  const last = states[states.length - 1];
  assert.equal(last.analyzing, false);
  assert.equal(last.errorMsg, "");
  assert.deepEqual(last.pair, { a: "A2", b: "B2" });
  assert.deepEqual(last.textsA, ["page-one-A2"]);

  // Every emitted state belongs to the latest pair (after the second run began).
  for (const s of states.slice(1)) {
    assert.deepEqual(s.pair, { a: "A2", b: "B2" });
  }
});

test("error belongs to its own pair and is cleared by the next success", async () => {
  const e1 = deferred<string[]>();
  const e2 = deferred<string[]>();
  const { states, diff, controller } = setup({ bad: e1, ok: e2 });

  controller.run({ a: "bad", b: "ok" });
  e1.reject(new Error("boom"));
  await flush();
  let last = states[states.length - 1];
  assert.equal(last.analyzing, false);
  assert.ok(last.errorMsg.includes("boom"));
  assert.deepEqual(last.pair, { a: "bad", b: "ok" });

  controller.run({ a: "ok", b: "ok" });
  e2.resolve(["t"]);
  e2.resolve(["t"]); // second resolve is a no-op on an already-settled promise
  await flush();
  assert.equal(diff.calls.length, 1);
  diff.calls[0].resolve(okOutcome());
  await flush();
  last = states[states.length - 1];
  assert.equal(last.errorMsg, "");
  assert.equal(last.analyzing, false);
  assert.deepEqual(last.pair, { a: "ok", b: "ok" });
});

test("superseded diff result (null) leaves analyzing to the newer request", async () => {
  const d = deferred<string[]>();
  const { states, diff, controller } = setup({ f: d });

  controller.run({ a: "f", b: "f" });
  d.resolve(["x"]);
  await flush();
  assert.equal(diff.calls.length, 1);

  // A newer run starts before the diff finishes: old diff promise resolves null.
  controller.run({ a: "f", b: "f" });
  diff.calls[0].resolve(null);
  await flush();
  const last = states[states.length - 1];
  assert.deepEqual(last.pair, { a: "f", b: "f" });
  assert.equal(last.analyzing, true, "null outcome must not flip analyzing off");

  // Finish the second run normally.
  d.resolve(["x2"]); // already-settled no-op; second diff call still pending
  const second = diff.calls[1];
  second.resolve(okOutcome());
  await flush();
  assert.equal(states[states.length - 1].analyzing, false);
});

test("canceled diff outcome is discarded", async () => {
  const d = deferred<string[]>();
  const { states, diff, controller } = setup({ f: d });

  controller.run({ a: "f", b: "f" });
  d.resolve(["x"]);
  await flush();
  diff.calls[0].resolve({ ...okOutcome(), canceled: true, stoppedEarly: true });
  await flush();
  assert.equal(states[states.length - 1].analyzing, true);
  assert.equal(states[states.length - 1].pages.length, 0);
});

test("dispose voids in-flight requests and releases the diff engine", async () => {
  const d = deferred<string[]>();
  const { states, diff, controller } = setup({ f: d });

  controller.run({ a: "f", b: "f" });
  controller.dispose();
  assert.equal(diff.disposed, true);

  const countBefore = states.length;
  d.resolve(["late"]);
  await flush();
  await flush();
  assert.equal(states.length, countBefore, "no state after dispose");
  const last = states[states.length - 1];
  assert.equal(last.analyzing, true, "dispose must not flip analyzing via a stale finally");
  assert.deepEqual(last.pair, { a: "f", b: "f" });
});
