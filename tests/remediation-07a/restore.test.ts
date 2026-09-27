import { beforeEach, describe, expect, it, vi } from "vitest";

const invokeMock = vi.fn();
const writeFileMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));
vi.mock("@tauri-apps/plugin-fs", () => ({
  writeFile: (...args: unknown[]) => writeFileMock(...args),
}));
vi.mock("@tauri-apps/api/path", () => ({
  tempDir: async () => "/mock-temp",
  join: (...parts: string[]) => parts.join("/"),
}));

import { readPdfFingerprint, restoreSnapshot } from "@/document/restore";

describe("restoreSnapshot IPC contract (07-A)", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    writeFileMock.mockReset();
  });

  it("stages the snapshot under the temp dir and maps args to camelCase", async () => {
    invokeMock.mockResolvedValue({ size: 10, modifiedMs: 99 });
    const bytes = new Uint8Array([1, 2, 3]);
    const fp = await restoreSnapshot("/docs/a.pdf", bytes, {
      size: 8,
      modifiedMs: 12,
    });
    expect(fp).toEqual({ size: 10, modifiedMs: 99 });
    expect(writeFileMock).toHaveBeenCalledOnce();
    const [scratch, written] = writeFileMock.mock.calls[0];
    expect(String(scratch)).toMatch(/^\/mock-temp\/pdf-seeker-snapshot-.*\.pdf$/);
    expect(written).toBe(bytes);
    expect(invokeMock).toHaveBeenCalledWith("commit_pdf_snapshot", {
      path: "/docs/a.pdf",
      snapshotPath: scratch,
      expectedSize: 8,
      expectedModifiedMs: 12,
    });
  });

  it("sends null expectations when no fingerprint is known", async () => {
    invokeMock.mockResolvedValue({ size: 3, modifiedMs: 4 });
    await restoreSnapshot("/docs/b.pdf", new Uint8Array([9]));
    expect(invokeMock).toHaveBeenCalledWith("commit_pdf_snapshot", {
      path: "/docs/b.pdf",
      snapshotPath: expect.any(String),
      expectedSize: null,
      expectedModifiedMs: null,
    });
  });

  it("propagates the backend refusal on a stale fingerprint", async () => {
    invokeMock.mockRejectedValue(
      new Error("'/docs/c.pdf' was modified outside this app; reload it before retrying"),
    );
    await expect(
      restoreSnapshot("/docs/c.pdf", new Uint8Array([1]), { size: 1, modifiedMs: 1 }),
    ).rejects.toThrow(/modified outside this app/);
  });

  it("readPdfFingerprint invokes the stat command with the plain path", async () => {
    invokeMock.mockResolvedValue({ size: 5, modifiedMs: 6 });
    await expect(readPdfFingerprint("/docs/d.pdf")).resolves.toEqual({
      size: 5,
      modifiedMs: 6,
    });
    expect(invokeMock).toHaveBeenCalledWith("pdf_file_fingerprint", {
      path: "/docs/d.pdf",
    });
  });
});
