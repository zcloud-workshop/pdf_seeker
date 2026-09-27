/** Per-file document sessions (R12): edit history, write mutex, view state and
 * a generation counter, all keyed by file path so they survive view switches
 * (Tools → Viewer → Tools). Sessions are released explicitly when the file's
 * tab is closed (see stores.closeTab). */
import { FileHistory } from "./history";
import {
  readPdfFingerprint,
  type PdfFingerprint,
} from "./restore";

export interface DocumentViewState {
  page: number;
  zoom: number;
}

export interface DocumentSession {
  path: string;
  /** Bumped on reload/close so in-flight async work can detect staleness
   * (R07/R14). */
  generation: number;
  /** Per-file write mutex for edit/undo/redo (R06). */
  writeBusy: boolean;
  /** Last (size, mtime) observed on disk, refreshed after every successful
   * write; used to detect external modifications before the next write
   * (07-A). Null until the first refresh. */
  fingerprint: PdfFingerprint | null;
  /** Mirrors of FileHistory counts kept on this reactive object so components
   * can $derive from them (FileHistory itself is a plain class instance). */
  undoCount: number;
  redoCount: number;
  history: FileHistory;
  view: DocumentViewState;
}

const sessions = $state(new Map<string, DocumentSession>());

export function peekSession(
  path: string | null | undefined,
): DocumentSession | undefined {
  return path ? sessions.get(path) : undefined;
}

export function getSession(path: string): DocumentSession {
  let s = sessions.get(path);
  if (!s) {
    s = {
      path,
      generation: 0,
      writeBusy: false,
      fingerprint: null,
      undoCount: 0,
      redoCount: 0,
      history: new FileHistory(),
      view: { page: 1, zoom: 1 },
    };
    sessions.set(path, s);
  }
  return s;
}

/** Explicit release when the file's tab is closed. Drops history and view
 * state; in-flight callbacks comparing generation will see the bump. */
export function releaseSession(path: string): void {
  const s = sessions.get(path);
  if (!s) return;
  s.generation++;
  sessions.delete(path);
}

export function bumpGeneration(path: string): void {
  const s = sessions.get(path);
  if (s) s.generation++;
}

// ─── History (delegated to FileHistory; counts mirrored for reactivity) ───

export function pushUndoSnapshot(path: string, bytes: Uint8Array): void {
  const s = getSession(path);
  s.history.pushUndo(bytes);
  s.undoCount = s.history.undoCount;
  s.redoCount = s.history.redoCount;
}

export function peekUndoSnapshot(path: string): Uint8Array | undefined {
  return peekSession(path)?.history.peekUndo();
}

export function peekRedoSnapshot(path: string): Uint8Array | undefined {
  return peekSession(path)?.history.peekRedo();
}

export function popUndoSnapshot(path: string): Uint8Array | undefined {
  const s = peekSession(path);
  if (!s) return undefined;
  const bytes = s.history.popUndo();
  s.undoCount = s.history.undoCount;
  s.redoCount = s.history.redoCount;
  return bytes;
}

export function pushRedoSnapshot(path: string, bytes: Uint8Array): void {
  const s = getSession(path);
  s.history.pushRedo(bytes);
  s.undoCount = s.history.undoCount;
  s.redoCount = s.history.redoCount;
}

export function popRedoSnapshot(path: string): Uint8Array | undefined {
  const s = peekSession(path);
  if (!s) return undefined;
  const bytes = s.history.popRedo();
  s.undoCount = s.history.undoCount;
  s.redoCount = s.history.redoCount;
  return bytes;
}

export function clearRedoStack(path: string): void {
  const s = peekSession(path);
  if (!s) return;
  s.history.clearRedo();
  s.undoCount = s.history.undoCount;
  s.redoCount = s.history.redoCount;
}

// ─── Per-file write mutex (R06: one edit/undo/redo per file at a time) ───

/** Returns false when another edit/undo/redo is already writing this file. */
export function beginFileWrite(path: string): boolean {
  const s = getSession(path);
  if (s.writeBusy) return false;
  s.writeBusy = true;
  return true;
}

export function endFileWrite(path: string): void {
  const s = sessions.get(path);
  if (s) s.writeBusy = false;
}

export function isFileWriteBusy(path: string): boolean {
  return sessions.get(path)?.writeBusy ?? false;
}

/** True when any open file has an edit/undo/redo in flight — used by window
 * close and update/restart coordination (07-A). */
export function anyWriteBusy(): boolean {
  for (const s of sessions.values()) {
    if (s.writeBusy) return true;
  }
  return false;
}

// ─── File fingerprint (07-A: external modification detection) ───

export function sessionFingerprint(
  path: string | null | undefined,
): PdfFingerprint | null {
  return path ? sessions.get(path)?.fingerprint ?? null : null;
}

export function setSessionFingerprint(path: string, fp: PdfFingerprint): void {
  const s = sessions.get(path);
  if (s) s.fingerprint = fp;
}

/** Re-reads (size, mtime) from disk and records it as the session's known
 * state. Call after every successful write. Best-effort: failures leave the
 * previous value untouched and return null. */
export async function refreshFileFingerprint(
  path: string,
): Promise<PdfFingerprint | null> {
  try {
    const fp = await readPdfFingerprint(path);
    const s = getSession(path);
    s.fingerprint = fp;
    return fp;
  } catch {
    return null;
  }
}

/** Best-effort pre-write check: if the session has a known fingerprint and
 * the file on disk no longer matches it, throw so the caller can surface the
 * conflict instead of overwriting the external change. The authoritative
 * check lives in commit_pdf_snapshot (enforced inside the write lock). */
export async function assertFileUnchanged(path: string): Promise<void> {
  const known = sessions.get(path)?.fingerprint;
  if (!known) return;
  const current = await readPdfFingerprint(path);
  if (current.size !== known.size || current.modifiedMs !== known.modifiedMs) {
    throw new Error(
      `'${path}' was modified outside this app; reload it before retrying`,
    );
  }
}
