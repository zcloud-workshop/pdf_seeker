import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "@/types";

export type ThemePreference = "system" | "light" | "dark";

export const themePreference = writable<ThemePreference>("system");
export const isDark = writable<boolean>(false);

export const THEME_STORAGE_KEY = "pdf_seeker_theme";

function applyThemeToDOM(dark: boolean) {
  if (typeof document !== "undefined" && document.documentElement) {
    document.documentElement.classList.toggle("dark", dark);
  }
}

let mediaQueryListener: ((e: MediaQueryListEvent) => void) | null = null;

function updateMediaListener(pref: ThemePreference) {
  if (typeof window === "undefined" || !window.matchMedia) return;

  try {
    const mq = window.matchMedia("(prefers-color-scheme: dark)");

    if (mediaQueryListener) {
      mq.removeEventListener("change", mediaQueryListener);
      mediaQueryListener = null;
    }

    if (pref === "system") {
      mediaQueryListener = (e: MediaQueryListEvent) => {
        isDark.set(e.matches);
        applyThemeToDOM(e.matches);
      };
      mq.addEventListener("change", mediaQueryListener);
    }
  } catch (_) {}
}

export function resolveIsDark(pref: ThemePreference): boolean {
  if (pref === "dark") return true;
  if (pref === "light") return false;
  if (typeof window !== "undefined" && window.matchMedia) {
    try {
      return window.matchMedia("(prefers-color-scheme: dark)").matches;
    } catch (_) {
      return false;
    }
  }
  return false;
}

/**
 * Initialize theme on app startup:
 * 1. Read from localStorage for immediate, zero-latency rendering.
 * 2. Sync with backend config.general.theme if available.
 */
export async function initTheme(): Promise<void> {
  let savedPref: ThemePreference = "system";

  try {
    const local = localStorage.getItem(THEME_STORAGE_KEY) as ThemePreference | null;
    if (local === "system" || local === "light" || local === "dark") {
      savedPref = local;
    }
  } catch (_) {}

  // Apply immediately from local storage
  themePreference.set(savedPref);
  const dark = resolveIsDark(savedPref);
  isDark.set(dark);
  applyThemeToDOM(dark);
  updateMediaListener(savedPref);

  // Sync with backend config if running in Tauri environment
  try {
    const config = await invoke<AppConfig>("get_config");
    const backendTheme = config?.general?.theme as ThemePreference | undefined;
    if (backendTheme && (backendTheme === "system" || backendTheme === "light" || backendTheme === "dark")) {
      if (backendTheme !== savedPref) {
        await setTheme(backendTheme, false);
      }
    }
  } catch (_) {}
}

/**
 * Set theme explicitly ("system" | "light" | "dark").
 */
export async function setTheme(pref: ThemePreference, persistBackend: boolean = true): Promise<void> {
  themePreference.set(pref);
  try {
    localStorage.setItem(THEME_STORAGE_KEY, pref);
  } catch (_) {}

  const dark = resolveIsDark(pref);
  isDark.set(dark);
  applyThemeToDOM(dark);
  updateMediaListener(pref);

  if (persistBackend) {
    try {
      const config = await invoke<AppConfig>("get_config");
      if (config && config.general && config.general.theme !== pref) {
        config.general.theme = pref;
        await invoke("update_config", { newConfig: config });
      }
    } catch (_) {}
  }
}

/**
 * Toggle between light and dark mode.
 */
export async function toggleTheme(): Promise<void> {
  const current = get(isDark);
  const next = !current;
  await setTheme(next ? "dark" : "light", true);
}
