import { writable, derived } from "svelte/store";

export interface TabState {
  id: string;
  filePath: string;
  fileName: string;
  isActive: boolean;
}

let tabIdCounter = 0;

function createTabId(): string {
  return `tab_${++tabIdCounter}_${Date.now()}`;
}

interface TabsState {
  tabs: TabState[];
  activeTabId: string | null;
}

function createInitialTabs(): TabsState {
  return { tabs: [], activeTabId: null };
}

function createTabsStore() {
  const { subscribe, set, update } = writable<TabsState>(createInitialTabs());

  function openTab(filePath: string, fileName: string): string {
    const tabId = createTabId();
    update((state) => {
      // If file already open, activate it
      const existing = state.tabs.find((t) => t.filePath === filePath);
      if (existing) {
        return {
          ...state,
          tabs: state.tabs.map((t) => ({ ...t, isActive: t.id === existing.id })),
          activeTabId: existing.id,
        };
      }
      const newTab: TabState = {
        id: tabId,
        filePath,
        fileName,
        isActive: true,
      };
      return {
        tabs: [...state.tabs.map((t) => ({ ...t, isActive: false })), newTab],
        activeTabId: tabId,
      };
    });
    return tabId;
  }

  function closeTab(tabId: string): void {
    update((state) => {
      const idx = state.tabs.findIndex((t) => t.id === tabId);
      if (idx < 0) return state;
      const newTabs = state.tabs.filter((t) => t.id !== tabId);
      if (state.activeTabId === tabId) {
        const nextIdx = Math.min(idx, newTabs.length - 1);
        const nextId = newTabs.length > 0 ? newTabs[nextIdx].id : null;
        return {
          tabs: newTabs.map((t) => ({ ...t, isActive: t.id === nextId })),
          activeTabId: nextId,
        };
      }
      return { ...state, tabs: newTabs };
    });
  }

  function activateTab(tabId: string): void {
    update((state) => ({
      ...state,
      tabs: state.tabs.map((t) => ({ ...t, isActive: t.id === tabId })),
      activeTabId: tabId,
    }));
  }

  function updateTabPath(tabId: string, filePath: string, fileName: string): void {
    update((state) => ({
      ...state,
      tabs: state.tabs.map((t) =>
        t.id === tabId ? { ...t, filePath, fileName } : t,
      ),
    }));
  }

  function closeActiveTab(): void {
    update((state) => {
      if (!state.activeTabId) return state;
      return closeTabInState(state, state.activeTabId);
    });
  }

  function activateNextTab(): void {
    update((state) => {
      const idx = state.tabs.findIndex((t) => t.id === state.activeTabId);
      if (idx < 0) return state;
      const nextIdx = (idx + 1) % state.tabs.length;
      const nextId = state.tabs[nextIdx].id;
      return {
        ...state,
        tabs: state.tabs.map((t) => ({ ...t, isActive: t.id === nextId })),
        activeTabId: nextId,
      };
    });
  }

  function activatePrevTab(): void {
    update((state) => {
      const idx = state.tabs.findIndex((t) => t.id === state.activeTabId);
      if (idx < 0) return state;
      const prevIdx = (idx - 1 + state.tabs.length) % state.tabs.length;
      const prevId = state.tabs[prevIdx].id;
      return {
        ...state,
        tabs: state.tabs.map((t) => ({ ...t, isActive: t.id === prevId })),
        activeTabId: prevId,
      };
    });
  }

  function closeOtherTabs(tabId: string): void {
    update((state) => {
      const tab = state.tabs.find((t) => t.id === tabId);
      if (!tab) return state;
      return {
        tabs: [{ ...tab, isActive: true }],
        activeTabId: tab.id,
      };
    });
  }

  function closeTabsToRight(tabId: string): void {
    update((state) => {
      const idx = state.tabs.findIndex((t) => t.id === tabId);
      if (idx < 0) return state;
      const keepTabs = state.tabs.slice(0, idx + 1);
      const activeKept = keepTabs.some((t) => t.id === state.activeTabId);
      const activeTabId = activeKept ? state.activeTabId : tabId;
      return {
        tabs: keepTabs.map((t) => ({ ...t, isActive: t.id === activeTabId })),
        activeTabId,
      };
    });
  }

  function clearTabs(): void {
    set(createInitialTabs());
  }

  function getActiveTab(state: TabsState): TabState | null {
    return state.tabs.find((t) => t.id === state.activeTabId) ?? null;
  }

  return {
    subscribe,
    set,
    update,
    openTab,
    closeTab,
    closeOtherTabs,
    closeTabsToRight,
    activateTab,
    updateTabPath,
    closeActiveTab,
    activateNextTab,
    activatePrevTab,
    clearTabs,
    getActiveTab,
  };
}

function closeTabInState(state: TabsState, tabId: string): TabsState {
  const idx = state.tabs.findIndex((t) => t.id === tabId);
  if (idx < 0) return state;
  const newTabs = state.tabs.filter((t) => t.id !== tabId);
  if (state.activeTabId === tabId) {
    const nextIdx = Math.min(idx, newTabs.length - 1);
    const nextId = newTabs.length > 0 ? newTabs[nextIdx].id : null;
    return {
      tabs: newTabs.map((t) => ({ ...t, isActive: t.id === nextId })),
      activeTabId: nextId,
    };
  }
  return { ...state, tabs: newTabs };
}

export const tabsStore = createTabsStore();

export const activeTab = derived(tabsStore, (state) =>
  state.tabs.find((t) => t.id === state.activeTabId) ?? null,
);

export const tabCount = derived(tabsStore, (state) => state.tabs.length);
