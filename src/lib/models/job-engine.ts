/**
 * Unified Job Engine & Batch Processing Models (Phase 1 & Phase 2)
 */

export type RecipeType =
  | "rotate"
  | "watermark"
  | "pdf2text"
  | "pdf2img"
  | "compress"
  | "extract";

export interface RotateRecipeParams {
  angle: 90 | 180 | 270;
}

export interface WatermarkRecipeParams {
  text: string;
  fontSize: number;
  opacity: number;
  color: string;
  angle: number;
}

export interface Pdf2TextRecipeParams {
  scope: "all" | "first" | "custom";
  customRange?: string;
}

export interface Pdf2ImgRecipeParams {
  scale: 1 | 1.5 | 2;
  format: "png" | "jpeg";
}

export interface CompressRecipeParams {
  preset: "balanced" | "high_quality" | "max_compression" | "grayscale";
}

export interface ExtractRecipeParams {
  pageRange: string;
}

export type RecipeParams =
  | RotateRecipeParams
  | WatermarkRecipeParams
  | Pdf2TextRecipeParams
  | Pdf2ImgRecipeParams
  | CompressRecipeParams
  | ExtractRecipeParams;

export interface RecipeSpec {
  id: string;
  name: string;
  description: string;
  type: RecipeType;
  params: RecipeParams;
}

export type ConflictStrategy = "rename" | "replace" | "fail";

export interface OutputPolicy {
  outputDir: string; // "same" or specific directory
  namingTemplate: string; // e.g. "{name}_{suffix}"
  conflictStrategy: ConflictStrategy;
}

export interface BatchInputFile {
  path: string;
  name: string;
  sizeBytes: number;
  pageCount?: number;
}

export type ItemStatus = "pending" | "running" | "done" | "error" | "cancelled" | "skipped";

export interface JobItemResult {
  filePath: string;
  fileName: string;
  fileSizeBytes: number;
  status: ItemStatus;
  outputPath?: string;
  outputSizeBytes?: number;
  ratio?: number;
  durationMs?: number;
  message?: string;
  error?: string;
}

export interface JobSummary {
  id: string;
  recipeName: string;
  recipeType: RecipeType;
  totalFiles: number;
  completedFiles: number;
  failedFiles: number;
  skippedFiles: number;
  startTime: number;
  endTime?: number;
  totalOriginalBytes: number;
  totalOutputBytes: number;
  bytesSaved: number;
  items: JobItemResult[];
}

/**
 * Resolves the destination output file path given naming templates and conflict strategy.
 */
export function resolveOutputPath(
  inputPath: string,
  outputDir: string,
  template: string,
  suffix: string,
  ext: string = "pdf",
  conflictStrategy: ConflictStrategy = "rename",
  existingFiles: Set<string> = new Set()
): string {
  const normPath = inputPath.replace(/\\/g, "/");
  const lastSlash = normPath.lastIndexOf("/");
  const originalDir = lastSlash >= 0 ? normPath.substring(0, lastSlash) : ".";
  const fullName = lastSlash >= 0 ? normPath.substring(lastSlash + 1) : normPath;
  const rawName = fullName.replace(/\.[^/.]+$/, "");

  const targetDir = outputDir && outputDir !== "same" ? outputDir.replace(/\\/g, "/") : originalDir;

  let baseOutputName = template
    .replace(/{name}/g, rawName)
    .replace(/{suffix}/g, suffix)
    .replace(/{date}/g, new Date().toISOString().slice(0, 10));

  if (!baseOutputName.includes(suffix) && !template.includes("{suffix}")) {
    baseOutputName = `${baseOutputName}_${suffix}`;
  }

  const cleanExt = ext.startsWith(".") ? ext.slice(1) : ext;
  let candidate = `${targetDir}/${baseOutputName}.${cleanExt}`;

  if (conflictStrategy === "replace") {
    return candidate;
  }

  if (conflictStrategy === "fail" && existingFiles.has(candidate)) {
    throw new Error(`Output file already exists: ${candidate}`);
  }

  // Handle "rename" conflict
  let counter = 1;
  while (existingFiles.has(candidate)) {
    candidate = `${targetDir}/${baseOutputName}_${counter}.${cleanExt}`;
    counter++;
  }

  return candidate;
}

/**
 * Format bytes to readable human strings.
 */
export function formatBytes(bytes: number): string {
  if (isNaN(bytes) || bytes < 0) return "0 B";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

/**
 * Generate report in CSV or Markdown format
 */
export function generateJobReport(summary: JobSummary, format: "csv" | "markdown"): string {
  if (format === "csv") {
    const headers = ["Index", "File Name", "Status", "Original Size", "Output Size", "Saved %", "Duration(ms)", "Output Path", "Details"];
    const rows = summary.items.map((item, idx) => [
      idx + 1,
      `"${item.fileName.replace(/"/g, '""')}"`,
      item.status,
      item.fileSizeBytes,
      item.outputSizeBytes || 0,
      item.ratio ? `${item.ratio.toFixed(1)}%` : "0%",
      item.durationMs || 0,
      `"${(item.outputPath || "").replace(/"/g, '""')}"`,
      `"${(item.error || item.message || "").replace(/"/g, '""')}"`,
    ]);
    return [headers.join(","), ...rows.map(r => r.join(","))].join("\n");
  }

  // Markdown format
  const durationSec = summary.endTime ? ((summary.endTime - summary.startTime) / 1000).toFixed(1) : "0";
  const lines: string[] = [
    `# PDF Seeker 批处理任务报告`,
    ``,
    `- **任务配方**: ${summary.recipeName} (${summary.recipeType})`,
    `- **处理文件总数**: ${summary.totalFiles} (成功: ${summary.completedFiles}, 失败: ${summary.failedFiles}, 跳过: ${summary.skippedFiles})`,
    `- **总耗时**: ${durationSec} 秒`,
    `- **总原始大小**: ${formatBytes(summary.totalOriginalBytes)}`,
    `- **总产物大小**: ${formatBytes(summary.totalOutputBytes)}`,
    `- **节省空间**: ${formatBytes(summary.bytesSaved)}`,
    ``,
    `| # | 文件名 | 状态 | 原始大小 | 产物大小 | 耗时 | 输出详情 |`,
    `|---|---|---|---|---|---|---|`,
  ];

  summary.items.forEach((item, idx) => {
    const orig = formatBytes(item.fileSizeBytes);
    const out = item.outputSizeBytes ? formatBytes(item.outputSizeBytes) : "-";
    const dur = item.durationMs ? `${item.durationMs}ms` : "-";
    const detail = item.error ? `⚠️ ${item.error}` : (item.message || "成功");
    lines.push(`| ${idx + 1} | \`${item.fileName}\` | **${item.status}** | ${orig} | ${out} | ${dur} | ${detail} |`);
  });

  return lines.join("\n");
}

