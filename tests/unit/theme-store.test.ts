import { describe, it, expect, beforeEach, vi } from "vitest";
import { get } from "svelte/store";
import {
  isDark,
  themePreference,
  setTheme,
  toggleTheme,
  initTheme,
  resolveIsDark,
  THEME_STORAGE_KEY,
} from "@/stores/theme";

describe("Persistent Theme Manager", () => {
  beforeEach(() => {
    localStorage.clear();
    document.documentElement.className = "";
  });

  it("resolves dark state accurately according to preference", () => {
    expect(resolveIsDark("dark")).toBe(true);
    expect(resolveIsDark("light")).toBe(false);
  });

  it("sets dark theme, updates store, persists to localStorage, and updates DOM", async () => {
    await setTheme("dark", false);

    expect(get(themePreference)).toBe("dark");
    expect(get(isDark)).toBe(true);
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("sets light theme, updates store, persists to localStorage, and updates DOM", async () => {
    // First set dark
    await setTheme("dark", false);
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    // Then switch to light
    await setTheme("light", false);
    expect(get(themePreference)).toBe("light");
    expect(get(isDark)).toBe(false);
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("toggles between dark and light modes reliably", async () => {
    await setTheme("light", false);
    expect(get(isDark)).toBe(false);

    await toggleTheme();
    expect(get(isDark)).toBe(true);
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);

    await toggleTheme();
    expect(get(isDark)).toBe(false);
    expect(localStorage.getItem(THEME_STORAGE_KEY)).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);
  });

  it("restores dark mode from localStorage on initTheme", async () => {
    localStorage.setItem(THEME_STORAGE_KEY, "dark");

    await initTheme();

    expect(get(themePreference)).toBe("dark");
    expect(get(isDark)).toBe(true);
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });
});
