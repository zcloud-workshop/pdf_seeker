# pdfseek CLI

离线、本地、零上传的「扫描件/PDF → 排版文本」提取工具，附元数据一键脱敏。

- **纯本地运行**：OCR 推理在设备上完成（PP-OCRv5，MNN），不上传任何文件
- **输入**：`png / jpg / jpeg`（直接识别）或 `pdf`（自动光栅化后识别，需系统安装 poppler）
- **输出**：按行与段落重排的 Markdown

## 用法

```bash
# 图片 → Markdown
pdfseek 扫描件.jpg

# PDF → Markdown（指定输出与清晰度）
pdfseek 讲义.pdf -o 讲义.md --dpi 200

# 顺带生成去除元数据（作者/生成器/XMP）的 PDF 副本
pdfseek 合同.pdf --redact

# 指定模型目录（默认自动寻找 ./models）
pdfseek 文件.png --models ./models
```

参数：`-o 输出路径` ｜ `--dpi N`（PDF 光栅化 DPI，默认 200）｜ `--redact` ｜ `--models 目录` ｜ `--no-gpu`

## 安装

从 [Releases](../../releases) 下载对应平台压缩包，解压后：

- **macOS**：`xattr -d com.apple.quarantine pdfseek`（首次运行解除隔离），随后直接使用
- **处理 PDF 需要一次性安装 poppler**：macOS `brew install poppler`；Ubuntu `sudo apt install poppler-utils`（纯图片识别无需安装）

## 已知不支持（如实告知）

手写体、复杂公式与表格结构、严重弯曲变形的扫描件——这些场景输出可能不完整。

## 从源码构建

```bash
cd src-tauri && cargo build --release --bin pdfseek
```
- 含大量图表/架构图的页面（图内散落标签会碎）
