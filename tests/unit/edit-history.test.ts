import { describe, it, expect } from "vitest";
import {
  createEditHistory,
  pushOperation,
  undo,
  redo,
  canUndo,
  canRedo,
  getAppliedOperations,
  clearHistory,
} from "@/edit-history";

describe("EditHistory Core Logic", () => {
  it("initializes empty edit history", () => {
    const history = createEditHistory();
    expect(history.operations).toEqual([]);
    expect(history.currentIndex).toBe(-1);
    expect(canUndo(history)).toBe(false);
    expect(canRedo(history)).toBe(false);
    expect(getAppliedOperations(history)).toEqual([]);
  });

  it("pushes operations sequentially and updates index", () => {
    let history = createEditHistory();
    history = pushOperation(history, "addText", { text: "Hello", page: 1, x: 10, y: 20 });
    expect(history.operations.length).toBe(1);
    expect(history.currentIndex).toBe(0);
    expect(canUndo(history)).toBe(true);
    expect(canRedo(history)).toBe(false);

    history = pushOperation(history, "addRectangle", { page: 1, x: 50, y: 50, width: 100, height: 40 });
    expect(history.operations.length).toBe(2);
    expect(history.currentIndex).toBe(1);
    expect(getAppliedOperations(history).length).toBe(2);
  });

  it("handles single and multi-step undo/redo correctly", () => {
    let history = createEditHistory();
    history = pushOperation(history, "addText", { text: "Op 1" });
    history = pushOperation(history, "addHighlight", { text: "Op 2" });
    history = pushOperation(history, "addWhiteout", { text: "Op 3" });

    expect(history.currentIndex).toBe(2);

    // Undo step 1
    const res1 = undo(history);
    history = res1.history;
    expect(res1.operation?.type).toBe("addWhiteout");
    expect(history.currentIndex).toBe(1);
    expect(canUndo(history)).toBe(true);
    expect(canRedo(history)).toBe(true);
    expect(getAppliedOperations(history).length).toBe(2);

    // Undo step 2
    const res2 = undo(history);
    history = res2.history;
    expect(res2.operation?.type).toBe("addHighlight");
    expect(history.currentIndex).toBe(0);

    // Undo step 3
    const res3 = undo(history);
    history = res3.history;
    expect(res3.operation?.type).toBe("addText");
    expect(history.currentIndex).toBe(-1);
    expect(canUndo(history)).toBe(false);
    expect(canRedo(history)).toBe(true);
    expect(getAppliedOperations(history).length).toBe(0);

    // Excessive undo should be a safe no-op
    const res4 = undo(history);
    expect(res4.operation).toBeNull();
    expect(res4.history.currentIndex).toBe(-1);

    // Redo back to Op 1
    const redo1 = redo(history);
    history = redo1.history;
    expect(redo1.operation?.type).toBe("addText");
    expect(history.currentIndex).toBe(0);

    // Redo back to Op 2
    const redo2 = redo(history);
    history = redo2.history;
    expect(redo2.operation?.type).toBe("addHighlight");
    expect(history.currentIndex).toBe(1);
  });

  it("discards redo stack when a new operation is pushed after undo", () => {
    let history = createEditHistory();
    history = pushOperation(history, "addText", { text: "First" });
    history = pushOperation(history, "addText", { text: "Second" });
    history = pushOperation(history, "addText", { text: "Third" });

    // Undo twice -> currentIndex is 0 ("First")
    history = undo(history).history;
    history = undo(history).history;
    expect(history.currentIndex).toBe(0);
    expect(canRedo(history)).toBe(true);

    // Push new operation -> "Second" and "Third" must be discarded
    history = pushOperation(history, "addSignature", { text: "New Branch" });
    expect(history.operations.length).toBe(2);
    expect(history.currentIndex).toBe(1);
    expect(history.operations[0].params.text).toBe("First");
    expect(history.operations[1].params.text).toBe("New Branch");
    expect(canRedo(history)).toBe(false);
  });

  it("clears history completely", () => {
    let history = createEditHistory();
    history = pushOperation(history, "addText", { text: "Temp" });
    history = clearHistory();
    expect(history.operations).toEqual([]);
    expect(history.currentIndex).toBe(-1);
    expect(canUndo(history)).toBe(false);
  });
});

