import { invoke } from "@tauri-apps/api/core";
import type { AppConfig } from "@/types";
import { setLocale } from "@/i18n/index.svelte.ts";
import { isDark } from "@/stores";
import { writable } from "svelte/store";

type ThemePreference = "system" | "light" | "dark";

let themePreference: ThemePreference = "system";
let stopSystemThemeListener: (() => void) | null = null;
export const themeSaveError = writable<string | null>(null);
export const activeThemePreference = writable<ThemePreference>("system");

function applyDarkState(dark: boolean) {
  isDark.set(dark);
  document.documentElement.classList.toggle("dark", dark);
  document.documentElement.style.colorScheme = dark ? "dark" : "light";
}

export function applyConfig(config: AppConfig) {
  const language = config.general.language === "en" ? "en" : "zh";
  const theme = config.general.theme;
  themePreference = theme === "dark" || theme === "light" ? theme : "system";
  activeThemePreference.set(themePreference);
  setLocale(language);
  document.documentElement.lang = language;
  applyThemePreference();
}

function applyThemePreference() {
  const dark =
    themePreference === "dark" ||
    (themePreference === "system" &&
      window.matchMedia("(prefers-color-scheme: dark)").matches);
  applyDarkState(dark);
}

export function listenForSystemTheme(): () => void {
  if (stopSystemThemeListener) return stopSystemThemeListener;
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  const handleChange = () => {
    if (themePreference === "system") applyDarkState(media.matches);
  };
  media.addEventListener("change", handleChange);
  handleChange();
  const cleanup = () => {
    media.removeEventListener("change", handleChange);
    stopSystemThemeListener = null;
  };
  stopSystemThemeListener = cleanup;
  return cleanup;
}

export async function loadAndApplyConfig(): Promise<AppConfig> {
  const config = await invoke<AppConfig>("get_config");
  applyConfig(config);
  return config;
}

export async function saveAndApplyConfig(config: AppConfig): Promise<void> {
  await invoke("update_config", { newConfig: config });
  applyConfig(config);
}

export async function setThemePreference(next: "light" | "dark"): Promise<void> {
  try {
    const updated = await invoke<AppConfig>("set_theme_preference", { theme: next });
    applyConfig(updated);
    themeSaveError.set(null);
  } catch (error) {
    themeSaveError.set(String(error));
    throw error;
  }
}
