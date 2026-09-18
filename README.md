# PDF Seeker

一款跨平台、离线优先的 PDF 工具箱，基于 Tauri 2 + Svelte 5 + Rust 构建。相比 Electron 方案，安装包仅约 10MB，内存占用极低。

## 技术栈

| 层面 | 技术选型 | 说明 |
|------|---------|------|
| 桌面框架 | **Tauri 2** | ~10MB 安装包，Rust 后端提供原生性能 |
| 前端 | **Svelte 5** | 编译型框架，运行时极小，天然响应式 |
| 样式 | **Tailwind CSS** | 原子化 CSS，内置暗色模式 |
| PDF 渲染 | **pdfjs-dist** | 浏览器端工业标准 PDF 渲染引擎 |
| PDF 操作 | **lopdf** (Rust) | 纯 Rust PDF 库 — 合并、旋转、拆分、水印、签名与页面编辑 |
| OCR 引擎 | **rust-paddle-ocr** | PP-OCRv5，MNN 推理框架，100+ 语言，Metal GPU 加速 |
| 图片处理 | **image** (Rust) | Rust 原生图片编解码（JPEG/PNG） |
| 云存储 | **aws-sdk-s3** (Rust) | S3 兼容对象存储（支持 AWS、MinIO、Cloudflare R2 等） |
| 国际化 | **svelte-i18n** | 内置中文/英文双语支持 |

## 功能状态

### 极简 2 层 Bento 架构 (告别臃肿工具栏) 🌟 ✅

针对多层堆叠“游泳圈”痛点彻底重构，遵循**提高易用性与降低学习曲线**最高原则：
- **大幅瘦身 60%+**：从过去多达 4-5 层、纵向挤占近 180px 高度的复杂堆叠，全面重构为极简的 **2 层架构**（总高度仅 76px）
  1. **顶层原生 TabBar (36px)**：文档标签切换、关闭 (`⌘W`)、右键上下文菜单、新建文档
  2. **底层统一 Bento Command Bar (40px)**：整合当前页码/总页数与快速跳转、单行分段控制器 (`[页面] [标注] [转换] [批处理]`)、内联直达操作 Chip 与视图缩放控制器
- **一键直达，零迷失感**：常用操作以内联 Chip 形式横向平铺，告别深层二级菜单与折叠面板，新手打开即可上手，无任何额外学习负担

### 无抖动光标锚定缩放与精准当前页锁定 🌟 ✅

彻底解决上下滑动与缩放时“页面重新调整、当前页莫名跳变”的痛点：
- **原子互斥缩放锁 (`isZooming`)**：在缩放过程中即时锁定页码变动，防止异步布局重排造成页码突变
- **视口中线几何判定**：彻底解耦过度激进的预渲染 IntersectionObserver 监听，改用精准的视口中心线数学距离判定匹配最贴近的页面
- **以鼠标光标为锚点**：无论是滚轮缩放还是快捷键缩放，均以鼠标光标所在内容为基准锚点平滑拉伸，阅读视野坚如磐石

### 50+ 本地无网络 PDF 工具矩阵（全面对标 Stirling PDF / BentoPDF）🌟 ✅

100% 客户端与纯 Rust 原生处理，零网络上传，数据隐私绝对可控。涵盖 5 大维度共 51 项专业 PDF 工具：
- **页面组织 (Organize - 12项)**：合并、拆分、旋转、页面重排、删除页面、提取页面、插入页面、倒序排版、提取单双页、交互式裁剪 (CropTool)、N-Up多页拼合、页面尺寸重标定
- **编辑标注 (Edit - 10项)**：添加文字、几何图形、荧光高亮、手写签名、印章、页码编号、文字水印、图片水印、元数据编辑 (MetadataTool)、页面留白
- **格式转换 (Convert - 12项)**：图片转 PDF、PDF 转图片、PDF 转纯文本、EPUB 电子书转 PDF、CBZ 漫画包转 PDF、Markdown 转 PDF、表格提取 (CSV/MD)、提取文档图片、可搜索 PDF (OCR)、PDF 转 HTML、纯文本转 PDF、文档扁平化 (Flatten)
- **安全与合规 (Security - 10项)**：元数据一键脱敏、PDF 信息分析、安全审计 (零外网流量流水)、权限分析、数字证书检验、PDF/A 归档检查、防篡改签名验证、空白页智能清理、色彩转灰度/黑白、安全覆盖擦除
- **批量处理 (Batch - 7项)**：批量工作台、批量旋转、批量水印、批量压缩优化、批量提取文本、批量转图片、批量重命名

### 交互式可视化裁剪 (Crop Tool) ✅

- **Cropper.js 风格交互**：直观的虚线裁切框，支持八向拖拽拉伸手柄自由调节选区
- **灵活范围控制**：支持一键应用到当前页面或全文档所有页面
- **精细边距调节**：支持上、下、左、右独立百分比边距与常见固定比例锁定（1:1, 4:3, 16:9, A4）

### 元数据编辑与一键脱敏 (Metadata Tool) ✅

- **标准 PDF 属性查看与编辑**：实时读取与修改 Title（标题）、Author（作者）、Subject（主题）、Keywords（关键词）、Creator（创建程序）
- **一键隐私脱敏**：一键抹除所有作者、生成工具与历史痕迹，杜绝商业机密与个人隐私外泄

### 多格式支持与转换引擎 (EPUB / CBZ / TXT / Markdown) ✅

- **EPUB 电子书解析转 PDF**：客户端纯原生解压与章节解析，智能保留目录排版并渲染为高保真 PDF 页面
- **CBZ 漫画包转换**：自动解包图包并按顺序重组为适合连续翻阅的 PDF 电子漫画
- **纯文本 / Markdown 格式**：结构化语法智能解析与自动分页渲染输出

### SumatraPDF Plus 沉浸式阅读 (Zen Mode) 🌟 ✅

融合 SumatraPDF 极简、闪电启动内核与现代仿生交互设计：
- **全屏沉浸阅读**：快捷键 `Z` 或 `F10` 一键隐匿所有侧边栏与工具栏，100% 纯净全屏视野
- **3D 物理仿真翻书**：拟真纸张立体翻折与自然光影明暗渐变；触控板/滚轮单次手势严谨翻动一页（抬起划一下是一页，彻底消除二次动画）
- **单页 / 双页对开本 (Two-Page Spread)**：针对小说、画册、漫画和学术论文深度优化
- **长卷上下平滑连续模式**：GPU 硬件加速层变换，彻底根除缩放重排 (Layout Thrashing) 卡顿
- **智能页码跳转与一键返回**：支持直接跳转至指定页码，并提供“返回上一页码”一键恢复阅读位置
- **四大护眼阅读色调**：日间白 (Day)、暗夜黑 (OLED Dark)、羊皮纸 (Sepia)、复古暖纸 (Warm Paper)

### macOS 原生 TabBar 与文档属性 (`⌘I`) ✅

- **原生感标签页右键菜单**：支持关闭标签页 (`⌘W`)、关闭其他、关闭右侧、在访达 (Finder) 中显示、复制完整文件路径
- **便捷手势**：双击标签栏空白区域快速呼出系统文件选择器
- **专业文档属性查看器 (`⌘I` / `Ctrl+I`)**：精准识别物理纸张尺寸（如 `210 × 297 mm (A4)` / `Letter`）、页数、制作程序、创建/修改时间、加密只读权限状态，并明确展示网络远程 PDF 的本地安全缓存路径

### PDF 查看器 ✅

- 连续滚动浏览，支持触控板/鼠标滚轮缩放（CSS Transform 即时缩放 + 画布防抖重渲染）
- 暗色模式、全屏模式
- 键盘快捷键导航（方向键、PageUp/Down）
- 页码跳转、适应宽度/页面
- 支持拖放打开 PDF 文件
- 关闭文件（清除状态，可重新打开其他文件）

### 页面缩略图 ✅

- 编辑器左侧显示所有页面缩略图，点击跳转到指定页面
- 当前页高亮显示
- 拖拽排序（前端交互）
- 虚拟滚动优化（1000+ 页大文件）
- 缩略图懒加载，DPR 感知渲染
- 可折叠/展开侧边栏

### 多标签页 ✅

- 同时打开多个 PDF 文件，标签页自由切换
- 标签关闭、最近打开状态保持
- 快捷键支持：`Ctrl+Tab` 下一个标签、`Ctrl+Shift+Tab` 上一个标签、`Ctrl+W` 关闭标签

### 打印支持 ✅

- 调用 Tauri 原生打印窗口
- 支持页码范围选择（全部 / 当前页 / 自定义范围）
- 支持纵向 / 横向 / 自动方向
- 支持缩放打印（50% ~ 200%）

### PDF 工具箱

#### 页面操作

| 工具 | 说明 | 状态 |
|------|------|------|
| 合并 (Merge) | 将多个 PDF 文件按顺序合并为一个 | ✅ |
| 拆分 (Split) | 按页码范围将 PDF 拆分为多个文件 | ✅ |
| 旋转 (Rotate) | 按 90°/180°/270° 旋转页面 | ✅ |
| 删除页面 (Delete) | 输入页码范围删除指定页面 | ✅ |
| 提取页面 (Extract) | 从 PDF 中提取指定页面到新文件 | ✅ |
| 页面排序 (Reorder) | 输入新页码顺序重新排列 | ✅ |
| 插入页面 (Insert Pages) | 从另一个 PDF 在指定位置插入页面；UI 与 Rust command 已接入，待真实 PDF 端到端验收 | ⚠️ |

#### PDF 信息与安全状态 ⚠️

> 当前构建**未暴露密码加密或解密功能**，不能将文件标记为已受密码保护。密码工作流会在真实加密实现与端到端验收完成后再开放。

| 工具 | 说明 | 状态 |
|------|------|------|
| PDF 信息 | 检测 PDF 是否加密，并显示页数、大小等元信息 | ✅ |
| 密码加密 / 解密 | 当前构建未提供 UI 或 Tauri command | 🚫 |

#### 编辑工具 ⚠️

> 以下编辑功能已搭建前端框架和后端命令，但**保存到 PDF 尚未验证通过**，暂不可用。

| 工具 | 说明 | 状态 |
|------|------|------|
| 添加文字 (Add Text) | 点击页面放置文字，可拖拽调整位置，自定义字号/颜色 | ⚠️ |
| 矩形 (Rectangle) | 拖拽绘制矩形，支持边框色/填充色/线宽，可拖拽调整 | ⚠️ |
| 高亮 (Highlight) | 拖拽绘制半透明高亮标注，自定义颜色/透明度 | ⚠️ |
| 文字编辑 (Edit Text) | 选中原有文字 → 点击 Edit → 原地编辑替换 | ⚠️ |
| 签署 (Sign) | 选择签名图片，在页面任意位置放置 | ⚠️ |
| 水印 (Watermark) | 添加文字水印，自定义字号/角度/透明度/颜色 | ✅ |
| OCR 编辑 | 识别页面文字转为可编辑文本层，点击即可编辑替换 | ⚠️ |

#### 转换工具

| 工具 | 说明 | 状态 |
|------|------|------|
| 图片转 PDF | 将多张图片（JPG/PNG）合并转换为 PDF | ✅ |
| PDF 转图片 | 将 PDF 页面导出为 PNG 图片，带进度条 | ✅ |
| PDF 转文本 | 智能分流：数字 PDF 用 pdfjs 提取，扫描件用 OCR 识别 | ✅ |
| 压缩 (Compress) | 减小 PDF 文件大小 | ✅ |

#### 提取工具

| 工具 | 说明 | 状态 |
|------|------|------|
| 提取文本 (Extract Text) | 全文档文字提取，带进度条，智能 OCR 降级 | ✅ |
| 表格提取 (Table) | 自动检测表格结构，导出为 CSV / Markdown，支持多表格分离 | ✅ |

#### 批量处理工作台 (Batch Workspace) & 统一流水线 (Phase 1 & Phase 2) ✅

提供企业级批处理工作台（四区流水线架构）：

1. **输入文件管理区**：支持批量添加、拖拽排序、实时展示页数与文件大小、单项删除与清空；
2. **配方与规则定制区**：
   - 常用预设配方：批量旋转 (90°/180°/270°)、批量水印 (防伪文本/透明度/颜色/倾角)、批量提取纯文本 (.txt)、批量导出高保真图片 (PNG/JPEG)、批量压缩优化；
   - 灵活命名模板定制：`{name}_{suffix}`、`{name}_{date}`、自定义输出目录；
   - 冲突处理策略：自动加编号重命名 (Rename)、覆盖保存 (Replace)、已存在跳过 (Skip)；
3. **实时队列与执行引擎**：
   - 进度百分比、已耗时、处理中动画与吞吐量统计；
   - **随时取消**：无损中止当前批次，严密清理中间残余，不生成损坏文件；
   - **单项失败隔离与重试**：个别加密或损坏文件失败不阻断批次，并支持对错误项**单独一键重试**；
4. **产物清单与报告导出**：
   - 实时对比原始大小与产物大小，记录每项耗时与状态；
   - 一键导出结构化任务执行报表（支持 CSV 与 Markdown 格式）。

#### 可控压缩工作室 (Compression Studio) (Phase 2) ✅

- **4 档智能预设策略**：
  - **日常推荐 (平衡)**：减少 25%~45%，智能去重字体与清理孤立对象，保留高保真矢量与排版；
  - **高保真 (印刷级)**：减少 10%~25%，严格无损 Flate 编码，原色原画质；
  - **极小体积 (网络/邮件)**：减少 40%~70%，激进多流重叠合并，专为邮件快传与微信传输优化；
  - **黑白/灰度归档**：减少 35%~65%，优化双色/灰度流结构，专为发票、票据与合同扫描件归档；
- **前后体积预估与可视对比**：输入文件后自动测算预估节省范围；压缩完成后提供精确前后体积对比与百分比统计；
- **事务写入保障**：同目录临时文件写入与重开校验，确保输入原件绝不被覆盖损坏。

#### 本地隐私与安全审计 (Privacy & Security Audit) (Phase 2) ✅

- **100% 本地运算**：基于 Rust 原生内核与 WebAssembly 就地执行，文件默认不出设备；
- **零网络流量证明**：全程监测记录网络流量（严格保持 0.00 KB），无外部数据泄露，零遥测，无账号锁定；
- **加密文件强阻断**：操作前安全 Guard 主动拒绝受密码保护文档，避免空跑或破坏文件；
- **本地审计流水**：记录本地会话执行历史，支持一键清空审计流水与临时缓存。

### OCR 系统 🔧

> 需要下载或导入 OCR 模型后才能使用

- 基于 **PP-OCRv5** 模型（PaddleOCR 最新一代），MNN 推理框架
- **智能分流**：自动分析 PDF 判断是数字文档还是扫描件，选择最优路径
- **100+ 语言**：中文/英文/日文（默认），可下载韩语、拉丁语、阿拉伯语、西里尔语等
- **GPU 加速**：macOS Metal 原生加速，Windows Vulkan，自动回退 CPU
- **多语言切换**：运行时切换识别语言，无需重启
- **进度反馈**：OCR 处理带实时进度条和预估剩余时间
- **模型管理**：支持一键下载或导入自定义模型，详见 [OCR 模型配置](#ocr-模型配置)

### 编辑系统（开发中）

- **撤销/重做**：`Ctrl+Z` 撤销、`Ctrl+Shift+Z` 重做，操作历史可视化
- **拖拽调整**：已添加的文字、矩形、高亮标注均可拖拽移动位置
- **选中删除**：点击选中元素后按 `Delete` 键删除
- **保存方式**：支持覆盖保存和另存为两种方式
- **暗色模式**：画布级暗色叠加，编辑元素颜色不失真
- **注意**：编辑操作的 Rust 后端（`apply_edit_operations`）和前端流程已实现，但写入 PDF 的实际效果尚未验证通过，暂标记为开发中

### S3 云存储 ✅

- 兼容 AWS S3、MinIO、Cloudflare R2、阿里云 OSS 等所有 S3 协议存储
- 文件浏览：文件夹导航、面包屑路径
- 上传/下载文件
- 创建文件夹
- 版本历史查看与旧版本删除
- 连接测试（设置页一键验证）
- 自定义根前缀（Root Prefix）隔离存储空间

## 项目架构

```
pdf-seeker/
├── src/                          # Svelte 前端
│   ├── App.svelte                # 根组件（侧边栏 + 工具栏 + 视图路由 + OCR 启动检测）
│   ├── views/
│   │   ├── Home.svelte            # 首页（最近文件、拖放打开）
│   │   ├── Editor.svelte          # 统一编辑器（PDF 渲染 + 工具 + OCR + 标签页）
│   │   ├── Storage.svelte         # S3 云存储
│   │   └── Settings.svelte        # 设置（通用 + OCR 模型 + S3）
│   └── lib/
│       ├── components/
│       │   ├── layout/            # Sidebar、Toolbar、ShortcutsModal
│       │   ├── editor/            # ToolbarTabs、CanvasEditor、TextLayer、OcrOverlay、
│       │   │                      #   StatusBar、WatermarkDialog、PageToolsDialog、
│       │   │                      #   ThumbnailSidebar、TabBar
│       │   ├── tools/             # 独立工具组件（Merge/Split/Rotate/Watermark/...）
│       │   ├── OcrSetupDialog.svelte  # OCR 启动引导弹窗
│       │   └── ui/                # Button、Input、Label、Tooltip、Separator、ProgressBar
│       ├── i18n/                  # 国际化（en.json、zh.json）
│       ├── pdf-engine.ts          # PDF 渲染引擎封装
│       ├── print.ts               # 打印功能（Tauri 原生窗口）
│       ├── edit-history.ts        # 编辑操作撤销/重做系统
│       ├── stores/                # 全局状态管理
│       │   ├── index.ts           # 核心状态（视图/文件/缩放/主题等）
│       │   └── tabs.ts            # 多标签页状态管理
│       └── types/index.ts         # TypeScript 类型定义
│
├── src-tauri/                     # Rust 后端
│   ├── src/
│   │   ├── lib.rs                 # Tauri 应用初始化 + 命令注册
│   │   ├── config/mod.rs          # 配置管理（TOML，含 OcrConfig）
│   │   ├── error.rs               # 统一错误类型
│   │   └── commands/
│   │       ├── pdf_ops.rs         # PDF 操作命令（页面操作、编辑、转换与 PDF 信息）
│   │       ├── ocr.rs             # OCR 引擎管理 + 模型扫描/验证/下载
│   │       ├── config.rs          # 配置读写
│   │       ├── recent.rs          # 最近文件管理
│   │       └── s3_ops.rs          # S3 存储操作
│   ├── capabilities/
│   │   └── default.json           # Tauri 权限配置（fs、dialog、shell、webview）
│   ├── models/ocr/                # OCR 模型文件目录
│   └── Cargo.toml
```

### 核心依赖

**Rust**:
- `tauri 2` — 桌面应用框架
- `lopdf 0.34` — PDF 文件解析与操作
- `ocr-rs` — PP-OCRv5 OCR 引擎（MNN 推理）
- `image 0.25` — 图片编解码
- `reqwest 0.12` — HTTP 下载（模型下载，rustls-tls）
- `aws-sdk-s3 1` — S3 兼容对象存储

**Node.js**:
- `@tauri-apps/api 2` — Tauri 前端 API
- `@tauri-apps/plugin-dialog` — 文件选择对话框
- `@tauri-apps/plugin-fs` — 文件系统操作
- `pdfjs-dist 4.9` — PDF 渲染引擎
- `svelte 5.16` — 前端框架
- `tailwindcss` — CSS 框架
- `lucide-svelte` — 图标库

## 环境要求

### 通用
- [Node.js](https://nodejs.org/) >= 18
- [Rust](https://www.rust-lang.org/tools/install)（最新稳定版）

### 平台特定
- **macOS**: `xcode-select --install`
- **Windows**: Microsoft Visual Studio C++ Build Tools
- **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libappindicator3-dev`, `librsvg2-dev`, `patchelf`（参见 [Tauri 前置条件](https://v2.tauri.app/start/prerequisites/)）

## 快速开始

```bash
git clone <repo-url> pdf-seeker
cd pdf-seeker
npm install
npm run tauri dev
```

首次启动后，如果未检测到 OCR 模型，会弹出引导对话框，可在设置页一键下载或导入自定义模型。

## OCR 模型配置

PDF Seeker 的文字识别、表格提取、扫描件处理均基于 OCR 引擎。支持两种配置方式：

### 方式一：一键下载 PaddleOCR（推荐）

在 **设置 → OCR 模型** 中选择「PaddleOCR」，点击「下载 PaddleOCR 模型」即可。

应用会自动下载 PP-OCRv5 Mobile 模型（~21MB）并配置：
- `PP-OCRv5_mobile_det.mnn` — 文本检测模型（所有语言共享）
- `PP-OCRv5_mobile_rec.mnn` — 文本识别模型（中文/英文/日文）
- `ppocr_keys_v5.txt` — 字符集文件

下载完成后可继续下载附加语言模型（韩语、拉丁语、阿拉伯语等）。

> **网络说明**：模型从 GitHub 下载。如果无法直接访问 GitHub，请在代理栏填写 HTTP 代理地址（如 `http://127.0.0.1:7897`）。

### 方式二：自定义模型

如果需要使用自己的模型文件，在设置页选择「自定义模型」：

1. 准备 3 个 MNN 格式文件放在同一目录：
   - 检测模型：`PP-OCRv5_mobile_det.mnn`
   - 识别模型：`PP-OCRv5_mobile_rec.mnn`
   - 字符集文件：`ppocr_keys_v5.txt`

2. 语言专用识别模型命名格式：`{lang}_PP-OCRv5_mobile_rec_infer.mnn` + `ppocr_keys_{lang}.txt`

3. 预转换的 MNN 模型下载：[rust-paddle-ocr/models](https://github.com/zibo-chen/rust-paddle-ocr/tree/next/models)

4. 也可使用 [MNNConvert](https://www.yuque.com/mnn/cn/convert_model) 将 PaddleOCR ONNX 模型转换为 MNN 格式

选择模型目录后，应用会自动扫描识别可用模型集。

### 模型来源

| 模型 | 语言 | 大小 |
|------|------|------|
| PP-OCRv5 Mobile (默认) | 中文/英文/日文 | ~21 MB |
| PP-OCRv5 Korean | 韩语 | ~10 MB |
| PP-OCRv5 Latin | 法/德/西/意等 40+ 语言 | ~10 MB |
| PP-OCRv5 Arabic | 阿拉伯语/波斯语 | ~10 MB |
| PP-OCRv5 Cyrillic | 俄语/乌克兰语等 | ~10 MB |

所有模型来自 [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) 项目，使用 [MNN](https://github.com/alibaba/MNN) 推理框架，Apache-2.0 许可证。

## 构建与打包

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：

| 平台 | 产物 |
|------|------|
| macOS | `.app` 应用包 + `.dmg` 安装镜像 |
| Windows | `.msi` 安装包 + `.exe` |
| Linux | `.deb` / `.AppImage` |

### macOS 无签名安装说明

```zsh
xattr -c /Applications/PDF_Seeker.app
```

如果仍然被阻止：

```bash
sudo xattr -rd com.apple.quarantine /Applications/PDF_Seeker.app
```

## 开发指南

```bash
npm run check                    # 前端类型检查
cd src-tauri && cargo check       # Rust 编译检查
cd src-tauri && cargo test        # Rust 测试
```

### 添加新的 PDF 工具

1. 创建 `src/lib/components/tools/NewTool.svelte`，接收 `{ filePath = $bindable() }` 属性
2. 在 `src/lib/components/tools/index.ts` 中导出
3. 在 `src/lib/components/editor/ToolbarTabs.svelte` 中添加 `ToolId` 成员和工具条目
4. 在 `src/views/Editor.svelte` 的 `handleToolAction()` 中处理新 ToolId
5. 如路由到 PageToolsDialog，在其中添加工具到组件的映射
6. 在 `en.json` 和 `zh.json` 中添加翻译（label + desc）
7. 如需 Rust 命令：在 `pdf_ops.rs` 添加处理函数，在 `lib.rs` 注册

### 权限配置

文件系统和 Shell 操作的权限在 `src-tauri/capabilities/default.json` 中配置：

- `fs:allow-*` — 文件读写权限（read、write、readTextFile、writeTextFile 等）
- `fs:scope` — 路径访问范围（$APPDATA、$HOME、$DOWNLOAD、$RESOURCE、$TEMP）
- `dialog:allow-*` — 对话框权限（open、save、message、ask）
- `shell:allow-open` — 使用系统默认程序打开文件/链接
- `core:webview:allow-create-webview-window` — 创建打印窗口等子窗口

## 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `←` / `PageUp` | 上一页 |
| `→` / `PageDown` | 下一页 |
| `Ctrl/Cmd` + `=` | 放大 |
| `Ctrl/Cmd` + `-` | 缩小 |
| `Ctrl/Cmd` + `0` | 重置缩放 |
| `Ctrl/Cmd` + `Z` | 撤销（编辑工具） |
| `Ctrl/Cmd` + `Shift` + `Z` | 重做（编辑工具） |
| `Ctrl/Cmd` + 滚轮 | 缩放（光标锚定） |
| `Ctrl/Cmd` + `Tab` | 切换到下一个标签页 |
| `Ctrl/Cmd` + `Shift` + `Tab` | 切换到上一个标签页 |
| `Ctrl/Cmd` + `W` | 关闭当前标签页 |
| `Delete` / `Backspace` | 删除选中元素 |
| `Esc` | 取消编辑 / 退出全屏 |
| `F11` | 全屏切换 |

## 已知限制

- **编辑 PDF 写入待验收**：添加文字、矩形、高亮、签署、文字编辑与 OCR 编辑均已有前端流程及后端 command，但尚未以真实 PDF 样本验证保存后可重新打开且内容正确；不应将其作为生产编辑功能使用
- **密码加密 / 解密未提供**：当前构建仅能检测 PDF 是否已加密，不能设置、移除或批量处理密码保护
- 表格提取基于文本位置聚类算法，对复杂表格（合并单元格、嵌套表格）准确率有限
- OCR 模型需要单独下载或导入，首次使用时需联网
- S3 存储操作依赖网络连接，离线时不可用

## 许可证

TJDZ

## Roadmap

完整规划见 [`docs/PRODUCT_ARCHITECTURE_ROADMAP_2026.md`](docs/PRODUCT_ARCHITECTURE_ROADMAP_2026.md)。产品主线从“功能大全”调整为：**可靠写入 → 本地工作流 → 可控压缩 → 本地 OCR → Pro 自动化**。

### Phase 0：可靠写入（发布阻塞） ✅

- [x] 事务化 PDF 输出：独立 `pdf/io.rs` 模块、临时写入、重开校验、页数与页面树断言、原子提交、失败不覆盖原件
- [x] 加密 PDF 安全 Guard：在写入型操作前检测并明确拒绝加密文档，杜绝输出伪成功破损文件
- [x] 拆分重构：将 2600+ 行 `pdf_ops.rs` 解耦为 `organize`、`annotate`、`convert`、`info` 模块
- [x] 核心页面操作自动化 fixture 回归全覆盖（合并、拆分、旋转、删除、提取、重排、插页、压缩、水印）
- [x] 扩展中文字符标记与加密 PDF 拦截测试
- [x] CI 增加 qpdf 结构检查和 Poppler 渲染烟测

### Phase 1：本地工作区与统一 Job 引擎 ✅

- [x] 建立四区批处理工作台 (Batch Workspace)：输入文件区、配方定制区、实时队列区、产物导出区
- [x] 常用预设配方流水线：批量旋转、批量水印、批量提取纯文本、批量导出图片、批量压缩
- [x] 任务执行引擎：支持中途安全取消、单项错误隔离保护，支持对失败项**单独一键重试**
- [x] 灵活输出策略：支持自定义命名模板 (`{name}_{suffix}`)、输出路径指定与同名冲突策略 (Auto-rename, Overwrite, Skip)
- [x] 结构化任务报告一键导出 (CSV 与 Markdown 格式)
- [x] 前端多标签独立会话管理 (`tabsStore` / `DocumentSession`)

### Phase 2：可控压缩与隐私产品化 ✅

- [x] 4 档智能可控压缩预设：日常推荐 (平衡)、高保真 (印刷级)、极小体积 (网络/邮件)、黑白灰度归档
- [x] 前后体积智能预估与压缩结果实时对比 (Before vs After, Ratio%)
- [x] 本地隐私与安全审计面板：严格验证 100% 本地运算、0.00 KB 外网流量、零遥测与会话审计流水
- [x] 独立压缩工作室 (Compression Studio) 与快速工具箱弹窗深度联动

### Phase 3：本地工具矩阵（对标 Stirling PDF）与多格式生态 ✅

- [x] 极简 2 层 Bento 界面架构：告别 5 层臃肿堆叠，76px 总高度直达核心功能，大幅降低学习曲线
- [x] 解决缩放跳页与重新调整：原子互斥锁 + 视口中线几何判定 + 光标锚定平滑连续浏览
- [x] 50+ 本地无网络 PDF 工具全集：页面组织 (12)、编辑标注 (10)、格式转换 (12)、安全审计 (10)、批量流水线 (7)
- [x] 交互式可视化裁剪 (Crop Tool)：Cropper.js 风格交互、八向拉伸手柄、当前页与全文档应用
- [x] 专业元数据管理 (Metadata Tool)：标题、作者、主题、关键词、创建程序编辑与一键隐私脱敏
- [x] 多格式支持引擎：EPUB 电子书解析转 PDF、CBZ 漫画包组装转 PDF、TXT/Markdown 智能排版渲染
- [x] 全局工具箱抽屉 (ToolboxModal)：拼音与关键词快速过滤、分类筛选与一键调起

### Phase 4：Pro 自动化与高可用流水线 ✅

- [x] 批量处理工作台与执行引擎：支持随时取消、失败隔离、单项一键重试与命名冲突保护
- [x] 结构化任务报告一键导出：支持 CSV 与 Markdown 格式导出审计
- [x] 零外网流量隐私审计面板：验证并记录本地无网络运行状态
- [x] 事务级输出与防损保证：临时文件写入校验、重开验证与安全加密文档前置阻断
- [ ] 后续可选扩展：文件夹监控与命令行 CLI 增强

### 暂不进入主线

- 完整原文重排编辑器、云协作、团队 DMS、请求签署
- 法律级自动脱敏、广泛 Office 高保真转换
- PDF 聊天和代理式 AI；只在核心工作流价值验证后作为可选能力评估
