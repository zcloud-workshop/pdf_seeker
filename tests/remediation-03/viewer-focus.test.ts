import { assert, assertEquals, test } from "./framework.ts";
import {
  claimViewerFocus,
  isViewerFocusOwner,
  releaseViewerFocus,
  subscribeViewerFocus,
  viewerFocusOwner,
} from "../../src/lib/document/viewerFocus.ts";

test("focus: unclaimed state lets any scroller act", () => {
  releaseViewerFocus("a");
  assertEquals(viewerFocusOwner(), null, "no owner");
  assert(isViewerFocusOwner("a"), "a may act when unclaimed");
  assert(isViewerFocusOwner("b"), "b may act when unclaimed");
});

test("focus: claim makes exactly one owner (R29)", () => {
  releaseViewerFocus("a");
  claimViewerFocus("a");
  assert(isViewerFocusOwner("a"), "owner acts");
  assert(!isViewerFocusOwner("b"), "non-owner blocked");
  assertEquals(viewerFocusOwner(), "a", "owner tracked");
  claimViewerFocus("b");
  assert(!isViewerFocusOwner("a"), "previous owner loses claim");
  assert(isViewerFocusOwner("b"), "new owner acts");
});

test("focus: release only by the owner; subscribers notified", () => {
  claimViewerFocus("b");
  let notified = 0;
  const unsub = subscribeViewerFocus(() => notified++);
  releaseViewerFocus("a"); // not the owner — no effect, no notification
  assertEquals(notified, 0, "foreign release ignored");
  assertEquals(viewerFocusOwner(), "b", "owner unchanged");
  releaseViewerFocus("b");
  assertEquals(notified, 1, "release notified subscribers");
  assertEquals(viewerFocusOwner(), null, "owner cleared");
  unsub();
});
