import { writable, derived } from "svelte/store";

export type ViewName = "home" | "editor" | "tools" | "storage" | "settings";
export type ToolTabMode = "tools" | "batch" | "compress" | "privacy";

export const currentView = writable<ViewName>("home");
export const activeToolTab = writable<ToolTabMode>("tools");

export const currentFilePath = writable<string | null>(null);

export const currentFileName = writable<string>("");

export const zoomLevel = writable<number>(1.0);

export const currentPage = writable<number>(1);

export const totalPages = writable<number>(0);

export {
  isDark,
  themePreference,
  initTheme,
  setTheme,
  toggleTheme,
  type ThemePreference,
} from "./theme";

export const isFullscreen = writable<boolean>(false);

export const sidebarCollapsed = writable<boolean>(false);

// Edit history for undo/redo
import { createEditHistory, canUndo, canRedo, type EditHistory } from "../edit-history";

export const editHistory = writable<EditHistory>(createEditHistory());
export const canUndoStore = derived(editHistory, h => canUndo(h));
export const canRedoStore = derived(editHistory, h => canRedo(h));

// Editor selection state (shared between CanvasEditor and Editor)
export const selectedEditId = writable<string | null>(null);
export const signPlacement = writable<{ active: boolean; imagePath: string } | null>(null);

// OCR results per page: { pageNum: OcrBlock[] }
export interface OcrBlock {
  points: [number, number][];
  text: string;
  confidence: number;
}
export const ocrResults = writable<Record<number, OcrBlock[]>>({});
