/** Per-file document sessions (R12): edit history, write mutex, view state and
 * a generation counter, all keyed by file path so they survive view switches
 * (Tools → Viewer → Tools). Sessions are released explicitly when the file's
 * tab is closed (see stores.closeTab). */
import { FileHistory } from "./history";

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
