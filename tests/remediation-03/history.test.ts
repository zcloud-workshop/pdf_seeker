import { assert, assertEquals, test } from "./framework.ts";
import {
  FileHistory,
  HISTORY_MAX_BYTES,
  HISTORY_MAX_ENTRIES,
} from "../../src/lib/document/history.ts";

test("history: push/pop undo roundtrip keeps counts and bytes in sync", () => {
  const h = new FileHistory();
  h.pushUndo(new Uint8Array([1, 2, 3]));
  assertEquals(h.undoCount, 1, "undoCount after push");
  assertEquals(h.byteSize, 3, "byteSize after push");
  const popped = h.popUndo();
  assert(popped !== undefined && popped.length === 3 && popped[0] === 1, "pop returns the snapshot");
  assertEquals(h.undoCount, 0, "undoCount after pop");
  assertEquals(h.byteSize, 0, "byteSize after pop");
});

test("history: entry cap evicts the oldest undo snapshots", () => {
  const h = new FileHistory();
  for (let i = 0; i < HISTORY_MAX_ENTRIES + 5; i++) {
    h.pushUndo(new Uint8Array([i]));
  }
  assertEquals(h.undoCount, HISTORY_MAX_ENTRIES, "undoCount capped at max entries");
  assertEquals(h.byteSize, HISTORY_MAX_ENTRIES, "byteSize matches surviving entries");
  assert(h.peekUndo()?.[0] === HISTORY_MAX_ENTRIES + 4, "newest snapshot survived");
});

test("history: byte budget drops oldest entries first", () => {
  const h = new FileHistory();
  const size = 25 * 1024 * 1024;
  for (let i = 0; i < 3; i++) {
    h.pushUndo(new Uint8Array(size)); // 3 × 25 MB = 75 MB > 64 MB budget
  }
  assert(h.byteSize <= HISTORY_MAX_BYTES, `byteSize ${h.byteSize} within budget`);
  assertEquals(h.undoCount, 2, "oldest of three 25MB snapshots evicted");
});

test("history: redo counts toward the same budget and clearRedo frees it", () => {
  const h = new FileHistory();
  h.pushUndo(new Uint8Array(10));
  h.popUndo();
  h.pushRedo(new Uint8Array(20));
  assertEquals(h.redoCount, 1, "redo recorded");
  assertEquals(h.byteSize, 20, "redo bytes tracked");
  h.clearRedo();
  assertEquals(h.redoCount, 0, "redo cleared");
  assertEquals(h.byteSize, 0, "bytes released");
});

test("history: pushUndo clears nothing — redo branch is explicit", () => {
  // After an undo, a new edit must clear redo explicitly (see Tools'
  // applyEditInPlace calling clearRedoStack) — FileHistory stays mechanical.
  const h = new FileHistory();
  h.pushRedo(new Uint8Array([9]));
  h.pushUndo(new Uint8Array([1]));
  assertEquals(h.redoCount, 1, "redo untouched by pushUndo");
});
