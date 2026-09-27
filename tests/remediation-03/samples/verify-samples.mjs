/** Verify the generated samples parse with the locked pdfjs-dist (metadata
 * only — no rendering). Run after make-samples.mjs:
 *   node tests/remediation-03/samples/verify-samples.mjs */
import { getDocument } from "pdfjs-dist/legacy/build/pdf.mjs";
import { readFile } from "node:fs/promises";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
let failed = 0;
for (const f of ["rotate-90.pdf", "cropbox-offset.pdf", "float-a4.pdf"]) {
  try {
    const data = new Uint8Array(await readFile(`${here}/${f}`));
    const doc = await getDocument({ data, isEvalSupported: false }).promise;
    const page = await doc.getPage(1);
    const vp = page.getViewport({ scale: 1 });
    const tc = await page.getTextContent();
    const text = tc.items.map((i) => i.str).join("");
    if (!text) throw new Error("no text layer");
    console.log(`ok   - ${f}: pages=${doc.numPages} rotate=${page.rotate} vp=${vp.width.toFixed(2)}x${vp.height.toFixed(2)}`);
    doc.destroy();
  } catch (e) {
    failed++;
    console.error(`FAIL - ${f}: ${e.message ?? e}`);
  }
}
if (failed > 0) process.exitCode = 1;
