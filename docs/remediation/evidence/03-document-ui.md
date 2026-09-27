# 03 文档会话与编辑界面 — 实施证据

分支 `codex/remediation-03`,基线 `cd01590`(docs 提交;其父 `6e73574` = `origin/main`)。
环境: macOS 27 arm64, Node v24.18.0 (nvm), npm + 既有 `package-lock.json`(01 的 pnpm/锁文件迁移未合并,按 03 文档"记录本地实际环境, 不另造锁文件/测试 runner"执行;独立 worktree `../pdf_seeker_03`,符号链接共享 checkout 的 node_modules)。

## 验证结果

| 命令 | 结果 |
| --- | --- |
| `npx svelte-check --tsconfig ./tsconfig.json` | 1 error + 3 warnings — 全部为基线存量且在他人独占文件: `Home.svelte:25`(01 包)、`Storage.svelte` ×3(05 包)。本包独占文件(Tools/PdfScroller/Tooltip/stores/pdf-engine)零错误零警告;基线中本包文件上的 2 个 a11y 警告(Tooltip mouseenter 无 role、Tools 缩略图网格无键盘)已随修复消除,未用 ignore 注释掩盖 |
| `npm run build` | ✓ built (vite), 产物正常 |
| `node tests/remediation-03/run.ts` | 21/21 通过(零依赖测试,Node ≥22.6 type-strip 直跑;无新增依赖/runner/锁文件) |
| `node tests/remediation-03/samples/make-samples.mjs` | 3 个样本生成,xref 自检通过 |
| `node tests/remediation-03/samples/verify-samples.mjs` | 3/3 通过: `rotate-90.pdf` rotate=90 vp=100×200、`cropbox-offset.pdf` vp=540×720(CropBox 生效)、`float-a4.pdf` vp=595.28×841.89,文本层均可提取 |

GUI 行为验证(验收表 8 项)需 Tauri 运行时,本包未在 headless 环境执行,见"残余限制"。

## 各 R 项落地方式

### R26 pdfjs 类型
- 删除 `src/pdfjs-dist.d.ts`(过窄手写声明,遮蔽真实类型),删除 `pdf-engine.ts` 中的 `getTextContent` 模块增强(真实类型已含)。
- 检索全部消费者: `pdf-engine.ts`(getDocument/PDFDocumentProxy/PDFPageProxy)、`PdfScroller.svelte`(getOutline/getDestination/getPageIndex/getViewport/render)、`Tools.svelte`(getTextContent items 用 `"str" in item` 区分 TextItem/TextMarkedContent, 替换原 `(item: any)`)。Compare 不直接 import pdfjs-dist(grep 证实)。check/build 通过,未放宽 tsconfig。
- OCR samples 脚本经 `pdfjs-dist/legacy` 在 Node 中验证真实类型的运行时行为一致。

### R12 会话状态(新模块 `src/lib/document/`)
- `session.svelte.ts`: 按文件路径管理 generation / writeBusy / 历史(FileHistory)/ 视图(页码+缩放)。Tools→Viewer→Tools 或标签切换后从会话恢复。
- `history.ts`: 30 条 × 64 MiB 双栈字节预算(undo 优先驱逐最旧,redo 共享预算),node 直测。
- 释放规则: `stores.closeTab` → `releaseSession(path)`(关闭标签是唯一显式释放点;视图切换不释放)。
- 撤销能力 = 会话历史非空,与 `isEditingTool`/大预览解耦: 工具面板头部显示 Undo/Redo(仅当历史非空),replaceText 面板因此可撤销/重做;`onEditKeydown` 去掉 isEditingTool 门控,新增 input/textarea/contenteditable 排除(不吞原生文字撤销)与 busy 门控(R06)。PdfScroller 的 Cmd+Z 同样排除 contenteditable 并检查 outlineSaving。
- 就地编辑行为保留,未静默改为工作副本。

### R06 前端互斥
- `beginFileWrite/endFileWrite` 按文件互斥: 编辑/撤销/重做/快捷键共用同一入口;二次进入直接拒绝。
- 历史"成功后才移动": 编辑先读 `before`,invoke 成功后才 `pushUndo`;失败尝试用 `before` 恢复磁盘字节,历史不动(可重试)。撤销/重做 peek→写→成功后 pop/push;失败用起始字节恢复,历史不动。
- 残余(转 07-A): 后端 `writeFile` 快照恢复非原子替换;跨进程写版本校验。

### R07 文件身份
- 所有 execute*/load 系函数在 await 前捕获 path,await 后 `fileSwitched(path)` 拒绝应用到 UI(loadFormFields/findReplaceMatches/executeTable/executePdf2Text/loadThumbnails/OCR/applyEditInPlace/undo/redo)。
- 新增文件切换 `$effect`: 清空缩略图(先 destroy)、表单、匹配坐标、选择状态、OCR 状态、表格结果等全部文档态;若当前工具需要数据(hasPreview/pdf2text/form/ocr)立即自动加载 — 覆盖"无文件时进入重排工具再 Open PDF 不加载缩略图"。

### R11 前端
- `loadDocument` 区分同文件刷新与真换文件: 同文件刷新不清 `outlineUndoSnapshot`、保留页码/缩放(页码 clamp 到新页数);真换文件才重置。书签保存→reload→Cmd+Z 仍可恢复。
- `OutlineNode.targetKind ∈ {page, external, unresolvable}`;`applyOutlineEdit` 遇 external/unresolvable 目标时明确报错阻止写回(列出场数与示例标题),不再过滤后保存子集。深层 Count 修复属 02/07-C。

### R10 前端
- `formInitial` 记录加载值;仅提交 `value !== initial` 的字段 — 主动清空("")会被提交,未修改字段(含未知导出值)不提交。
- checkbox 显示用 `isTruthyExportValue`(yes/true/1/on/checked),未知导出值原样展示并标注"unknown export value — left unchanged unless toggled";不再猜成 Yes。radio 当前值不在 options 时追加为选项,显示真实值。

### R13 前端
- `ocrTaskSeq` 任务身份: 新任务/换文件/切工具/组件销毁均使旧任务全部 await 点失效;过期结果不写 UI。
- 按页决策: 文字页走内置提取,无文字页进 Tesseract;混合文档不漏页。无 Tesseract 不再禁用按钮 — 文字页照常导出,扫描页逐页列出"skipped"。
- 渲染失败逐页记录(页号+原因),不再静默 continue;PNG 文件名用零填充**原始页号**,后端按文件名排序返回的段映射回真实页号,合成全文按真实页序输出 `--- Page N ---`。
- 残余(转 07-D): 逐页结果协议、统一取消 UI、任务目录完整清理跟踪。

### R14 生命周期
- PdfScroller `docGeneration`: 每次 load 递增,readFile/loadPdf/尺寸/outline 各 await 点后校验;过期文档立即 destroy;新文档就绪后才 destroy 旧文档。onDestroy 追加 zoomRaf/scrollRaf 取消、docGeneration 递增、焦点释放。
- Tools `thumbGeneration` + `disposeThumbDoc`(替换前 destroy);table/replace/pdf2img/OCR 的临时文档 finally destroy;onDestroy 递增 thumbGeneration/largeRenderSeq/ocrTaskSeq 并释放 thumbDoc。
- 大预览串行队列 `scheduleLargeRender`: 同一 canvas 上请求按序执行,新请求使旧请求成为 no-op(固定参数在调度时捕获)。

### R15 页范围与预算
- `pageRanges.ts` 严格解析: 全词匹配拒绝 `3abc`/`1-`/`-3`/空项;拒绝 0、倒序、越界(已知总页数时)、展开超 2000 页预算。delete/extract/pdf2img 输入即时行内报错且保留上次有效选择;split ranges 前端语法校验(后端仍复验)。
- Viewer: 首屏只等第 1 页尺寸,其余页以第 1 页尺寸估计立即布局,真实尺寸渐进替换;页面缓存 40 条上限改为 128 MiB 像素字节预算(驱逐最旧,重置时清零计数);离屏(出 400px 扩展视口)slot canvas 置 0×0 释放像素,回视口从缓存重绘。
- 残余: Tools 缩略图网格仍全量渲染(1000 页场景预算验收留 07-D/GUI);性能阈值按 03 文档以固定机器基线另行记录。

### R27 交互
- 表格工具: 缩略图网格点击/键盘真正选中 `tablePage`(此前提示点击但无处理)。
- 水印: `watermarkPage` + setPreviewPage/previewPage 派生 → 大预览可翻页,缩略图条高亮。
- `parseFloat(v)||def` 全部 36 处替换为 `numOr/intOr`(仅 NaN/空回退,0/0.0/-45 可表达);页码输入 clamp 到 [1, pageCount]。`numOr`/`intOr` node 直测。
- Tooltip: focusin/focusout(键盘 Tab 可触发,原 focus/blur 不冒泡无效)、`aria-describedby` 关联 + `role="tooltip"`,消除基线 a11y 警告;缩略图网格加 role/tabindex/Enter/Space,删除既有 ignore 注释。完整字典文案转 07-E(本包新增文案沿用现状英文硬编码风格)。

### R29 scroller 边界
- 新模块 `viewerFocus.ts`: 单一焦点持有者。active 的 scroller 挂载时若无人认领则认领;用户 pointerdown 所在面板重新认领;仅持有者挂载 window keydown 并写共享 zoomLevel/currentPage/totalPages。Compare 双 active 时由交互决定归属,未改 Compare 任何代码;inactive 仍加载/渲染页面(未动)。

### R28 前端
- pdf2img: 输出前 `list_dir_files` 预检同名 PNG → `ask` 对话框明确覆盖/取消,拒绝时中止;成功/失败按页计(`Exported X of Y; failed: …`),不再显示虚假总数。
- OCR 结果同为诚实计数(见 R13)。
- 残余(转 07-D): split 的后端命名冲突(输出名由 02/07 控制)、结构化部分失败明细、统一取消/重试。

### R08 现状记录(仅样本+记录,不改坐标)
- 现状: `Tools.svelte` `toX/toY` 与 `onLargePointerUp` 的 `toPdfX/toPdfY` 只按预览宽高等比缩放 + y 翻转,**未**处理 /Rotate 与非零 CropBox/MediaBox 原点;而后端(pdf_ops.rs)按整数 MediaBox 右上角当宽高并忽略继承 Rotate(02 包范围)。pdfjs `getViewport` 本身已正确反映旋转与 CropBox(样本验证: rotate-90 → 100×200, cropbox → 540×720),因此预览显示正确但**点击/拖框坐标换算错位** — 这正是 07-B 统一坐标协议要解决的。
- 样本: `tests/remediation-03/samples/`(生成器 + 已验证产物): rotate-90、cropbox-offset、float-a4。

## 交付给后续包的缺口

- **07-A**: 旧 `writeFile` 快照恢复无后端原子替换(当前仅字节恢复);跨进程版本校验;会话 busy 入口已就绪(`beginFileWrite`),需后端事务配合关闭 R06。
- **07-B**: 坐标协议统一(样本已备);Tools 前端换算点在 `renderLargePreview` 的 `toX/toY` 与 `onLargePointerUp` 的 `toPdfX/toPdfY`。
- **07-C**: 完整书签目标协议(现以阻止写回代替);表单真实 widget/导出值契约(现以"只提交修改+不猜测"收敛);新能力说明文案。
- **07-D**: OCR 逐页结果协议/取消/清理跟踪;split 输出命名冲突;Tools 缩略图可视窗口加载;性能基线记录。
- **07-E**: 本包新增英文文案的完整翻译(未调用不存在的字典键);Tooltip/对话框键盘路径的全面复核。

## 残余限制

1. GUI 验收表 8 项(标签切换污染、跨视图历史保留、写入失败历史、输入框 Cmd+Z、书签刷新、快速翻页、非法范围/1000 页、表格选页/数值 0)逻辑均已实现并有单测覆盖对应纯逻辑,但未在 Tauri GUI 中人工执行 — 需在本 PR 合并前或 07 阶段补 GUI 证据。
2. 测试入口: `node tests/remediation-03/run.ts` 可复现,但未接入 `package.json` scripts(01 交付统一测试配置后接入;03 文档允许先交接)。
3. 关闭标签时会话释放后,若该文件仍有在途写入,写入仍指向被捕获的原路径(正确文件),其 `pushUndoSnapshot` 会重建已释放的会话对象(小对象,无泄漏放大;07-A 接版本校验时一并处理)。
