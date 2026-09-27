/** R08 sample generator: small hand-built PDFs for coordinate-space testing.
 * The first-round remediation only records current behavior; 07-B switches
 * to a unified coordinate protocol and consumes these samples.
 *
 *   node tests/remediation-03/samples/make-samples.mjs
 *
 * Produces (next to this script):
 *   rotate-90.pdf       — page with /Rotate 90 (200×100 media box)
 *   cropbox-offset.pdf  — non-zero-origin /CropBox [36 36 576 756]
 *   float-a4.pdf        — floating-point A4 media box
 *
 * The script self-checks the xref table and required dictionary entries. */

import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

/** Build a minimal PDF from object bodies keyed by object number. */
function buildPdf(objects, rootNum) {
  const parts = ["%PDF-1.4\n%\xE2\xE3\xCF\xD3\n"];
  const offsets = new Map();
  let pos = 0;
  for (const p of parts) pos += p.length;
  for (const num of [...objects.keys()].sort((a, b) => a - b)) {
    offsets.set(num, pos);
    const chunk = `${num} 0 obj\n${objects.get(num)}\nendobj\n`;
    parts.push(chunk);
    pos += chunk.length;
  }
  const xrefPos = pos;
  const max = Math.max(...objects.keys());
  let xref = `xref\n0 ${max + 1}\n0000000000 65535 f \n`;
  for (let i = 1; i <= max; i++) {
    const off = offsets.get(i);
    xref +=
      String(off ?? 0).padStart(10, "0") +
      " 00000 " +
      (off !== undefined ? "n" : "f") +
      " \n";
  }
  parts.push(xref);
  parts.push(
    `trailer\n<< /Size ${max + 1} /Root ${rootNum} 0 R >>\nstartxref\n${xrefPos}\n%%EOF\n`,
  );
  return Buffer.from(parts.join(""), "latin1");
}

function contentObject(stream) {
  return `<< /Length ${stream.length} >>\nstream\n${stream}\nendstream`;
}

function makeSample(pageExtra, content, fileName, checks) {
  const objects = new Map();
  objects.set(1, "<< /Type /Catalog /Pages 2 0 R >>");
  objects.set(
    2,
    "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
  );
  objects.set(
    3,
    `<< /Type /Page /Parent 2 0 R /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R ${pageExtra} >>`,
  );
  objects.set(4, contentObject(content));
  objects.set(
    5,
    "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
  );
  const buf = buildPdf(objects, 1);
  // Self-check: every xref offset must point at its "N 0 obj".
  const text = buf.toString("latin1");
  const xrefMatch = text.match(/startxref\n(\d+)\n%%EOF/);
  if (!xrefMatch) throw new Error(`${fileName}: missing startxref`);
  const xrefStart = Number(xrefMatch[1]);
  const entryRe = /^(\d{10}) 00000 n /gm;
  const xrefText = text.slice(xrefStart);
  entryRe.lastIndex = xrefText.indexOf("\n") + 1; // skip free entry 0
  let m;
  let objNum = 1;
  while ((m = entryRe.exec(xrefText)) !== null) {
    const off = Number(m[1]);
    const expect = `${objNum} 0 obj`;
    if (text.slice(off, off + expect.length) !== expect) {
      throw new Error(`${fileName}: xref offset for object ${objNum} wrong`);
    }
    objNum++;
  }
  for (const c of checks) {
    if (!text.includes(c)) throw new Error(`${fileName}: missing "${c}"`);
  }
  writeFileSync(join(dirname(fileURLToPath(import.meta.url)), fileName), buf);
  console.log(`ok   - ${fileName} (${buf.length} bytes)`);
}

makeSample(
  "/MediaBox [0 0 200 100] /Rotate 90",
  "BT /F1 10 Tf 20 40 Td (rot90 marker) Tj ET",
  "rotate-90.pdf",
  ["/Rotate 90", "/MediaBox [0 0 200 100]"],
);

makeSample(
  "/MediaBox [0 0 612 792] /CropBox [36 36 576 756]",
  "BT /F1 10 Tf 40 700 Td (cropbox marker) Tj ET",
  "cropbox-offset.pdf",
  ["/CropBox [36 36 576 756]", "/MediaBox [0 0 612 792]"],
);

makeSample(
  "/MediaBox [0 0 595.28 841.89]",
  "BT /F1 10 Tf 72 720 Td (float a4 marker) Tj ET",
  "float-a4.pdf",
  ["/MediaBox [0 0 595.28 841.89]"],
);

console.log("all samples generated");
