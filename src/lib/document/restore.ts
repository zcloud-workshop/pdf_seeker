/** 07-A: atomic snapshot restore through the backend write transaction.
 *
 * Undo/redo and failure rollbacks used to write bytes straight to the target
 * file with plugin-fs, bypassing the backend's locked atomic save path. These
 * helpers route every restore through `commit_pdf_snapshot`, which takes the
 * PDF write lock, optionally verifies the target's (size, mtime) fingerprint
 * and replaces the file atomically. */
import { invoke } from "@tauri-apps/api/core";
import { writeFile } from "@tauri-apps/plugin-fs";
import { join, tempDir } from "@tauri-apps/api/path";

export interface PdfFingerprint {
  size: number;
  modifiedMs: number;
}

export async function readPdfFingerprint(path: string): Promise<PdfFingerprint> {
  return invoke<PdfFingerprint>("pdf_file_fingerprint", { path });
}

/** Writes `bytes` to a scratch file under the system temp dir and commits it
 * as the new content of `path`. `expected`, when provided, is the (size,
 * mtime) the caller last observed; if the file changed outside this app in
 * between, the restore is refused with an error so the caller can reload.
 * The backend deletes the scratch file; no cleanup is needed here. */
export async function restoreSnapshot(
  path: string,
  bytes: Uint8Array,
  expected?: PdfFingerprint | null,
): Promise<PdfFingerprint> {
  const scratch = await join(
    await tempDir(),
    `pdf-seeker-snapshot-${crypto.randomUUID()}.pdf`,
  );
  await writeFile(scratch, bytes);
  return invoke<PdfFingerprint>("commit_pdf_snapshot", {
    path,
    snapshotPath: scratch,
    expectedSize: expected?.size ?? null,
    expectedModifiedMs: expected?.modifiedMs ?? null,
  });
}
