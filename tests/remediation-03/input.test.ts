import { assertEquals, test } from "./framework.ts";
import {
  clampInt,
  intOr,
  isTruthyExportValue,
  numOr,
} from "../../src/lib/document/input.ts";

test("input: legitimate 0 survives numOr (R27)", () => {
  assertEquals(numOr("0", 48), 0, "zero kept, not replaced by default");
  assertEquals(numOr("0.0", -45), 0, "zero float kept");
});

test("input: NaN/empty falls back", () => {
  assertEquals(numOr("", 48), 48, "empty → default");
  assertEquals(numOr("abc", 48), 48, "garbage → default");
  assertEquals(numOr("-45", -45), -45, "negative default expressible");
});

test("input: intOr and clampInt", () => {
  assertEquals(intOr("7", 1), 7, "int parsed");
  assertEquals(intOr("", 1), 1, "int fallback");
  assertEquals(clampInt(99, 1, 10), 10, "clamped high");
  assertEquals(clampInt(0, 1, 10), 1, "clamped low");
});

test("input: checkbox export values — no guessing (R10)", () => {
  for (const v of ["Yes", "YES", "true", "1", "on", "checked", " On "]) {
    assertEquals(isTruthyExportValue(v), true, `"${v}" recognized as checked`);
  }
  for (const v of ["Off", "", "#1", "no", "0"]) {
    assertEquals(isTruthyExportValue(v), false, `"${v}" not treated as checked`);
  }
});
