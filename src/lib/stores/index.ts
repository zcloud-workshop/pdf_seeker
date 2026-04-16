import { writable } from "svelte/store";

export type ViewName = "home" | "viewer" | "tools" | "storage" | "settings";

export const currentView = writable<ViewName>("home");

export const currentFilePath = writable<string | null>(null);

export const currentFileName = writable<string>("");

export const zoomLevel = writable<number>(1.0);

export const currentPage = writable<number>(1);

export const totalPages = writable<number>(0);

export const isDark = writable<boolean>(false);

export const isFullscreen = writable<boolean>(false);

export const sidebarCollapsed = writable<boolean>(false);
