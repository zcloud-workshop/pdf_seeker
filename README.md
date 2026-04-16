# PDF Seeker

一款跨平台、离线优先的 PDF 工具箱，基于 Tauri 2 + Svelte 5 + Rust 构建。相比 Electron 方案，安装包仅约 10MB，内存占用极低。

## 技术栈

| 层面 | 技术选型 | 说明 |
|------|---------|------|
| 桌面框架 | **Tauri 2** | ~10MB 安装包，Rust 后端提供原生性能 |
| 前端 | **Svelte 5** | 编译型框架，运行时极小，天然响应式 |
| 样式 | **Tailwind CSS** | 原子化 CSS，内置暗色模式 |
| PDF 渲染 | **pdfjs-dist** | 浏览器端工业标准 PDF 渲染引擎 |
| PDF 操作 | **lopdf** (Rust) | 纯 Rust PDF 库 — 合并、旋转、拆分、水印、签名等 |
| 图片处理 | **image** (Rust) | Rust 原生图片编解码（JPEG/PNG） |
| 云存储 | **aws-sdk-s3** (Rust) | S3 兼容对象存储（支持 AWS、MinIO、Cloudflare R2 等） |
| 国际化 | **svelte-i18n** | 内置中文/英文双语支持 |

## 功能一览

### PDF 查看器
- 连续滚动浏览，支持触控板/鼠标滚轮缩放
- 暗色模式、全屏模式
- 键盘快捷键导航（方向键、PageUp/Down）
- 页码跳转、适应宽度/页面

### PDF 工具箱
| 工具 | 说明 |
|------|------|
| 合并 (Merge) | 将多个 PDF 文件按顺序合并为一个 |
| 拆分 (Split) | 按页码范围将 PDF 拆分为多个文件 |
| 旋转 (Rotate) | 按 90°/180°/270° 旋转页面，支持缩略图预览 |
| 删除页面 (Delete) | 通过缩略图选择或输入页码删除指定页面 |
| 提取页面 (Extract) | 从 PDF 中提取指定页面到新文件 |
| 压缩 (Compress) | 压缩 PDF 文件体积，显示压缩率 |
| 水印 (Watermark) | 添加文字水印，支持自定义字号、角度、透明度、颜色 |
| 图片转 PDF | 将多张图片（JPG/PNG）合并转换为 PDF |
| PDF 转图片 | 将 PDF 每页导出为 PNG 图片 |
| PDF 转文本 | 提取 PDF 文字内容为纯文本 |
| 签署 (Sign) | 在指定页面位置添加签名图片 |
| OCR 识别 | 提取 PDF 中的文字，内置 pdfjs 提取 + Tesseract 兜底 |
| 添加文字 | 在 PDF 页面指定坐标位置添加文字 |
| 矩形 | 在 PDF 页面绘制矩形（支持边框、填充色） |
| 高亮 | 在 PDF 页面添加半透明高亮标注 |
| 表格提取 | 计划中... |

### 编辑工具特性
- 所有编辑工具支持**大画布实时预览**（接近原始尺寸）
- **鼠标拖拽选区**：在预览画布上拖拽即可设定坐标，自动回填 X/Y/W/H 输入框
- 缩略图条快速切换页面，页码与输入框双向同步
- 拖拽完成后自动重新渲染预览

### S3 云存储
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
│   ├── main.ts                   # 入口文件
│   ├── App.svelte                # 根组件（侧边栏 + 工具栏 + 视图路由）
│   ├── views/                    # 页面组件
│   │   ├── Home.svelte            # 首页（最近文件、快速操作）
│   │   ├── Viewer.svelte          # PDF 查看器（连续滚动、缩放）
│   │   ├── Tools.svelte           # PDF 工具箱（所有工具面板）
│   │   ├── Storage.svelte         # S3 云存储（浏览/上传/下载/版本）
│   │   └── Settings.svelte        # 设置（通用 + S3 配置）
│   └── lib/
│       ├── components/
│       │   ├── layout/            # 布局组件（Sidebar、Toolbar）
│       │   └── ui/                # 通用 UI 组件（Button、Input、Label）
│       ├── i18n/
│       │   ├── index.svelte.ts    # 国际化配置
│       │   └── locales/
│       │       ├── en.json        # 英文翻译
│       │       └── zh.json        # 中文翻译
│       ├── pdf-engine.ts          # PDF 渲染引擎封装（pdfjs-dist）
│       ├── stores/index.ts        # 全局状态管理（视图、文件、缩放等）
│       ├── types/index.ts         # TypeScript 类型定义
│       └── utils/cn.ts             # Tailwind class 工具
│
├── src-tauri/                     # Rust 后端
│   ├── src/
│   │   ├── main.rs                # Rust 入口
│   │   ├── lib.rs                 # Tauri 应用初始化 + 命令注册
│   │   ├── config/mod.rs          # 配置管理（TOML 持久化）
│   │   ├── error.rs               # 统一错误类型
│   │   └── commands/
│   │       ├── mod.rs             # 命令模块导出
│   │       ├── config.rs          # 配置读写命令
│   │       ├── pdf_ops.rs         # PDF 操作命令（20+ 个命令）
│   │       ├── recent.rs          # 最近文件管理
│   │       └── s3_ops.rs          # S3 存储操作命令
│   ├── Cargo.toml                 # Rust 依赖配置
│   ├── tauri.conf.json            # Tauri 应用配置
│   ├── capabilities/default.json   # 安全权限声明
│   └── icons/                     # 各平台应用图标
│
├── package.json                   # Node.js 依赖与脚本
├── vite.config.ts                 # Vite 构建配置
├── svelte.config.js               # Svelte 编译配置
├── tailwind.config.js             # Tailwind CSS 配置
└── tsconfig.json                 # TypeScript 配置
```

### 前后端通信

前端通过 Tauri 的 `invoke` 调用 Rust 后端命令，采用异步 IPC 通信：

```
Svelte (invoke)  ──→  Tauri IPC  ──→  Rust Command Handler
     ↑                                      │
  Result/Event  ←───────────────────────────┘
```

### 核心依赖

**Rust (src-tauri/Cargo.toml)**:
- `tauri 2` — 桌面应用框架
- `lopdf 0.34` — PDF 文件解析与操作
- `image 0.25` — 图片编解码（JPEG/PNG）
- `aws-sdk-s3 1` — S3 兼容对象存储客户端
- `serde / serde_json` — 序列化
- `toml` — 配置文件解析
- `directories` — 跨平台目录定位

**Node.js (package.json)**:
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

### 安装依赖

```bash
git clone <repo-url> pdf-seeker
cd pdf-seeker
npm install
```

### 开发模式

```bash
npm run tauri dev
```

启动后会自动打开应用窗口，前端热更新，Rust 后端修改后自动重编译。

### 仅前端开发（不含 Rust 后端）

```bash
npm run dev
```

在浏览器中访问 `http://localhost:1420`，但 PDF 操作等后端功能不可用。

## 构建与打包

### 构建当前平台

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/`：

| 平台 | 产物 |
|------|------|
| macOS | `.app` 应用包 + `.dmg` 安装镜像 |
| Windows | `.msi` 安装包 + `.exe` |
| Linux | `.deb` / `.AppImage` |

### 构建发布版本

```bash
# Release 模式构建（体积更小、性能更优）
npm run tauri build -- --release
```

### 交叉编译

Tauri 2 支持交叉编译，但需要额外配置目标工具链：

```bash
# macOS → Windows（需要 Windows 目标工具链）
npm run tauri build -- --target x86_64-pc-windows-msvc

# macOS → Linux（需要 Linux 目标工具链）
npm run tauri build -- --target x86_64-unknown-linux-gnu
```

> 详细交叉编译配置参见 [Tauri Cross-Compilation Guide](https://v2.tauri.app/guides/building/cross-compile/)

### CI/CD 集成

可使用 [GitHub Actions](https://github.com/features/actions) 自动构建多平台安装包：

```yaml
# .github/workflows/build.yml 示例
on: [push, pull_request]
jobs:
  build:
    strategy:
      matrix:
        os: [macos-latest, ubuntu-22.04, windows-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: actions/setup-node@v4
        with: { node-version: 20 }
      - run: npm install
      - run: npm run tauri build
```

## 开发指南

### 代码检查

```bash
# 前端类型检查
npm run check

# Rust 编译检查
cd src-tauri && cargo check

# Rust 测试
cd src-tauri && cargo test
```

### 添加新的 PDF 操作

1. 在 `src-tauri/src/commands/pdf_ops.rs` 中添加 Rust 命令处理函数
2. 在 `src-tauri/src/lib.rs` 的 `invoke_handler!` 中注册新命令
3. 在 `src/views/Tools.svelte` 中添加前端 UI 和调用逻辑

### 添加新的 S3 操作

1. 在 `src-tauri/src/commands/s3_ops.rs` 中添加命令函数
2. 在 `src-tauri/src/lib.rs` 中注册
3. 在 `src/views/Storage.svelte` 中添加前端交互

### 国际化

翻译文件位于 `src/lib/i18n/locales/`，当前支持：
- `en.json` — 英文
- `zh.json` — 中文

添加新语言：复制 `en.json`，翻译所有键值，然后在 `src/lib/i18n/index.svelte.ts` 中注册。

## S3 云存储配置

支持所有 S3 兼容的对象存储服务：

| 服务商 | Endpoint 示例 | 备注 |
|--------|------------|------|
| AWS S3 | `https://s3.amazonaws.com` | 勾选「强制路径风格」 |
| MinIO | `http://localhost:9000` | 开发测试常用 |
| Cloudflare R2 | `https://<account>.r2.cloudflarestorage.com` | 无路径风格 |
| 阿里云 OSS | `https://oss-cn-hangzhou.aliyuncs.com` | S3 兼容模式 |
| 腾讯云 COS | `https://cos.ap-guangzhou.myqcloud.com` | S3 兼容模式 |

配置路径：设置 → S3 兼容存储 → 启用并填写连接信息。

## 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `←` / `PageUp` | 上一页 |
| `→` / `PageDown` | 下一页 |
| `Ctrl/Cmd` + `=` | 放大 |
| `Ctrl/Cmd` + `-` | 缩小 |
| `Ctrl/Cmd` + `0` | 重置缩放 |
| 滚轮 / 触控板 | 自由缩放 |
| `F11` | 全屏切换 |
| `Esc` | 退出全屏 |

## 后续更新计划

### 短期（v0.2）
- [ ] 表格提取功能（Table Extraction）
- [ ] 深色模式下的水印/签名预览优化
- [ ] 编辑工具撤销/重做（Undo/Redo）
- [ ] PDF 页面拖拽排序
- [ ] 批量文件处理（文件夹导入）

### 中期（v0.3）
- [ ] PDF 页面内容编辑（替换文本）
- [ ] PDF 表单填写
- [ ] PDF 批注（高亮、下划线、便签）
- [ ] PDF 安全（加密/解密、密码保护）
- [ ] 页面裁剪

### 长期（v1.0）
- [ ] 云端同步（S3 自动备份/恢复配置文件）
- [ ] 多标签页同时打开多个 PDF
- [ ] PDF 书签管理
- [ ] PDF 对比（Diff）
- [ ] 插件系统
- [ ] 自动更新（Tauri updater 插件）

## 已知限制

- PDF 内容级别的文本编辑暂不支持（仅支持页面级操作：添加文字/矩形/高亮覆盖层）
- OCR 识别依赖外部 Tesseract 引擎，未安装时仅使用 pdfjs 内置文字提取
- S3 存储操作依赖网络连接，离线时不可用

## 许可证

MIT
