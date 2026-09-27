/** Byte-budgeted undo/redo history for in-place PDF edits, keyed per file.
 * Pure logic with no framework imports so it can run directly under node in
 * tests/remediation-03 (R12: 历史字节预算). */

export const HISTORY_MAX_ENTRIES = 30;
export const HISTORY_MAX_BYTES = 64 * 1024 * 1024;

export class FileHistory {
  private undoStack: Uint8Array[] = [];
  private redoStack: Uint8Array[] = [];
  private bytes = 0;

  get undoCount(): number {
    return this.undoStack.length;
  }

  get redoCount(): number {
    return this.redoStack.length;
  }

  get byteSize(): number {
    return this.bytes;
  }

  /** Record a pre-edit snapshot of the file. Oldest undo entries are dropped
   * first when the combined byte budget or entry cap is exceeded. */
  pushUndo(snapshot: Uint8Array): void {
    this.undoStack.push(snapshot);
    this.bytes += snapshot.length;
    this.evict();
  }

  popUndo(): Uint8Array | undefined {
    const s = this.undoStack.pop();
    if (s) this.bytes -= s.length;
    return s;
  }

  peekUndo(): Uint8Array | undefined {
    return this.undoStack[this.undoStack.length - 1];
  }

  /** Record the post-undo state so a redo can restore it. */
  pushRedo(snapshot: Uint8Array): void {
    this.redoStack.push(snapshot);
    this.bytes += snapshot.length;
    this.evict();
  }

  popRedo(): Uint8Array | undefined {
    const s = this.redoStack.pop();
    if (s) this.bytes -= s.length;
    return s;
  }

  peekRedo(): Uint8Array | undefined {
    return this.redoStack[this.redoStack.length - 1];
  }

  clearRedo(): void {
    for (const s of this.redoStack) this.bytes -= s.length;
    this.redoStack = [];
  }

  private evict(): void {
    while (this.bytes > HISTORY_MAX_BYTES) {
      const from = this.undoStack.length > 0 ? this.undoStack : this.redoStack;
      if (from.length === 0) break;
      const dropped = from.shift()!;
      this.bytes -= dropped.length;
    }
    while (this.undoStack.length > HISTORY_MAX_ENTRIES) {
      const dropped = this.undoStack.shift()!;
      this.bytes -= dropped.length;
    }
    while (this.redoStack.length > HISTORY_MAX_ENTRIES) {
      const dropped = this.redoStack.shift()!;
      this.bytes -= dropped.length;
    }
  }
}
