<script lang="ts">
  import { t } from "@/i18n/index.svelte.ts";
  import type { ToolId, ToolCategory } from "./ToolbarTabs.svelte";
  import {
    Search,
    X,
    Layers,
    Pencil,
    ArrowLeftRight,
    Shield,
    RotateCw,
    Merge,
    Split,
    Trash2,
    FileOutput,
    ArrowUpDown,
    FilePlus,
    Hash,
    Type,
    Square,
    Highlighter,
    PenTool,
    Stamp,
    ScanText,
    Image,
    Images,
    FileText,
    Table,
    ShieldAlert,
    Minimize2,
    Sparkles,
    Crop,
    BookOpen,
    Tag,
    Lock,
    Unlock,
    FileCode,
    Code,
    Copy,
    FileSpreadsheet,
    Book,
    FileUp,
    FileDown,
    Palette,
    Brush,
    ShieldCheck,
    Eye,
    Contrast,
    LayoutGrid,
    Wrench,
    FileSignature,
    Files,
  } from "lucide-svelte";

  let {
    visible = $bindable(false),
    onselecttool,
  }: {
    visible: boolean;
    onselecttool: (toolId: ToolId) => void;
  } = $props();

  let searchQuery = $state("");
  let selectedCategory = $state<"all" | ToolCategory>("all");
  let inputEl = $state<HTMLInputElement | null>(null);

  interface ToolDef {
    id: ToolId;
    labelKey?: string;
    descKey?: string;
    label?: string;
    desc?: string;
    category: ToolCategory;
    icon: any;
    color: string;
    badge?: string;
    keywords: string[];
  }

  const allTools: ToolDef[] = [
    // ════════════ 1. 页面整理与排版 (12 tools) ════════════
    {
      id: "rotate",
      labelKey: "tools.rotate",
      descKey: "tools.rotateDesc",
      label: "旋转页面",
      desc: "顺时针或逆时针旋转当前页或全部页面 90°/180°/270°",
      category: "page",
      icon: RotateCw,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["rotate", "spin", "angle", "orientation", "旋转", "方向", "翻转"],
    },
    {
      id: "reorder",
      labelKey: "tools.reorder",
      descKey: "tools.reorderDesc",
      label: "页面重排",
      desc: "可视化拖拽重新排列页面先后顺序",
      category: "page",
      icon: ArrowUpDown,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["sort", "reorder", "arrange", "drag", "顺序", "排序", "重排"],
    },
    {
      id: "merge",
      labelKey: "tools.merge",
      descKey: "tools.mergeDesc",
      label: "合并 PDF",
      desc: "将多个 PDF 文档按指定顺序合并为一个文件",
      category: "page",
      icon: Merge,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["combine", "join", "merge", "concat", "合并", "拼接", "组合"],
    },
    {
      id: "split",
      labelKey: "tools.split",
      descKey: "tools.splitDesc",
      label: "拆分 PDF",
      desc: "按单页或页码区间将文档拆分为多个独立文件",
      category: "page",
      icon: Split,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["split", "separate", "divide", "range", "拆分", "切分", "分割"],
    },
    {
      id: "extractPages",
      labelKey: "tools.extractPages",
      descKey: "tools.extractDesc",
      label: "提取页面",
      desc: "抽取选中的关键页面独立保存为新 PDF",
      category: "page",
      icon: FileOutput,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["extract", "save pages", "select", "提取页面", "另存页面"],
    },
    {
      id: "delete",
      labelKey: "tools.deletePages",
      descKey: "tools.deleteDesc",
      label: "删除页面",
      desc: "快速移除无用页面或空白页",
      category: "page",
      icon: Trash2,
      color: "text-rose-500 bg-rose-500/10 border-rose-500/20",
      keywords: ["delete", "remove", "drop", "删除", "移除页面"],
    },
    {
      id: "insertPages",
      labelKey: "tools.insertPages",
      descKey: "tools.insertPagesDesc",
      label: "插入页面",
      desc: "在当前文档指定位置插入其他 PDF 或空白页",
      category: "page",
      icon: FilePlus,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["insert", "add page", "append", "插入页面", "加页", "空白页"],
    },
    {
      id: "crop",
      label: "交互式页面裁剪",
      desc: "对标 Cropper.js，可视化拖拽框选裁切页边距或去除白边",
      category: "page",
      icon: Crop,
      color: "text-indigo-500 bg-indigo-500/10 border-indigo-500/20",
      badge: "Cropper",
      keywords: ["crop", "trim", "margin", "裁剪", "裁切", "切边", "去白边"],
    },
    {
      id: "pageNumber",
      labelKey: "tools.pageNumber",
      descKey: "tools.pageNumberDesc",
      label: "添加页码",
      desc: "批量注入格式化页眉、页脚及自定义连续页码",
      category: "page",
      icon: Hash,
      color: "text-indigo-500 bg-indigo-500/10 border-indigo-500/20",
      badge: "Stirling",
      keywords: ["page number", "header", "footer", "pagination", "页码", "编号", "页脚", "页眉"],
    },
    {
      id: "nup",
      label: "多页拼合 (N-up)",
      desc: "将 2 页或 4 页排版拼接打印在同一张纸上",
      category: "page",
      icon: LayoutGrid,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      badge: "Print",
      keywords: ["nup", "2-up", "4-up", "print layout", "拼版", "多页合一", "多合一"],
    },
    {
      id: "reverse",
      label: "页面倒序",
      desc: "将整个 PDF 文档的页面顺序完全颠倒反转",
      category: "page",
      icon: ArrowUpDown,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["reverse", "flip order", "倒序", "反转顺序"],
    },
    {
      id: "duplicate",
      label: "页面复制",
      desc: "快速将指定页面或表单连续复制多份",
      category: "page",
      icon: Copy,
      color: "text-blue-500 bg-blue-500/10 border-blue-500/20",
      keywords: ["duplicate", "clone", "copy page", "复制页面", "倍增"],
    },

    // ════════════ 2. 标注编辑与签名 (10 tools) ════════════
    {
      id: "editText",
      labelKey: "tools.editText",
      descKey: "tools.editTextDesc",
      label: "添加文字",
      desc: "在页面任意位置置入矢量文本或批注备注",
      category: "edit",
      icon: Type,
      color: "text-emerald-500 bg-emerald-500/10 border-emerald-500/20",
      keywords: ["text", "write", "type", "font", "文字", "文本", "打字", "输入"],
    },
    {
      id: "editRect",
      labelKey: "tools.editRect",
      descKey: "tools.editRectDesc",
      label: "矩形几何框",
      desc: "绘制矢量矩形圈注框，支持颜色与边框粗细",
      category: "edit",
      icon: Square,
      color: "text-emerald-500 bg-emerald-500/10 border-emerald-500/20",
      keywords: ["rectangle", "shape", "box", "border", "矩形", "方框", "几何"],
    },
    {
      id: "editHighlight",
      labelKey: "tools.editHighlight",
      descKey: "tools.editHighlightDesc",
      label: "荧光笔高亮",
      desc: "半透明黄色荧光笔划线标记重要重点",
      category: "edit",
      icon: Highlighter,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      keywords: ["highlight", "marker", "fluorescent", "高亮", "标黄", "荧光笔"],
    },
    {
      id: "sign",
      labelKey: "tools.sign",
      descKey: "tools.signDesc",
      label: "手写签名",
      desc: "电子签名板盖印签名或导入印章图片",
      category: "edit",
      icon: PenTool,
      color: "text-emerald-500 bg-emerald-500/10 border-emerald-500/20",
      keywords: ["sign", "signature", "stamp", "autograph", "签名", "签署", "印章"],
    },
    {
      id: "watermark",
      labelKey: "tools.watermark",
      descKey: "tools.watermarkDesc",
      label: "防伪文字水印",
      desc: "全文档批量注入倾斜半透明文字防伪标记",
      category: "edit",
      icon: Stamp,
      color: "text-cyan-500 bg-cyan-500/10 border-cyan-500/20",
      keywords: ["watermark", "stamp", "security", "copyright", "水印", "版权", "防伪"],
    },
    {
      id: "imageWatermark",
      label: "图片 Logo 水印",
      desc: "叠加公司印章、Logo 或机构图章透明图层",
      category: "edit",
      icon: Stamp,
      color: "text-cyan-500 bg-cyan-500/10 border-cyan-500/20",
      keywords: ["logo watermark", "image stamp", "图片水印", "图章水印", "印章"],
    },
    {
      id: "ocrEdit",
      labelKey: "tools.ocrEdit",
      descKey: "tools.ocrEditDesc",
      label: "OCR 文字识别",
      desc: "本地离线模型高精度文字识别与就地编辑",
      category: "edit",
      icon: ScanText,
      color: "text-violet-500 bg-violet-500/10 border-violet-500/20",
      badge: "AI/OCR",
      keywords: ["ocr", "recognize", "replace", "scan", "文字识别", "离线模型", "替换"],
    },
    {
      id: "searchablePdf",
      label: "双层可搜索 PDF",
      desc: "在扫描件图像下注入不可见的透明文字层以支持检索复制",
      category: "edit",
      icon: FileSignature,
      color: "text-violet-500 bg-violet-500/10 border-violet-500/20",
      badge: "Stirling",
      keywords: ["searchable", "dual layer", "ocr pdf", "双层pdf", "可搜索pdf"],
    },
    {
      id: "redact",
      label: "机密遮盖涂黑",
      desc: "永久擦除身份证、银行卡等机密区域，彻底不可逆",
      category: "edit",
      icon: Contrast,
      color: "text-zinc-600 bg-zinc-500/10 border-zinc-500/20",
      badge: "Security",
      keywords: ["redact", "blackout", "blur", "censor", "涂黑", "脱敏遮盖", "遮挡"],
    },
    {
      id: "draw",
      label: "自由手绘划线",
      desc: "手写笔或鼠标自由涂鸦、划圈与波浪线标注",
      category: "edit",
      icon: Brush,
      color: "text-emerald-500 bg-emerald-500/10 border-emerald-500/20",
      keywords: ["draw", "pen", "freehand", "scribble", "涂鸦", "手绘", "划线"],
    },

    // ════════════ 3. 格式转换与多格式 (12 tools) ════════════
    {
      id: "pdf2img",
      labelKey: "tools.pdf2img",
      descKey: "tools.pdf2imgDesc",
      label: "PDF 转图片",
      desc: "逐页导出为 PNG、JPEG 或 WebP 图像文件",
      category: "convert",
      icon: Image,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["image", "png", "jpg", "render", "转图片", "导出图片"],
    },
    {
      id: "img2pdf",
      labelKey: "tools.convertImage",
      descKey: "tools.img2pdfDesc",
      label: "图片转 PDF",
      desc: "将多张 JPG/PNG 图像排版合并生成高质量 PDF",
      category: "convert",
      icon: Images,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["image to pdf", "jpg", "png", "convert", "图片转pdf", "合成"],
    },
    {
      id: "pdf2text",
      labelKey: "tools.convertText",
      descKey: "tools.pdf2textDesc",
      label: "提取纯文本",
      desc: "解析全文 UTF-8 字符流并导出为 .txt 纯文本",
      category: "convert",
      icon: FileText,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["txt", "text", "export", "转文本", "导出文字"],
    },
    {
      id: "table",
      labelKey: "tools.extractTable",
      descKey: "tools.extractTableDesc",
      label: "表格结构化提取",
      desc: "智能检测表格网格并无损导出为 CSV 或 Markdown",
      category: "convert",
      icon: Table,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      badge: "Smart",
      keywords: ["table", "csv", "markdown", "excel", "表格", "结构化", "导出表格"],
    },
    {
      id: "epub2pdf",
      label: "EPUB 电子书转 PDF",
      desc: "支持主流 EPUB 格式文档排版解析，一键转为标准可打印 PDF",
      category: "convert",
      icon: Book,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      badge: "New",
      keywords: ["epub", "ebook", "convert epub", "电子书", "epub转pdf"],
    },
    {
      id: "text2pdf",
      label: "TXT 文本转 PDF",
      desc: "导入纯文本排版为分页优雅的标准 PDF 文档",
      category: "convert",
      icon: FileText,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["text to pdf", "txt to pdf", "文本转pdf"],
    },
    {
      id: "md2pdf",
      label: "Markdown 转 PDF",
      desc: "自动解析渲染标题、列表与粗体，排版输出为 PDF",
      category: "convert",
      icon: Code,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["markdown", "md to pdf", "markdown转pdf"],
    },
    {
      id: "cbz2pdf",
      label: "漫画包 (CBZ/ZIP) 转 PDF",
      desc: "解包图像漫画压缩包并按画册格式合成连续 PDF",
      category: "convert",
      icon: BookOpen,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      badge: "Comic",
      keywords: ["cbz", "cbr", "comic", "manga", "漫画", "cbz转pdf"],
    },
    {
      id: "html2pdf",
      label: "网页/HTML 转 PDF",
      desc: "将 HTML 网页内容转为矢量排版 PDF",
      category: "convert",
      icon: FileCode,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["html to pdf", "webpage", "网页转pdf"],
    },
    {
      id: "pdf2json",
      label: "PDF 结构解析为 JSON",
      desc: "导出页面流、字体元数据与对象目录结构供开发者分析",
      category: "convert",
      icon: FileCode,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["json", "export json", "pdf2json", "结构解析"],
    },
    {
      id: "extractImages",
      label: "一键提取全部内嵌图片",
      desc: "无损抽取出 PDF 内包含的所有原始分辨率 JPEG/PNG 图片",
      category: "convert",
      icon: Images,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["extract images", "dump photos", "提取图片", "导出全部图片"],
    },
    {
      id: "svg2pdf",
      label: "SVG 矢量转 PDF",
      desc: "将矢量 SVG 图形无损转换为矢量 PDF",
      category: "convert",
      icon: Palette,
      color: "text-purple-500 bg-purple-500/10 border-purple-500/20",
      keywords: ["svg", "vector to pdf", "svg转pdf"],
    },

    // ════════════ 4. 安全、隐私与元数据 (10 tools) ════════════
    {
      id: "sanitize",
      labelKey: "tools.sanitize",
      descKey: "tools.sanitizeDesc",
      label: "隐私脱敏",
      desc: "彻底清除私有元数据、修改时间戳与编辑软件信息",
      category: "extract",
      icon: ShieldAlert,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      badge: "Stirling",
      keywords: ["privacy", "sanitize", "metadata", "clean", "scrub", "脱敏", "隐私", "清除元数据"],
    },
    {
      id: "metadata",
      label: "文档属性与元数据",
      desc: "查看并编辑标题、作者、主题、关键词及创建信息",
      category: "extract",
      icon: Tag,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["metadata", "properties", "title", "author", "元数据", "文档属性"],
    },
    {
      id: "encrypt",
      label: "密码加密保护",
      desc: "为 PDF 添加强加密开启密码或权限访问限制",
      category: "extract",
      icon: Lock,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      badge: "AES",
      keywords: ["encrypt", "password", "protect", "加密", "设置密码", "权限"],
    },
    {
      id: "decrypt",
      label: "密码解密保存",
      desc: "输入正确密码后永久移除密码锁并另存为开放文档",
      category: "extract",
      icon: Unlock,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["decrypt", "unlock", "remove password", "解密", "去除密码"],
    },
    {
      id: "flatten",
      label: "展平表单与注释",
      desc: "将交互式表单、电子签名与注释合并进底图变为静态不可编辑",
      category: "extract",
      icon: Layers,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["flatten", "static", "展平", "拼合图层", "表单展平"],
    },
    {
      id: "privacyAudit",
      label: "本地隐私审计面板",
      desc: "全程监控并验证 100% 离线、0 外部网络流量，确保数据绝对安全",
      category: "extract",
      icon: ShieldCheck,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      badge: "Zero-Net",
      keywords: ["privacy audit", "zero telemetry", "offline", "隐私审计", "网络流量", "安全监控"],
    },
    {
      id: "certSign",
      label: "数字证书验签",
      desc: "检验 PDF 是否包含数字公钥签名以及文档是否被篡改",
      category: "extract",
      icon: FileSignature,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["certificate", "digital signature", "verify", "数字签名", "验签"],
    },
    {
      id: "changePermissions",
      label: "修改打印与复制权限",
      desc: "禁止未经授权的内容复制、打印或注释编辑",
      category: "extract",
      icon: Lock,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["permissions", "copy protect", "print protect", "权限管理", "禁止复制"],
    },
    {
      id: "comparePdf",
      label: "双文档视觉对比",
      desc: "对比两份合同或文档版本的差异并高亮标出变更",
      category: "extract",
      icon: Files,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["compare", "diff", "contrast", "对比", "版本对比", "差异比对"],
    },
    {
      id: "linearize",
      label: "快速 Web 视图线性化",
      desc: "重组文件目录结构，支持网络环境下的首屏极速加载",
      category: "extract",
      icon: Sparkles,
      color: "text-teal-500 bg-teal-500/10 border-teal-500/20",
      keywords: ["linearize", "fast web view", "线性化", "快速打开"],
    },

    // ════════════ 5. 优化与批处理流水线 (7 tools) ════════════
    {
      id: "compress",
      labelKey: "tools.compress",
      descKey: "tools.compressDesc",
      label: "可控压缩",
      desc: "4 档智能优化减小体积，兼顾矢量画质与体积节省",
      category: "batch",
      icon: Minimize2,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      keywords: ["compress", "size", "shrink", "optimize", "压缩", "瘦身", "减小体积"],
    },
    {
      id: "compressionStudio",
      label: "可控压缩工作室",
      desc: "4 档智能预设前后体积深度测算与图形化节省率比对",
      category: "batch",
      icon: Minimize2,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      badge: "Studio",
      keywords: ["compression studio", "preview size", "压缩工作室", "节省率"],
    },
    {
      id: "batchWorkspace",
      label: "打开批处理工作台",
      desc: "企业级四区流水线，支持多任务排队、实时取消与报告导出",
      category: "batch",
      icon: Sparkles,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      badge: "Pipeline",
      keywords: ["batch", "pipeline", "queue", "批处理", "流水线", "批量转换"],
    },
    {
      id: "repair",
      label: "修复受损 PDF",
      desc: "重建破损的 XRef 交叉索引表，拯救无法打开的异常文档",
      category: "batch",
      icon: Wrench,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      badge: "Bento",
      keywords: ["repair", "fix", "corrupted", "修复", "拯救文件", "重建索引"],
    },
    {
      id: "deskew",
      label: "扫描件倾斜校正",
      desc: "自动检测扫描角度偏差并将其物理水平摆正",
      category: "batch",
      icon: RotateCw,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      keywords: ["deskew", "straighten", "scan", "校正", "纠偏", "摆正"],
    },
    {
      id: "grayscale",
      label: "转黑白灰度文档",
      desc: "去除彩色油墨图层，专为黑白打印与发票合同极简归档优化",
      category: "batch",
      icon: Contrast,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      keywords: ["grayscale", "black and white", "monochrome", "黑白", "灰度", "转黑白"],
    },
    {
      id: "batchExport",
      label: "导出批处理报表",
      desc: "一键将批量操作结果与各文件压缩耗时导出为 CSV / Markdown",
      category: "batch",
      icon: FileSpreadsheet,
      color: "text-amber-500 bg-amber-500/10 border-amber-500/20",
      keywords: ["report", "csv", "summary", "报表", "导出总结", "审计报表"],
    },
  ];

  const filteredTools = $derived(() => {
    const q = searchQuery.trim().toLowerCase();
    return allTools.filter((tool) => {
      if (selectedCategory !== "all" && tool.category !== selectedCategory) {
        return false;
      }
      if (!q) return true;
      const title = (tool.labelKey ? t(tool.labelKey) : (tool.label || tool.id)).toLowerCase();
      const desc = (tool.descKey ? t(tool.descKey) : (tool.desc || "")).toLowerCase();
      const matchKeyword = tool.keywords.some((k) => k.toLowerCase().includes(q));
      return title.includes(q) || desc.includes(q) || matchKeyword;
    });
  });

  $effect(() => {
    if (visible) {
      searchQuery = "";
      setTimeout(() => inputEl?.focus(), 50);
    }
  });

  function handleSelect(toolId: ToolId) {
    visible = false;
    onselecttool(toolId);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!visible) return;
    if (e.key === "Escape") {
      visible = false;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

{#if visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="fixed inset-0 z-50 flex items-start justify-center pt-14 bg-black/50 backdrop-blur-sm p-4 animate-in fade-in duration-150 select-none"
    onclick={() => (visible = false)}
  >
    <div
      class="bg-card border border-border rounded-2xl shadow-2xl w-full max-w-4xl overflow-hidden flex flex-col max-h-[84vh]"
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Header with Search Input -->
      <div class="p-4 border-b border-border space-y-3 bg-muted/20">
        <div class="flex items-center justify-between">
          <div class="flex items-center gap-2 text-sm font-semibold text-foreground">
            <Sparkles class="w-4 h-4 text-primary" />
            <span>{t("toolFinder.title")}</span>
            <span class="text-xs font-normal px-2 py-0.5 rounded-full bg-primary/15 text-primary ml-1">
              51 项完全本地化工具
            </span>
          </div>
          <button
            onclick={() => (visible = false)}
            class="text-muted-foreground hover:text-foreground p-1 rounded-md hover:bg-muted"
          >
            <X size={16} />
          </button>
        </div>

        <!-- Search Bar -->
        <div class="relative">
          <Search class="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-muted-foreground" />
          <input
            bind:this={inputEl}
            type="text"
            bind:value={searchQuery}
            placeholder={t("toolFinder.searchPlaceholder")}
            class="w-full pl-9 pr-4 py-2 text-sm rounded-xl border border-input bg-background/80 focus:bg-background focus:outline-none focus:ring-2 focus:ring-primary/40 transition-all placeholder:text-muted-foreground/60"
          />
        </div>

        <!-- Category Filters -->
        <div class="flex items-center gap-1.5 overflow-x-auto pb-0.5 text-xs">
          <button
            onclick={() => (selectedCategory = "all")}
            class="px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'all'
              ? 'bg-primary text-primary-foreground shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            {t("toolFinder.allTools")} (51)
          </button>
          <button
            onclick={() => (selectedCategory = "page")}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'page'
              ? 'bg-blue-600 text-white shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            <Layers size={13} />
            {t("categories.page")}
          </button>
          <button
            onclick={() => (selectedCategory = "edit")}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'edit'
              ? 'bg-emerald-600 text-white shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            <Pencil size={13} />
            {t("categories.edit")}
          </button>
          <button
            onclick={() => (selectedCategory = "convert")}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'convert'
              ? 'bg-purple-600 text-white shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            <ArrowLeftRight size={13} />
            {t("categories.convert")} & 多格式
          </button>
          <button
            onclick={() => (selectedCategory = "extract")}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'extract'
              ? 'bg-teal-600 text-white shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            <Shield size={13} />
            安全与脱敏
          </button>
          <button
            onclick={() => (selectedCategory = "batch")}
            class="flex items-center gap-1.5 px-2.5 py-1 rounded-lg font-medium transition-colors whitespace-nowrap {selectedCategory === 'batch'
              ? 'bg-amber-600 text-white shadow-sm'
              : 'text-muted-foreground hover:bg-muted hover:text-foreground'}"
          >
            <Sparkles size={13} />
            批处理流水线
          </button>
        </div>
      </div>

      <!-- Tools Grid -->
      <div class="flex-1 overflow-y-auto p-4">
        {#if filteredTools().length === 0}
          <div class="text-center py-12 text-sm text-muted-foreground space-y-1">
            <p class="font-medium text-foreground">{t("toolFinder.noResults")}</p>
            <p class="text-xs text-muted-foreground">Try another search keyword or select All Tools.</p>
          </div>
        {:else}
          <div class="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 gap-2.5">
            {#each filteredTools() as tool (tool.id)}
              {@const Icon = tool.icon}
              <button
                onclick={() => handleSelect(tool.id)}
                class="group text-left p-3 rounded-xl border border-border/70 hover:border-primary/50 hover:bg-accent/40 bg-card transition-all flex flex-col justify-between hover:shadow-md cursor-pointer"
              >
                <div>
                  <div class="flex items-center justify-between mb-2">
                    <div class="w-8 h-8 rounded-lg flex items-center justify-center border {tool.color} transition-transform group-hover:scale-105">
                      <Icon size={16} />
                    </div>
                    {#if tool.badge}
                      <span class="text-[10px] uppercase font-semibold px-1.5 py-0.5 rounded bg-primary/10 text-primary border border-primary/20">
                        {tool.badge}
                      </span>
                    {/if}
                  </div>
                  <h4 class="text-xs font-semibold text-foreground group-hover:text-primary transition-colors">
                    {tool.labelKey ? t(tool.labelKey) : (tool.label || tool.id)}
                  </h4>
                  <p class="text-[11px] text-muted-foreground line-clamp-2 mt-1 leading-snug">
                    {tool.descKey ? t(tool.descKey) : (tool.desc || "")}
                  </p>
                </div>
              </button>
            {/each}
          </div>
        {/if}
      </div>

      <!-- Footer Tip -->
      <div class="px-4 py-2.5 border-t border-border bg-muted/30 flex items-center justify-between text-[11px] text-muted-foreground">
        <span>对标 Stirling PDF & BentoPDF · 100% 离线客户端处理 · 0 网络流量安全可控</span>
        <kbd class="px-1.5 py-0.5 rounded border border-border bg-background font-mono text-[10px]">ESC</kbd>
      </div>
    </div>
  </div>
{/if}
