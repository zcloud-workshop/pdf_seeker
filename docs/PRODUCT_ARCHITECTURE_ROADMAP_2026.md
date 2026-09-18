# PDF Seeker 产品、架构与开发路线（2026 重规划）

> 状态：提案基线 · 日期：2026-09-16  
> 适用范围：桌面端 PDF Seeker（Tauri 2 + Svelte 5 + Rust）  
> 本文替代旧的“大而全 PDF 工具箱”路线；旧专项设计仍可作为历史实现记录，但优先级以本文为准。

## 1. 执行摘要

PDF Seeker 不应继续以“功能数量”追赶 Adobe Acrobat、Foxit、PDFgear、UPDF 或 Stirling-PDF。查看、合并、拆分、旋转、OCR、压缩等基础能力已经高度商品化，免费产品也能覆盖多数单次任务。

新的产品定位是：

> **隐私可证明的本地 PDF 工作流工作台：文件默认不出设备；重复任务可保存为配方；批处理结果可检查、可恢复、可复现。**

首批目标用户：经常处理扫描件、报销材料、档案、研究资料、法律或财务文件的个人专业用户与小团队。核心购买动机不是“更多按钮”，而是：

1. 避免敏感文档上传云端；
2. 将重复处理步骤保存并复用；
3. 批量执行时减少漏项、覆盖和命名错误；
4. 失败后能定位、重试和验证输出；
5. 避免长期订阅和账号锁定。

产品优先级统一为：

**可靠写入 > 批处理工作流 > 可控压缩 > 本地 OCR > 本地工作区 > 可选 S3 > 可选 AI。**

## 2. 市场判断与产品边界

### 2.1 竞品启示

| 竞品 | 已验证的市场供给 | PDF Seeker 的策略 |
|---|---|---|
| Adobe Acrobat | 深度编辑、表单、签署、保护、企业信任和订阅生态 | 不追逐完整编辑器、企业签署和协作体系 |
| Foxit | Acrobat 式功能与永久授权并存 | 验证“一次性授权 + 离线核心能力”可行，但不正面比拼功能深度 |
| PDFgear | 免费、免登录、编辑/OCR/AI 等基础能力 | 基础工具不能作为收费墙或核心差异化 |
| PDFsam Basic | 免费、离线、无水印的页面操作 | 页面操作必须可靠且免费；差异化来自工作流与审计 |
| UPDF | 现代 UI、跨平台、一次性授权、AI 营销 | 保留现代体验与一次性购买，不把泛 AI 作为首发主线 |
| Stirling-PDF | 本地/自托管、工作区、流水线、文件夹扫描与 API | 直接验证工作流需求；PDF Seeker 用原生桌面、低配置成本和可视化检查取胜 |

市场来源（2026-09 查阅）：[Adobe Acrobat 定价](https://www.adobe.com/mena_en/acrobat/pricing.html)、[Foxit PDF Editor](https://www.foxit.com/pdf-editor/)、[PDFgear](https://www.pdfgear.com/)、[PDFsam Basic](https://pdfsam.org/pdfsam-basic/)、[Stirling-PDF 文档](https://docs.stirlingpdf.com/)。这些来源证明竞品供给和厂商定位，不等同于逐项付费意愿证明。

### 2.2 明确不做

在本路线的前四个阶段内，不做：

- 像 Word 一样重排原文的“完整 PDF 编辑器”；
- 请求签署、团队协作、云账号、云同步和企业 DMS；
- 法律级脱敏、安全删除、合规审计承诺；
- 广泛 Office 格式高保真转换；
- PDF 聊天、自动摘要或代理式文档操作作为主功能；
- 默认上传 S3 或任何默认联网处理；
- 未经真实测量的“约 10MB”营销承诺；
- 在没有真实密码实现前宣传加密、解密或密码保护。

### 2.3 免费版与 Pro 边界

**免费版**必须能完整完成单次任务，不加水印、不强制登录：

- 查看、搜索、缩放、缩略图与基础页面操作；
- 合并、拆分、删除、旋转、抽取、重排；
- 基础压缩；
- 单文件基础 OCR；
- 默认离线、无遥测和可删除的本地历史；
- 少量近期任务和配方。

**Pro**按“自动化规模与节省时间”收费，而不是按隐私收费：

- 大批次、并发队列、失败重试和文件夹监控；
- 无限配方、工作区、命名规则和任务报告；
- 批量 OCR、多语言包、图像预处理；
- 高级压缩策略和批量对比；
- S3 输出、CLI/API 与自动化钩子；
- 商用支持与优先更新。

优先验证“一次性授权”或“永久授权 + 一年更新”，不在可靠写入和工作流价值完成前建设订阅系统。

## 3. 当前工程基线

截至 2026-09-16，已经确认：

- 前端生产构建与 Svelte/TypeScript 检查通过；仍有可访问性 warning；
- Rust `cargo check`、Clippy 和 23 项测试通过；
- CI 已覆盖前端检查/构建与锁文件模式 Rust 检查/测试；
- 已建立 3 个确定性 PDF fixture 和 4 项 fixture 回归测试；
- fixture 已捕获并推动修复批量编辑后原始内容流丢失问题；
- 插入页面已完成 UI → IPC → Rust command 接入，但未完成真实 PDF 人工验收；
- 密码加密、解密与批量加密未提供；
- OCR 使用 PP-OCRv5/MNN，模型按需下载或导入；
- S3 已有浏览、上传、下载和版本能力，但不是核心离线路径。

主要架构热点：

| 文件/模块 | 当前规模 | 问题 |
|---|---:|---|
| `src/views/Editor.svelte` | 约 1382 行 | 渲染、编辑、打印、OCR、对话框和状态耦合 |
| `PageToolsDialog.svelte` | 约 871 行 | 多个无关工具及文件 IO 集中在同一组件 |
| `BatchTool.svelte` | 约 406 行 | 队列仅存在前端内存，取消和历史不能跨重启恢复 |
| `pdf_ops.rs` | 约 2432 行 | command、PDF 领域逻辑、文件事务和测试集中在单文件 |
| `ocr.rs` | 约 1014 行 | 模型管理、下载、引擎生命周期和识别流程耦合 |
| Tauri commands | 50 个 | command 粒度不统一，缺少版本化 DTO 和统一任务接口 |
| 前端自动测试 | 0 套配置 | 交互状态、参数映射和关键工作流缺少自动保护 |

## 4. 目标架构

### 4.1 总体原则

1. **本地单体，而非分布式系统**：不引入服务端、消息队列、Redis 或云数据库。
2. **命令薄、应用层厚、领域层纯**：Tauri command 只校验 DTO、调用 use case、映射错误。
3. **输入不可变、输出事务化**：不直接覆盖输入；同目录临时写入、重开校验、原子替换。
4. **长任务统一进入 Job 系统**：OCR、批处理、压缩、转图片、S3 传输都使用同一状态机。
5. **网络访问显式可见**：模型下载和 S3 是可审计网络步骤；PDF 本地处理不出网。
6. **渐进迁移**：先加边界和测试，再搬代码，不做一次性重写。

### 4.2 分层结构

```text
Svelte UI
├── app/                 应用壳、路由、主题、全局通知
├── features/
│   ├── viewer/          PDF 渲染、缩放、搜索、缩略图
│   ├── organize/        合并、拆分、旋转、删除、抽取、重排、插页
│   ├── annotate/        文字、矩形、高亮、水印、视觉签名
│   ├── workflow/        工作区、配方、队列、任务历史、结果检查
│   ├── ocr/             模型管理、识别、可搜索层
│   ├── compress/        压缩策略与前后对比
│   └── storage/         可选 S3 输出
├── entities/            DocumentSession、Recipe、Job、Artifact DTO/store
└── shared/
    ├── api/             强类型 Tauri IPC 客户端
    ├── ui/              通用组件
    └── lib/             i18n、路径显示、格式化

Tauri/Rust
├── commands/            薄 IPC 适配器
├── application/         Use cases：运行操作、创建任务、重试、取消
├── domain/
│   ├── document.rs      DocumentId、PageRange、OutputPolicy
│   ├── operation.rs     OperationSpec 与参数校验
│   ├── recipe.rs        多步骤配方
│   └── job.rs           Job/Step 状态机
├── pdf/
│   ├── io.rs            加载、事务写入、重开校验
│   ├── organize.rs      页面结构操作
│   ├── annotate.rs      内容流与资源写入
│   ├── extract.rs       文本/表格提取
│   └── compress.rs      压缩策略
├── ocr/                 模型注册、预处理、识别、可搜索 PDF
├── infrastructure/
│   ├── db/              SQLite 工作区、配方、任务、步骤、产物
│   ├── files/           原子输出、冲突策略、校验和
│   ├── network/         网络事件与显式权限
│   └── s3/              可选输出适配器
└── tests/fixtures/      可复现 PDF 回归样本
```

### 4.3 前端状态模型

取消“一个全局文件路径 + 多个全局编辑 store”的模式。每个标签对应独立 `DocumentSession`：

```ts
interface DocumentSession {
  id: string;
  inputPath: string;
  displayName: string;
  currentPage: number;
  zoom: number;
  sidebarMode: "pages" | "search" | "jobs";
  pendingOperations: OperationSpec[];
  selectedElementId: string | null;
  dirty: boolean;
}
```

全局仅保留：应用设置、工作区列表、任务摘要和网络状态。组件不得直接拼 Tauri 请求；统一通过 `shared/api` 中的强类型函数调用。

### 4.4 Rust 操作模型

所有 PDF 操作统一为可序列化的判别联合：

```rust
enum OperationSpec {
    Merge { inputs: Vec<PathBuf> },
    Split { ranges: Vec<PageRange> },
    Rotate { pages: PageSelection, angle: Rotation },
    Delete { pages: PageSelection },
    Extract { pages: PageSelection },
    Reorder { order: Vec<u32> },
    Insert { source: PathBuf, after_page: u32 },
    Watermark { spec: WatermarkSpec },
    Ocr { language: String, searchable_pdf: bool },
    Compress { preset: CompressionPreset },
}
```

command API 收敛为少数稳定入口：

- `analyze_document(input) -> DocumentAnalysis`
- `preview_operation(input, operation) -> OperationPreview`
- `run_operation(request) -> Artifact`
- `create_job(recipe, inputs, output_policy) -> JobId`
- `start_job(job_id)` / `cancel_job(job_id)` / `retry_step(step_id)`
- `get_job(job_id)` / `list_jobs(filter)`
- `list_models()` / `install_model(request)`

旧 command 在迁移期间保留兼容层，前端逐个切换后再删除。

### 4.5 文件写入事务

任何会写 PDF 的 operation 必须遵循：

1. 校验输入存在、可读、扩展名和 PDF 头；
2. 输入只读打开，禁止原地修改；
3. 在最终输出目录创建唯一临时文件；
4. 执行操作并保存；
5. 使用 `lopdf` 重开，校验页数、页面树和必需资源；
6. 可选运行 `qpdf --check` / 渲染烟测（CI）；
7. 根据冲突策略执行原子 rename；
8. 记录输入/输出大小、校验和、耗时和警告；
9. 失败时删除临时文件，保留原文件。

输出冲突策略只有三种：`fail`、`rename`、`replace_with_backup`。默认 `rename`。

### 4.6 Job 与配方模型

状态机：

```text
Draft → Queued → Running → Succeeded
                    ├── Failed → Retrying → Running
                    └── Cancelling → Cancelled
```

最小持久化表：

- `workspaces(id, name, created_at, updated_at)`
- `recipes(id, name, version, steps_json, output_policy_json)`
- `jobs(id, recipe_id, status, started_at, finished_at, summary_json)`
- `job_inputs(job_id, ordinal, input_path, input_hash)`
- `job_steps(id, job_id, ordinal, operation_json, status, progress, error_json)`
- `artifacts(id, job_id, step_id, path, size, sha256, validation_json)`
- `network_events(id, job_id, kind, destination, bytes, created_at)`

SQLite 使用 `rusqlite`，数据库访问放入 `spawn_blocking`；单机任务并发默认 1，允许用户调到 2–4，不建设分布式队列。

进度通过 Tauri events 推送，前端刷新后可从 SQLite 恢复，不依赖组件内存。

### 4.7 OCR 架构

- 主安装包不内置大型模型；模型包独立下载、校验 SHA-256、记录版本；
- 首发质量目标集中在中英日默认包，不用“100+ 语言”代替质量验收；
- 预处理独立为可测试步骤：旋转检测、灰度、二值化、去噪、倾斜校正；
- 产物分为纯文本、结构化 block 和可搜索 PDF 三类；
- OCR 输出必须记录语言、模型版本、置信度和处理时间；
- 模型下载属于显式网络事件，识别本身离线。

### 4.8 S3 边界

S3 从主导航能力降级为可选输出适配器：

- 默认关闭，不参与本地任务；
- 第一阶段仅支持任务完成后的单向上传；
- 凭据进入系统 Keychain，不写普通 TOML；
- 上传作为独立 Job step，可重试，不回滚已生成的本地文件；
- 记录目标 endpoint、bucket、key 和字节数，但不记录密钥；
- 云同步、多人共享和后台双向同步不在当前路线。

### 4.9 隐私与安全

- 默认无遥测、无账号、无联网；
- UI 显示本次任务是否发生网络访问；
- Tauri capability 从 `$HOME/**` 逐步收窄为用户选择路径、应用数据、下载和临时目录；
- 所有路径由 Rust 校验，前端路径仅用于展示；
- 密码支持分两步：先可靠识别和清晰错误；后续如引入经过审计的库，再提供解密/加密；
- 视觉签名明确标注为“签名图片”，不是数字证书签名；
- 不声称法律级脱敏或合规销毁。

## 5. 测试与发布门禁

### 5.1 回归金字塔

1. **Rust 单元测试**：页码范围、输出策略、状态机、路径校验；
2. **fixture command 测试**：真实 command 处理确定性 PDF；
3. **结构校验**：重开、页数、顺序、文本、资源、元数据；
4. **外部校验**：CI 使用 qpdf 检查结构，使用 Poppler 渲染关键页；
5. **前端组件测试**：Vitest + Testing Library 覆盖参数映射、队列和错误状态；
6. **Tauri E2E 烟测**：至少覆盖打开、页面操作、另存、重开；
7. **跨平台发布构建**：macOS ARM/Intel、Windows x64、Linux x64。

fixture 后续按风险扩展：

- 带图片和透明度；
- 不同 MediaBox/CropBox 与旋转页；
- 内联/引用 Resources 和 Contents 数组；
- 书签、元数据、附件和表单；
- 受密码保护 PDF（仅合法生成的测试密码）；
- 损坏 xref、缺失对象和超大页；
- 中文字体与扫描页。

### 5.2 发布硬门禁

候选版本必须满足：

- 所有自动测试通过；
- fixture 输出可重开，`qpdf --check` 无结构错误；
- 页面操作样本 100% 保持预期页数和顺序；
- 写入操作不得覆盖原件；
- 取消任务不得留下被误认为成功的最终文件；
- OCR 与 S3 的网络行为在 UI 可见；
- README 功能状态与 command/UI 实现一致；
- macOS、Windows 至少完成一次真实应用烟测。

## 6. 分阶段开发计划

时间是单人开发的建议工作量，不是发布日期承诺。每阶段必须独立可发布。

### Phase 0：可靠写入与真实验收（2–3 周，发布阻塞）

目标：从“能编译”升级到“不会悄悄损坏用户文件”。

- 建立 `pdf/io.rs` 事务写入层；
- 页面操作和编辑操作统一重开校验；
- fixture 扩充至图片、旋转、资源数组、中文、损坏输入；
- CI 增加 qpdf 结构检查和 Poppler 渲染烟测；
- 对合并、拆分、旋转、删除、抽取、重排、插页逐项人工验收；
- 编辑功能继续标记实验性，直到视觉结果验收完成；
- 明确受密码保护文件的错误提示。

验收：核心页面操作在 fixture 与真实样本上 100% 通过；任何失败不覆盖输入；输出均可重开和渲染。

### Phase 1：本地工作区与统一 Job 引擎（3–4 周）

目标：建立产品真正的差异化骨架。

- 引入 SQLite 与 Job/Step/Artifact 状态机；
- 把 `BatchTool.svelte` 的循环迁移到 Rust Job runner；
- 支持取消、失败重试、恢复、输出冲突策略和结果清单；
- 新建 Workspace 视图：输入、配方、队列、结果四区；
- 第一批配方：页面整理、批量旋转、批量水印、批量转文本；
- 前端每个标签使用独立 `DocumentSession`。

验收：应用重启后任务历史和未完成状态可恢复；单文件失败不影响其他输入；结果清单可导出。

### Phase 2：可控压缩与批处理产品化（2–3 周）

目标：提供可量化、可付费的时间节省。

- 压缩预设：保真、平衡、较小、灰度、自定义；
- 处理前估算、处理后大小/节省比例；
- 样本预览和输出比较；
- 命名模板、输出目录、冲突策略；
- 配方保存、复制、导入导出；
- 本地隐私面板和网络事件记录。

验收：批次报告包含每个输入的结果、大小变化、耗时和错误；压缩不承诺绝对“无损”。

### Phase 3：本地 OCR 质量闭环（3–4 周）

目标：完成扫描资料处理主场景。

- 中英日默认模型包和模型完整性校验；
- 旋转/倾斜/去噪预处理；
- 单文件 OCR 免费；批量 OCR 进入工作流；
- 输出纯文本、结构化结果和可搜索 PDF；
- OCR 结果抽样检查、置信度展示和失败重跑；
- 添加扫描件、中文字体与混合文档 fixtures。

验收：固定扫描样本有可重复的字符准确率基线；断网时已安装模型可完整运行；模型下载失败可恢复。

### Phase 4：Pro 自动化与可选输出（3–4 周）

目标：验证商业价值，而不是扩展功能清单。

- 无限配方和批次；
- 文件夹监控与延迟稳定检测；
- CLI 调用现有 Job/Recipe 核心；
- S3 单向输出 step、失败重试与 Keychain 凭据；
- 一次性许可证验证，不要求账号登录；
- 本地生成使用报告，默认不上传。

验收：同一配方可由 GUI、文件夹监控和 CLI 得到一致产物；S3 故障不损坏本地结果。

### Phase 5：选择性增强（需求验证后）

仅在前四阶段有真实留存和付费信号后选择：

- PDF Diff；
- 书签与目录；
- 高级表格提取；
- 可选 BYOK AI 信息提取；
- 数字证书签名；
- 更多企业输出适配器。

AI 必须在每次调用前说明是否出网、使用哪个提供商、发送哪些页面；不能破坏离线默认值。

## 7. 近期执行队列（下一轮）

按顺序执行，不并行铺开：

1. 为现有 fixture 增加 qpdf/Poppler CI 校验；
2. 引入 `pdf/io.rs`，实现临时写入、重开校验和原子提交；
3. 将合并、旋转、删除、抽取、重排、插页迁移到事务写入；
4. 补带图片、旋转页、Contents 数组和中文文本 fixtures；
5. 用真实 PDF 完成页面操作验收矩阵；
6. 设计 SQLite schema 和 Job 状态机测试；
7. 把批量旋转迁移成第一个持久化 Job；
8. 建立 Workspace 最小 UI，再迁移其余批处理操作。

在完成第 5 项前，不将编辑功能从实验性改为可用；在完成第 8 项前，不开发 AI、协作或云同步。

## 8. 成功指标

不依赖强制遥测，主要通过本地测试、可选反馈与发布质量衡量：

- fixture 和真实样本无文件损坏回归；
- 核心 Job 成功率 ≥ 99.5%（排除明确无效输入）；
- 取消后残留最终产物为 0；
- 批量失败可定位率 100%；
- 应用崩溃后任务状态可恢复率 100%；
- 单次任务从打开到执行不超过 3 个主要决策步骤；
- 首批用户每周重复使用同一配方的比例，是比“安装量”更重要的产品信号；
- Pro 转化应来自批量、配方、自动化和输出集成，而不是基础隐私能力。
