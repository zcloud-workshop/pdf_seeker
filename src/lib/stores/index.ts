import { derived, get, writable } from "svelte/store";

export type ViewName = "home" | "viewer" | "tools" | "compare" | "storage" | "settings";

export const currentView = writable<ViewName>("home");

export interface Tab {
  id: string;
  path: string;
  name: string;
}

export const tabs = writable<Tab[]>([]);
export const activeTabId = writable<string | null>(null);

let nextTabSeq = 1;

/** Open a file in a tab; if the same path is already open, activate that tab
 * (in-place edits in Tools make duplicate tabs on one file unsafe). */
export function openTab(path: string): Tab {
  const existing = get(tabs).find((t) => t.path === path);
  if (existing) {
    activeTabId.set(existing.id);
    return existing;
  }
  const tab: Tab = {
    id: `tab-${nextTabSeq++}`,
    path,
    name: path.split(/[\\/]/).pop() || "Untitled",
  };
  tabs.update((all) => [...all, tab]);
  activeTabId.set(tab.id);
  return tab;
}

export function closeTab(id: string) {
  const all = get(tabs);
  const idx = all.findIndex((t) => t.id === id);
  if (idx === -1) return;
  tabs.set(all.filter((t) => t.id !== id));
  if (get(activeTabId) === id) {
    const rest = get(tabs);
    const next = rest[Math.min(idx, rest.length - 1)];
    activeTabId.set(next ? next.id : null);
  }
}

export function activateTab(id: string) {
  if (get(tabs).some((t) => t.id === id)) {
    activeTabId.set(id);
  }
}

/** Active tab's file path — read by Tools/Toolbar/etc. Read-only. */
export const currentFilePath = derived(
  [tabs, activeTabId],
  ([$tabs, $activeId]) => $tabs.find((t) => t.id === $activeId)?.path ?? null,
);

export const currentFileName = derived(
  [tabs, activeTabId],
  ([$tabs, $activeId]) => $tabs.find((t) => t.id === $activeId)?.name ?? "",
);

export const zoomLevel = writable<number>(1.0);

export const currentPage = writable<number>(1);

export const totalPages = writable<number>(0);

export const isDark = writable<boolean>(false);

export const isFullscreen = writable<boolean>(false);

/** Viewer left panel: PDF outline (bookmarks) visibility */
export const outlineVisible = writable<boolean>(false);

export const sidebarCollapsed = writable<boolean>(false);
