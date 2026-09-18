# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

PDF Seeker is a cross-platform, offline-first PDF toolbox built with **Tauri 2 + Svelte 5 + Rust**. Desktop app ~10MB, no Electron. All PDF processing uses `lopdf` (Rust) and `pdfjs-dist` (frontend rendering).

## Build & Development Commands

```bash
npm run tauri dev          # Full dev (frontend + backend hot reload)
npm run dev                # Frontend only (browser at localhost:1420, no backend)
npm run tauri build        # Build for current platform
npm run check              # Svelte/TypeScript type checking
cd src-tauri && cargo check   # Rust compilation check
cd src-tauri && cargo test    # Rust tests
cd src-tauri && cargo clippy  # Rust linting
```

## Architecture

### Path Aliases
- `@/` → `src/lib/`
- `$views/` → `src/views/`

### Frontend-Backend Communication
- Frontend calls Rust via `invoke("command_name", { req: {...} })`
- Commands registered in `src-tauri/src/lib.rs` via `tauri::generate_handler![]`
- All commands return `AppResult<T>` (see `src-tauri/src/error.rs`)
- Tauri plugins used: `tauri-plugin-dialog` (file open/save), `tauri-plugin-fs` (file read/write), `tauri-plugin-shell`

### View Routing

Simple key-based routing via `currentView` store (`ViewName = "home" | "editor" | "storage" | "settings"`). App.svelte maps the store value to a component using `{#key $currentView}`.

```
App.svelte
├── Sidebar.svelte (navigation + shortcuts button)
├── Toolbar.svelte (app actions, theme toggle)
└── main router:
    ├── Home.svelte (file open, drag-drop, recent files)
    ├── Editor.svelte (unified PDF viewing + editing + tools)
    ├── Storage.svelte (S3 file browser)
    └── Settings.svelte
```

### Editor Architecture

Editor.svelte (~870 lines) is the main view combining PDF rendering and all tools. Key sub-components in `src/lib/components/editor/`:

- **ToolbarTabs.svelte** — Tool category tabs (page/edit/convert/extract) + tool buttons. Defines `ToolCategory` and `ToolId` types.
- **CanvasEditor.svelte** — Overlay on each PDF page for drawing/editing. Handles pointer events for text placement, rectangle/highlight drawing, element selection, dragging, and sign placement. Dispatches `signplace` custom DOM event for parent.
- **TextLayer.svelte** — pdfjs text overlay for mouse-based text selection/editing.
- **OcrOverlay.svelte** — Displays OCR recognition results as editable blocks.
- **StatusBar.svelte** — Page navigation, zoom controls.
- **WatermarkDialog.svelte** — Watermark parameter dialog.
- **PageToolsDialog.svelte** — Routes page-level tools (merge, split, rotate, delete, extract, reorder, img2pdf, pdf2img, pdf2text, extractText, table) to their respective tool components.
- **ThumbnailSidebar.svelte** — Page thumbnail panel with virtual scrolling, click-to-navigate, and drag-to-reorder. Collapsible sidebar.
- **TabBar.svelte** — Multi-tab bar for open PDF files with Ctrl+Tab/Ctrl+Shift+Tab/Ctrl+W shortcuts.

**Tool dispatch flow**: `ToolbarTabs` fires `handleToolAction(toolId)` in Editor.svelte. Page tools open `PageToolsDialog`, watermark opens `WatermarkDialog`, sign triggers file picker, OCR triggers recognition pipeline, and edit tools (editText/editRect/editHighlight) toggle `activeTool` state passed to `CanvasEditor`.

### Tool Components (`src/lib/components/tools/`)

Each tool component receives `{ filePath = $bindable() }`. Used by PageToolsDialog for page-level operations. Components: MergeTool, SplitTool, RotateTool, DeletePagesTool, ExtractPagesTool, ReorderTool, WatermarkTool, Img2PdfTool, Pdf2ImgTool, Pdf2TextTool, SignTool, OcrTool, EditTool, BatchTool.

### State Management (`src/lib/stores/index.ts`)

Key stores: `currentView`, `currentFilePath`, `currentFileName`, `zoomLevel`, `currentPage`, `totalPages`, `isDark`, `isFullscreen`, `sidebarCollapsed`, `editHistory`, `canUndoStore`, `canRedoStore`, `selectedEditId`, `signPlacement`, `ocrResults`.

### Tab System (`src/lib/stores/tabs.ts`)

Multi-tab state management. `TabState` tracks per-file: `id`, `filePath`, `fileName`, `isActive`. Store exposes `openTab`, `closeTab`, `switchTab` operations. `TabBar.svelte` renders the tab strip.

### Edit System (`src/lib/edit-history.ts`)

Lightweight JSON-based undo/redo. Operations pushed to `editHistory` store with types: `addText`, `addRectangle`, `addHighlight`, `addWatermark`, `addSignature`. No PDF docs kept in memory — "Save All" calls `apply_edit_operations` Rust command to batch-apply. `Ctrl+Z`/`Ctrl+Shift+Z` keyboard shortcuts.

### PDF Rendering (Editor.svelte)

- Uses pdfjs-dist via `src/lib/pdf-engine.ts` (`loadPdf`, `getPageViewport`)
- Virtual scrolling with IntersectionObserver (`rootMargin: 400px`) for lazy page rendering
- Render queue with `MAX_CONCURRENT_RENDERS = 4`
- Page cache keyed by `pageNum@zoom` — cleared on zoom changes
- Dark mode: canvas-level `rgba(0,0,0,0.15)` overlay (preserves element colors)
- Ctrl+scroll wheel zoom with cursor-center anchoring

### Rust Backend

- `src-tauri/src/error.rs` — `AppError` enum (Config, Pdf, Io, S3 variants), `AppResult<T>` alias
- `src-tauri/src/config/` — TOML-based app config with `directories` crate for cross-platform paths
- `src-tauri/src/commands/`:
  - `pdf_ops.rs` — All PDF ops via lopdf: merge, split, rotate, delete_pages, extract_pages, compress, add_text_watermark, add_text_to_page, add_rectangle, add_highlight, apply_edit_operations, reorder_pages, sign_pdf, ocr_recognize, images_to_pdf, pdf_to_images, pdf_to_text, get_temp_dir, save_image_file, insert_pages
  - `pdf_content.rs` — Shared helpers for PDF page resource/content stream manipulation (`get_or_create_resources`, font dictionary management). Extracted from pdf_ops to reduce duplication.
  - `validation.rs` — Path validation utilities (`validate_path`, `validate_output_path`) to prevent directory traversal. Rejects `..` components and canonicalizes paths.
  - `ocr.rs` — OCR engine management, model scanning/validation/download
  - `config.rs` — get_config, update_config
  - `recent.rs` — get_recent_files, add_recent_file, clear_recent_files
  - `s3_ops.rs` — S3 operations (list, upload, download, delete, versions, presigned URL, connection test)

### Internationalization

Locales in `src/lib/i18n/locales/` (en.json, zh.json). Accessed via `$t("key")` from `@/i18n/index.svelte.ts`.

### UI Components (`src/lib/components/ui/`)

Primitive components: Button, Input, Label, Separator, Tooltip. Built with Tailwind + `tailwind-variants`. Icons from `lucide-svelte`.

## Key Patterns

**Adding a new tool**:
1. Create `src/lib/components/tools/NewTool.svelte` with `{ filePath = $bindable() }` prop
2. Add to `src/lib/components/tools/index.ts` barrel export
3. Add `ToolId` member in `src/lib/components/editor/ToolbarTabs.svelte`
4. Add tool entry in the `tools` array in `ToolbarTabs.svelte` with appropriate category
5. Handle the new ToolId in `handleToolAction()` in `Editor.svelte` (route to PageToolsDialog or add custom logic)
6. If routed via PageToolsDialog, add case in its tool-to-component mapping
7. Add i18n keys in `en.json` and `zh.json` (label + desc)
8. If it needs a Rust command: add handler in `pdf_ops.rs`, register in `lib.rs`

**Adding a new S3 operation**:
1. Add command in `src-tauri/src/commands/s3_ops.rs`
2. Register in `src-tauri/src/lib.rs`
3. Wire up in `src/views/Storage.svelte`

## Pre-existing Issues (not introduced by current work)
- `src/views/Home.svelte:25` — `path` on `File` type (TypeScript strict)
- `src/views/Settings.svelte:5` — module resolution for `@/lib/types`

## macOS Deployment
```bash
xattr -c /Applications/PDF_Seeker.app
```
