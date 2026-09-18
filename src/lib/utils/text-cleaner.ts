/**
 * Text Post-Processing Pipeline inspired by Umi-OCR.
 * Cleans OCR artifacts: cross-line hyphenation, broken sentence lines,
 * and normalizes spacing between CJK and Latin/alphanumerics.
 */

export function isCjk(char: string): boolean {
  if (!char) return false;
  const cp = char.codePointAt(0);
  if (cp === undefined) return false;
  return (
    (cp >= 0x4e00 && cp <= 0x9fff) || // CJK Unified Ideographs
    (cp >= 0x3400 && cp <= 0x4dbf) || // CJK Extension A
    (cp >= 0xf900 && cp <= 0xfaff) || // CJK Compatibility Ideographs
    (cp >= 0x3040 && cp <= 0x309f) || // Hiragana
    (cp >= 0x30a0 && cp <= 0x30ff) || // Katakana
    (cp >= 0xac00 && cp <= 0xd7af)    // Hangul Syllables
  );
}

export function isSentenceTerminator(char: string): boolean {
  return /^[.!?;:。！？；：]$/.test(char);
}

export function isAlphabetic(char: string): boolean {
  return /^[a-zA-Z]$/.test(char);
}

export function isAlphanumeric(char: string): boolean {
  return /^[a-zA-Z0-9]$/.test(char);
}

/**
 * Remove word hyphenation broken across line breaks (e.g. "inter-\n   national" -> "international").
 */
export function cleanHyphenation(text: string): string {
  // Regex matches letter followed by hyphen/soft-hyphen, followed by whitespace containing at least one newline, followed by letter
  return text.replace(/([a-zA-Z])[-\u00AD]\s*\n\s*([a-zA-Z])/g, "$1$2");
}

/**
 * Merge lines artificially broken by OCR layout without terminal punctuation.
 */
export function mergeParagraphs(text: string): string {
  const lines = text.split(/\r?\n/);
  if (lines.length === 0) return "";

  const result: string[] = [];

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    if (!line) {
      result.push("\n\n");
      continue;
    }

    result.push(line);

    if (i + 1 < lines.length) {
      const nextLine = lines[i + 1].trim();
      if (!nextLine) continue;

      const lastChar = line.slice(-1);
      const nextFirstChar = nextLine.slice(0, 1);

      if (isSentenceTerminator(lastChar)) {
        result.push("\n");
      } else if (isCjk(lastChar) && isCjk(nextFirstChar)) {
        // Continuous CJK: no space
      } else {
        // Latin/numbers: join with space
        result.push(" ");
      }
    }
  }

  return result.join("");
}

/**
 * Normalize spacing between CJK ideographs and ASCII alphanumeric words/numbers.
 * e.g. "PDF阅读器2026版" -> "PDF 阅读器 2026 版"
 */
export function normalizeCjkSpacing(text: string): string {
  let result = "";
  const chars = Array.from(text);

  for (let i = 0; i < chars.length; i++) {
    const curr = chars[i];
    result += curr;

    if (i + 1 < chars.length) {
      const next = chars[i + 1];
      const cjkThenAlnum = isCjk(curr) && isAlphanumeric(next);
      const alnumThenCjk = isAlphanumeric(curr) && isCjk(next);

      if (cjkThenAlnum || alnumThenCjk) {
        result += " ";
      }
    }
  }

  return result;
}

/**
 * Full OCR text cleaning pipeline.
 */
export function cleanOcrText(rawText: string): string {
  const unhyphenated = cleanHyphenation(rawText);
  const merged = mergeParagraphs(unhyphenated);
  return normalizeCjkSpacing(merged);
}

