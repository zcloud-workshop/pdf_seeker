/**
 * Controllable Compression Presets and Utilities (Phase 2)
 */

export interface CompressionPresetInfo {
  id: "balanced" | "high_quality" | "max_compression" | "grayscale";
  name: string;
  nameEn: string;
  badge: string;
  description: string;
  estimatedSavingMin: number; // percentage, e.g. 20
  estimatedSavingMax: number; // percentage, e.g. 50
  features: string[];
}

export const COMPRESSION_PRESETS: Record<CompressionPresetInfo["id"], CompressionPresetInfo> = {
  balanced: {
    id: "balanced",
    name: "日常推荐 (平衡)",
    nameEn: "Balanced (Recommended)",
    badge: "推荐",
    description: "兼顾清晰度与传输便利，智能优化对象流与嵌入字体，保留高保真矢量与文字锐度。",
    estimatedSavingMin: 25,
    estimatedSavingMax: 45,
    features: [
      "Flate Deflate 流无损重压缩",
      "合并与去重同名字体资源",
      "清理无用游离对象 (Scrub Orphaned Objects)",
      "保留最高清晰度矢量与文字排版",
    ],
  },
  high_quality: {
    id: "high_quality",
    name: "高保真 (印刷级)",
    nameEn: "High Quality (Print)",
    badge: "无损",
    description: "专为正式公文、设计图纸与高精度印刷准备，绝不损失任何色彩或图像细节。",
    estimatedSavingMin: 10,
    estimatedSavingMax: 25,
    features: [
      "严格无损 Flate 编码压制",
      "完全保留原始 ICC 色彩配置",
      "保留高清印刷图片原生 DPI",
      "精细清理 PDF 交叉引用冗余表",
    ],
  },
  max_compression: {
    id: "max_compression",
    name: "极小体积 (网络/邮件)",
    nameEn: "Maximum Compression (Web/Email)",
    badge: "极致",
    description: "最大力度缩减体积，重排对象流并剔除非关键元数据，适合邮件附件与微信快传。",
    estimatedSavingMin: 40,
    estimatedSavingMax: 70,
    features: [
      "激进多流重叠合并 (Cross-reference stream)",
      "剥离冗余私有应用元数据 (Scrub private tags)",
      "最大化字典与标注流精简",
      "优化移动设备极速载入",
    ],
  },
  grayscale: {
    id: "grayscale",
    name: "黑白/灰度归档",
    nameEn: "Grayscale (Archive)",
    badge: "归档",
    description: "针对扫描合同、发票凭据与纯文献归档设计，剔除多余彩色通道与冗余历史信息。",
    estimatedSavingMin: 35,
    estimatedSavingMax: 65,
    features: [
      "优化双色/灰度流压缩结构",
      "适合报销单、票据与合同长期存盘",
      "规范化页面树节点层次",
      "降低长期归档存储空间",
    ],
  },
};

/**
 * Calculate expected saving range in bytes
 */
export function estimateSavings(originalBytes: number, presetId: CompressionPresetInfo["id"]) {
  const preset = COMPRESSION_PRESETS[presetId] || COMPRESSION_PRESETS.balanced;
  const minSaved = Math.round((originalBytes * preset.estimatedSavingMin) / 100);
  const maxSaved = Math.round((originalBytes * preset.estimatedSavingMax) / 100);
  const minFinal = Math.max(1, originalBytes - maxSaved);
  const maxFinal = Math.max(1, originalBytes - minSaved);

  return {
    minSaved,
    maxSaved,
    minFinal,
    maxFinal,
    ratioRange: `${preset.estimatedSavingMin}% ~ ${preset.estimatedSavingMax}%`,
  };
}

/**
 * Format compression comparison string
 */
export function formatCompressionSummary(originalBytes: number, compressedBytes: number): {
  ratioPercent: number;
  bytesSaved: number;
  summaryText: string;
} {
  const bytesSaved = Math.max(0, originalBytes - compressedBytes);
  const ratioPercent = originalBytes > 0 ? ((originalBytes - compressedBytes) / originalBytes) * 100 : 0;

  return {
    ratioPercent,
    bytesSaved,
    summaryText: `节省 ${ratioPercent.toFixed(1)}% (减少 ${formatBytes(bytesSaved)})`,
  };
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`;
}

