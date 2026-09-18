export interface EditOperation {
  id: string;
  type: "addText" | "addRectangle" | "addHighlight" | "addWatermark" | "addSignature" | "addWhiteout";
  params: Record<string, any>;
  timestamp: number;
}

export interface EditHistory {
  operations: EditOperation[];
  currentIndex: number;
}

let idCounter = 0;

export function createEditHistory(): EditHistory {
  return { operations: [], currentIndex: -1 };
}

export function pushOperation(history: EditHistory, type: EditOperation["type"], params: Record<string, any>): EditHistory {
  // Discard any redo'd operations
  const newOps = history.operations.slice(0, history.currentIndex + 1);
  newOps.push({ id: String(++idCounter), type, params, timestamp: Date.now() });
  return { operations: newOps, currentIndex: newOps.length - 1 };
}

export function undo(history: EditHistory): { history: EditHistory; operation: EditOperation | null } {
  if (history.currentIndex < 0) return { history, operation: null };
  const operation = history.operations[history.currentIndex];
  return { history: { ...history, currentIndex: history.currentIndex - 1 }, operation };
}

export function redo(history: EditHistory): { history: EditHistory; operation: EditOperation | null } {
  if (history.currentIndex >= history.operations.length - 1) return { history, operation: null };
  const nextIndex = history.currentIndex + 1;
  const operation = history.operations[nextIndex];
  return { history: { ...history, currentIndex: nextIndex }, operation };
}

export function canUndo(history: EditHistory): boolean {
  return history.currentIndex >= 0;
}

export function canRedo(history: EditHistory): boolean {
  return history.currentIndex < history.operations.length - 1;
}

export function getAppliedOperations(history: EditHistory): EditOperation[] {
  return history.operations.slice(0, history.currentIndex + 1);
}

export function clearHistory(): EditHistory {
  return createEditHistory();
}
