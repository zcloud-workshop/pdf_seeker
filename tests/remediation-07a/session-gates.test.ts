import { beforeEach, describe, expect, it, vi } from "vitest";

const readPdfFingerprintMock = vi.fn();

vi.mock("@/document/restore", () => ({
  readPdfFingerprint: (...args: unknown[]) => readPdfFingerprintMock(...args),
}));

import { get } from "svelte/store";
import {
  anyWriteBusy,
  assertFileUnchanged,
  beginFileWrite,
  endFileWrite,
  getSession,
  isFileWriteBusy,
  refreshFileFingerprint,
  sessionFingerprint,
  setSessionFingerprint,
} from "@/document/session.svelte.ts";
import { closeTab, openTab, tabs } from "@/stores";

describe("per-file write gates (07-A)", () => {
  beforeEach(() => {
    readPdfFingerprintMock.mockReset();
  });

  it("aggregates the write mutex across files", () => {
    expect(anyWriteBusy()).toBe(false);
    expect(beginFileWrite("/gate-a.pdf")).toBe(true);
    expect(isFileWriteBusy("/gate-a.pdf")).toBe(true);
    expect(isFileWriteBusy("/gate-b.pdf")).toBe(false);
    expect(anyWriteBusy()).toBe(true);
    // Second write to the same file is refused while one is in flight.
    expect(beginFileWrite("/gate-a.pdf")).toBe(false);
    endFileWrite("/gate-a.pdf");
    expect(anyWriteBusy()).toBe(false);
    expect(beginFileWrite("/gate-a.pdf")).toBe(true);
    endFileWrite("/gate-a.pdf");
  });

  it("refresh records the fingerprint used by assertFileUnchanged", async () => {
    readPdfFingerprintMock.mockResolvedValue({ size: 11, modifiedMs: 22 });
    await expect(refreshFileFingerprint("/fp-a.pdf")).resolves.toEqual({
      size: 11,
      modifiedMs: 22,
    });
    expect(sessionFingerprint("/fp-a.pdf")).toEqual({ size: 11, modifiedMs: 22 });

    readPdfFingerprintMock.mockResolvedValue({ size: 11, modifiedMs: 22 });
    await expect(assertFileUnchanged("/fp-a.pdf")).resolves.toBeUndefined();

    readPdfFingerprintMock.mockResolvedValue({ size: 12, modifiedMs: 22 });
    await expect(assertFileUnchanged("/fp-a.pdf")).rejects.toThrow(
      /modified outside this app/,
    );

    // Unknown files have no baseline — the check is a no-op.
    readPdfFingerprintMock.mockClear();
    await expect(assertFileUnchanged("/fp-unknown.pdf")).resolves.toBeUndefined();
    expect(readPdfFingerprintMock).not.toHaveBeenCalled();
  });

  it("setSessionFingerprint updates the baseline for later comparisons", async () => {
    // The session must exist (a write gate or refresh creates it first).
    getSession("/fp-b.pdf");
    setSessionFingerprint("/fp-b.pdf", { size: 1, modifiedMs: 2 });
    expect(sessionFingerprint("/fp-b.pdf")).toEqual({ size: 1, modifiedMs: 2 });
    readPdfFingerprintMock.mockResolvedValue({ size: 1, modifiedMs: 2 });
    await expect(assertFileUnchanged("/fp-b.pdf")).resolves.toBeUndefined();
  });

  it("closeTab refuses while the file has a write in flight", () => {
    const tab = openTab("/close-guard.pdf");
    expect(get(tabs).length).toBe(1);
    expect(beginFileWrite("/close-guard.pdf")).toBe(true);

    expect(closeTab(tab.id)).toBe(false);
    expect(get(tabs).length).toBe(1);

    endFileWrite("/close-guard.pdf");
    expect(closeTab(tab.id)).toBe(true);
    expect(get(tabs).length).toBe(0);
  });
});
