import { assert, assertEquals, test } from "./framework.ts";
import {
  parsePageSelection,
  parseRangeGroups,
} from "../../src/lib/document/pageRanges.ts";

test("ranges: accepts plain lists, dedupes and sorts", () => {
  const r = parsePageSelection("3,1,1,2");
  assert(r.pages !== undefined, "parses");
  assertEquals(r.pages, [1, 2, 3], "sorted unique pages");
});

test("ranges: accepts ranges with spaces", () => {
  const r = parsePageSelection("1, 3, 5 - 7");
  assertEquals(r.pages, [1, 3, 5, 6, 7], "space-tolerant range");
});

test("ranges: rejects 3abc (R15)", () => {
  const r = parsePageSelection("3abc");
  assert(r.error !== undefined, "3abc rejected");
});

test("ranges: rejects 0 and negative-looking parts", () => {
  assert(parsePageSelection("0").error !== undefined, "0 rejected");
  assert(parsePageSelection("-3").error !== undefined, "-3 rejected");
  assert(parsePageSelection("1-").error !== undefined, "dangling dash rejected");
});

test("ranges: rejects empty input and empty items", () => {
  assert(parsePageSelection("").error !== undefined, "empty rejected");
  assert(parsePageSelection("   ").error !== undefined, "whitespace rejected");
  assert(parsePageSelection("1,,2").error !== undefined, "empty item rejected");
});

test("ranges: rejects reversed ranges", () => {
  assert(parsePageSelection("5-2").error !== undefined, "5-2 rejected");
});

test("ranges: bounds-checks against total pages", () => {
  assert(parsePageSelection("9", 5).error !== undefined, "out of bounds rejected");
  assert(parsePageSelection("1-9", 5).error !== undefined, "range end out of bounds");
  assertEquals(parsePageSelection("1-5", 5).pages, [1, 2, 3, 4, 5], "valid full range");
});

test("ranges: rejects oversized expansions before expanding further work", () => {
  const r = parsePageSelection("1-20000", undefined, 2000);
  assert(r.error !== undefined, "expansion budget enforced");
  assert(parsePageSelection("1-2000", undefined, 2000).pages?.length === 2000, "budget boundary allowed");
});

test("range groups: split-style syntax", () => {
  const g = parseRangeGroups("1-3,4-6,7");
  assert(g.groups !== undefined, "parses");
  assertEquals(g.groups, [[1, 2, 3], [4, 5, 6], [7]], "groups preserved in order");
  assert(parseRangeGroups("1-3,,4").error !== undefined, "empty group rejected");
  assert(parseRangeGroups("6-4").error !== undefined, "reversed group rejected");
  assert(parseRangeGroups("1-9", 5).error !== undefined, "group bounds checked");
});
