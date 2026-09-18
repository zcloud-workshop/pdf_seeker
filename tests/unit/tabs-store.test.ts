import { describe, it, expect, beforeEach } from "vitest";
import { get } from "svelte/store";
import { tabsStore, activeTab, tabCount } from "@/stores/tabs";

describe("Tabs Store State Management", () => {
  beforeEach(() => {
    tabsStore.clearTabs();
  });

  it("initializes with zero tabs and null active tab", () => {
    expect(get(tabCount)).toBe(0);
    expect(get(activeTab)).toBeNull();
  });

  it("opens a new tab and sets it as active", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    expect(get(tabCount)).toBe(1);
    const active = get(activeTab);
    expect(active?.id).toBe(id1);
    expect(active?.filePath).toBe("/docs/doc1.pdf");
    expect(active?.fileName).toBe("doc1.pdf");
    expect(active?.isActive).toBe(true);
  });

  it("activates existing tab instead of opening duplicate", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    const id2 = tabsStore.openTab("/docs/doc2.pdf", "doc2.pdf");
    expect(get(tabCount)).toBe(2);
    expect(get(activeTab)?.id).toBe(id2);

    // Reopen doc1 -> should switch activeTab to id1 without creating 3rd tab
    const reopenId = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    expect(reopenId).not.toBeNull();
    expect(get(tabCount)).toBe(2);
    expect(get(activeTab)?.id).toBe(id1);
  });

  it("closes tabs and adjusts activeTab smoothly", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    const id2 = tabsStore.openTab("/docs/doc2.pdf", "doc2.pdf");
    const id3 = tabsStore.openTab("/docs/doc3.pdf", "doc3.pdf");

    // Close the active tab (id3) -> activeTab should fallback to id2
    tabsStore.closeTab(id3);
    expect(get(tabCount)).toBe(2);
    expect(get(activeTab)?.id).toBe(id2);

    // Close id1 -> activeTab should stay id2
    tabsStore.closeTab(id1);
    expect(get(tabCount)).toBe(1);
    expect(get(activeTab)?.id).toBe(id2);

    // Close final tab -> activeTab becomes null
    tabsStore.closeTab(id2);
    expect(get(tabCount)).toBe(0);
    expect(get(activeTab)).toBeNull();
  });

  it("cycles through tabs using activateNextTab and activatePrevTab", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    const id2 = tabsStore.openTab("/docs/doc2.pdf", "doc2.pdf");
    const id3 = tabsStore.openTab("/docs/doc3.pdf", "doc3.pdf");
    expect(get(activeTab)?.id).toBe(id3);

    // Next from id3 wraps around to id1
    tabsStore.activateNextTab();
    expect(get(activeTab)?.id).toBe(id1);

    // Next to id2
    tabsStore.activateNextTab();
    expect(get(activeTab)?.id).toBe(id2);

    // Prev back to id1
    tabsStore.activatePrevTab();
    expect(get(activeTab)?.id).toBe(id1);

    // Prev from id1 wraps around to id3
    tabsStore.activatePrevTab();
    expect(get(activeTab)?.id).toBe(id3);
  });

  it("closes other tabs and keeps only target tab active", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    const id2 = tabsStore.openTab("/docs/doc2.pdf", "doc2.pdf");
    const id3 = tabsStore.openTab("/docs/doc3.pdf", "doc3.pdf");
    expect(get(tabCount)).toBe(3);

    tabsStore.closeOtherTabs(id2);
    expect(get(tabCount)).toBe(1);
    expect(get(activeTab)?.id).toBe(id2);
  });

  it("closes tabs to the right of target tab", () => {
    const id1 = tabsStore.openTab("/docs/doc1.pdf", "doc1.pdf");
    const id2 = tabsStore.openTab("/docs/doc2.pdf", "doc2.pdf");
    const id3 = tabsStore.openTab("/docs/doc3.pdf", "doc3.pdf");
    const id4 = tabsStore.openTab("/docs/doc4.pdf", "doc4.pdf");
    expect(get(tabCount)).toBe(4);

    // Close tabs to the right of id2 -> keeps id1 and id2
    tabsStore.closeTabsToRight(id2);
    expect(get(tabCount)).toBe(2);
    expect(get(activeTab)?.id).toBe(id2);
  });
});

