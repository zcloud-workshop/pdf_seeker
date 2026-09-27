# 整改实施分工与合并规则

状态: 01-06 已全部实施并合并(PR #5-#11, 2026-09-27), 07 实施中. 拆分日期: 2026-09-27.

审查依据: [原始审查计划](../code-review-and-remediation-plan.md). 审查代码基线为 `6e73574`, 本次拆分前 HEAD 为 `cd01590f47237bce95e31d80020ce2fd6bf74d34`. 原文行号是定位线索, 开工时必须按函数名和引用重新定位.

## 1. 如何分给不同的人

先把这组文档合入 `main`, 再由分配者公布该次合并后的完整 commit SHA 作为统一分发基线. 各人从该基线建立自己的分支, 只推自己的分支, 分别提交 PR 到 `main`. 不要求每个人开发期间不断跟随其他人的未合并分支.

| 包 | 可直接交付给实施人的文档 | 分支 | 主要职责 | 启动条件 |
| --- | --- | --- | --- | --- |
| 01 | [工程基线与交付门禁](01-engineering.md) | `codex/remediation-01` | 工具链, CI, 发布, Home 类型, 目录扫描, CSP | 分发基线 |
| 02 | [PDF 后端正确性](02-pdf-backend.md) | `codex/remediation-02` | PDF 结构, 后端保存, 加密, 兼容性回归 | 分发基线 |
| 03 | [文档会话与编辑界面](03-document-ui.md) | `codex/remediation-03` | Tools, 阅读器, 会话, 历史, 渲染与交互 | 分发基线 |
| 04 | [文档对比](04-comparison.md) | `codex/remediation-04` | 对比请求生命周期, diff 预算, 面板焦点 | 分发基线 |
| 05 | [设置、S3 与更新](05-settings-storage.md) | `codex/remediation-05` | 配置完整链路, S3, Input, 主题语言, updater | 分发基线 |
| 06 | [插件运行可靠性](06-plugins.md) | `codex/remediation-06` | 输出预算, 超时, 身份与路径校验 | 分发基线 |
| 07 | [协议联调与发布验收](07-integration.md) | `codex/remediation-07` | 跨边界协议, 全链路验证, 支持矩阵 | 对应基础包已合并 |

01-06 没有相互依赖的未合并提交, 可以同时开发、分别合并. 07 的各项有明确前置条件, 可以提前准备样本和测试方案, 不能宣称与 01-06 完全无依赖.

这里的独立合并是指: 每个 PR 包含自己功能所需的代码与测试, 与当时主分支兼容, 不要求另一个未合并 PR 才能运行. 它不表示 Git 永远无冲突, 也不表示一个包合并就关闭横跨多个包的整条 R 问题. 必须联合改协议的内容统一放在 07, 不把半套协议藏在独立包中.

02、03、05 工作量明显大于 04、06, 不承诺平均分工. 同一负责人可将包内工作拆成多个可运行 PR, 每个 PR 在当前主分支上独立验证. 不把同一个大文件再次交给多个人同时重写; 若必须增加人数, 先合并一轮模块提取并重新公布文件归属.

## 2. 文件归属

下表是第一轮排他写入范围. 未列出的已有文件默认只读. 各包的新模块和测试放在自己的目录, 不新建另一个包也要编辑的统一汇总文件.

| 包 | 独占的已有文件和目录 | 可新增的专属目录或文件 |
| --- | --- | --- |
| 01 | `package.json`, `package-lock.json`, `.github/workflows/`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src/views/Home.svelte`, `src-tauri/src/commands/fs_utils.rs` | `.nvmrc`, `.node-version` 二选一, `pnpm-lock.yaml`, `rust-toolchain.toml`, 前端测试配置, `scripts/remediation-01/`, `tests/remediation-01/` |
| 02 | `src-tauri/src/commands/pdf_ops.rs`, `src-tauri/src/commands/security.rs` | `src-tauri/src/commands/pdf_ops/`, `src-tauri/src/commands/security/`, `src-tauri/tests/remediation_02/`, `tests/remediation-02/` |
| 03 | `src/views/Tools.svelte`, `src/views/Viewer.svelte`, `src/lib/components/viewer/PdfScroller.svelte`, `src/lib/stores/index.ts`, `src/lib/components/layout/TabBar.svelte`, `src/lib/components/ui/Tooltip.svelte`, `src/lib/pdf-engine.ts`, `src/pdfjs-dist.d.ts` | `src/lib/document/`, `src/lib/components/tools/`, `tests/remediation-03/` |
| 04 | `src/views/Compare.svelte`, `src/lib/utils/diff.ts` | `src/lib/comparison/`, `tests/remediation-04/` |
| 05 | `src/views/Settings.svelte`, `src/views/Storage.svelte`, `src/lib/components/ui/Input.svelte`, `src/lib/types/index.ts`, `src/lib/i18n/`, `src/lib/components/layout/Toolbar.svelte`, `src/main.ts`, `src/lib/updater.ts`, `src-tauri/src/config/mod.rs`, `src-tauri/src/commands/config.rs`, `src-tauri/src/commands/recent.rs`, `src-tauri/src/commands/s3_ops.rs`, `src-tauri/src/lib.rs` | `src/lib/settings/`, `src/lib/storage/`, `src-tauri/src/config/credentials.rs`, `tests/remediation-05/` |
| 06 | `src-tauri/src/commands/plugins.rs`, `docs/plugins.md`, `docs/plugin-example/` | `src-tauri/src/commands/plugins/`, `tests/remediation-06/` |
| 07 | 第一轮结束后按联合 PR 逐项接管涉及文件; `README.md` 的最终更新 | `tests/remediation-07/`, `docs/remediation/evidence/07-*.md` |

所有负责人只更新自己的实施文档或 `docs/remediation/evidence/NN-*.md`, 不同时修改本总表和原始审查的勾选项. 分配者在合并后汇总. `src/App.svelte`, `src-tauri/src/commands/mod.rs`, `src-tauri/src/error.rs` 第一轮保持只读; 新子模块从本包拥有的父模块声明, 不为整理结构改公共入口.

依赖清单、锁文件和 CI 由 01 统一维护. 其他包需要新依赖或公共测试配置时, 先提交具体需求给 01, 由 01 交付独立前置 PR. 在它合并前继续不依赖该变化的工作, 依赖它的 PR 必须更新主分支后才能合并. 不为保持表面并行而复制第三方实现, 也不手工拼接锁文件. 发生这种依赖时如实记录, 该子任务不再属于任意顺序合并的范围.

## 3. 第一轮兼容约束

1. 02 不更名或删除已有 PDF IPC 命令, 不改变已有字段含义和返回结构. 外部修改版本校验、新的撤销恢复命令、书签完整目标、OCR 有序页面结果和几何协议放到 07. 可在旧协议下拒绝明确会损坏文件的输入, 并返回已有错误通道.
2. 03 保留现有 stores 导出名、`openTab(path)` 等公共调用形式, 保留 `Tab` 的既有字段. 新会话状态放到自己目录, 对旧入口做兼容接入. 不要求 Home、Toolbar、Storage、Compare 同时改调用.
3. 03 保留 `PdfScroller` 的 `tab`、`active`、`initialPage` 和 `getScrollContainer()` 接口. `active=false` 继续允许显示页面, 仅停用活动面板的全局快捷键/共享阅读状态写入, 不能使对比右侧消失.
4. 04 继续消费 `extract_page_texts(path)` 的 `string[]` 返回. 通过现有 `active` 属性控制焦点, 不引用 03 尚未合并的会话模块.
5. 05 可在自己的 S3 前后端链路内同时修正 DTO/key, 必须检索全部消费者并同 PR 更新. 配置 `get_config`/`update_config` 和最近文件入口保留旧消费者所需字段; 新字段有兼容默认值. 保留 `isDark` store 的使用方式, 不改它所在文件.
6. 06 保留 `list_plugins`、`run_plugin` 及 Tools 使用的请求/结果字段. 不改 Tools 以适配插件内部重构.
7. 全局字典由 05 独占. 其他包第一轮沿用已有键, 必须增加的完整翻译和跨页文案收口到 07, 不能调用尚不存在的键. 01 的 CSP 必须覆盖已有 worker、文件和更新路径; 新 worker/网络源必须做兼容复核.

如无法在上述约束下完成某项, 在本包记录尚未完成的子项并转交 07. 不静默削减原始验收, 不把协议变化伪装成内部实现.

## 4. Git 协作流程

### 4.1 开工与提交

先保证工作区干净. 同一台机器多人操作时, 使用各自 clone 或独立 worktree, 不共用一个可写 checkout. 每份实施文档给出了自己的建分支命令; 分配者公布基线后核对 SHA, 如 `origin/main` 已前进则明确选择公布的提交创建分支.

每个文件分别执行 `git add -- 文件路径` 和 `git commit -m 'type: desc'`, 不使用 `git add .` 混入他人文件. 一个功能可以有多个文件提交, 但把完整可运行的提交组放在一个 PR 内合并. 不要求每个中间文件提交独立发布, 不单独挑取半套功能. 提交类型使用 `feat`、`fix`、`refactor`、`docs` 或 `chore`.

### 4.2 每次合并前同步主分支

在自己的任务分支、工作区干净且当前修改已提交时执行:

```sh
git fetch origin
git merge origin/main
```

解决冲突后检查完整 diff, 重新运行本包验证和最新主分支门禁, 再执行:

```sh
git push
```

不要在任务分支直接使用不带目标的 `git pull` 来代替同步 `main`: 它通常更新的是当前分支的 upstream. 首次 push 见各包的 `git push -u` 命令.

此流程用 merge 保留已推送历史, 通常不需要强推. 不运行 `git push --force`, 不把本地任务分支强行推到 `main`. 私人分支若另行约定 rebase, 重写已推送历史也必须遵守团队规则; 本计划不要求 rebase.

### 4.3 push 被拒绝时

| 现象 | 原因 | 处理 |
| --- | --- | --- |
| 自己分支提示 non-fast-forward | 远端同名分支已有本地没有的提交, 不是单纯因为 main 前进 | fetch 后合并该远端分支, 再合并 main, 验证后 push |
| 直接 push main 提示受保护或拒绝 | 向共享主分支写入或违反仓库规则 | 将本地提交放到独立分支并通过 PR 合并 |
| push 成功但 PR 提示冲突/落后 | PR 与当前 main 有文本冲突或需要最新检查 | 按 4.2 同步, 人工解决并重新验证 |
| permission denied/认证失败 | 账号、SSH key 或仓库权限问题 | 修正权限或用 fork PR, 不用强推或 reset 解决 |
| CI 失败 | 工具链、现有失败或功能回归 | 阅读具体日志并修复/记录阻塞, 不能通过重复 push 视为解决 |

在已完成首次 push 且 upstream 是本人任务分支的前提下, non-fast-forward 可按以下流程保留双方提交:

```sh
git fetch origin
git merge '@{upstream}'
git merge origin/main
```

如果第一次 merge 冲突, 先完成该次合并, 不继续执行第二次. 用 `git status` 和下列命令查看冲突文件:

```sh
git diff --name-only --diff-filter=U
```

逐个阅读双方意图、编辑并暂存解决后的文件, 再执行 `git merge --continue`. merge 冲突解决属于同一个 Git 合并提交; 正常功能修改仍逐文件提交. 不确定语义时用 `git merge --abort` 回到合并前状态并联系文件负责人, 不整文件选择 ours/theirs 丢掉另一方修复.

### 4.4 合并队列

一次只合并一个已验证 PR. 每合入一个 PR, 下一个 PR 都重新检查与最新 main 的兼容性; 即使没有文本冲突也检查协议和行为. 仓库如支持且已配置 merge queue 可使用它, 否则由分配者手工串行. 代码评审通过不能替代最新合并结果的 CI.

01 优先合并有助于统一工具链, 但它不是其他人开始阅读、编码和 Rust 回归的前提. 第一轮基线已有类型错误等已知失败, 每个包记录自己的最小验证和与基线相同的失败; 已知失败不等于通过. 01/03/05 的相关修复到位后要求全量 check 零错误, 07 发布门禁不允许沿用存量错误豁免. 如果仓库已有必需检查不通过, PR 就保持未合并直到修复进入 main, 不能为并行承诺关闭检查.

## 5. 问题覆盖与最终关闭责任

| 原审查项 | 第一轮负责人 | 联调或最终关闭条件 |
| --- | --- | --- |
| R01 | 05 | Input 双向绑定及设置回显 |
| R02, R03, R04, R05 | 02 | 结构测试加独立 PDF 渲染 |
| R06 | 02 后端保存, 03 前端互斥/历史 | 07-A 接通包括撤销在内的全部写入与版本校验后关闭 |
| R07 | 03 | 文件身份、过期回调; 与 07-A 联合验证 |
| R08 | 02/03 记录现状与准备样本 | 07-B 统一坐标协议, 第一轮不单边改单位 |
| R09 | 02 | 文字/字体/图形状态跨阅读器验证; 新依赖走 01 |
| R10 | 02 表单后端, 03 清空语义 | 07-C 补真实 widget/导出值契约并联验 |
| R11 | 02 后代 Count/安全拒绝, 03 历史/安全拒绝 | 07-C 完整目标与子树无损往返 |
| R12 | 03 | 会话历史和阅读状态; 安全恢复另依赖 07-A |
| R13 | 02 后端任务隔离, 03 前端任务状态 | 07-D 有序页面、逐页结果、所有退出路径清理 |
| R14, R15 | 03 | 生命周期、严格页范围和资源预算; 后端解析由 02 配合 |
| R16, R17, R18, R19, R20, R21, R22 | 05 | 配置/S3/UI 完整链路, 凭证迁移和真实服务验收 |
| R23 | 06 | 输出、进程树、身份与产物验证 |
| R24 | 02 | 独立加密实现互操作和明文清理 |
| R25 | 01 | 标签与手动发布演练; 真实发布前由 07-E 复核 |
| R26 | 01 工具链/Home/门禁, 03 pdfjs 类型, 05 Storage 导入 | 07-E 干净安装、全部检查与失败门禁 |
| R27 | 03 Tools/Tooltip, 05 Toolbar/Storage | 07-E 完整国际化、焦点和能力说明 |
| R28 | 01 扫描, 02 PDF 批量, 03 导出界面, 05 下载 | 07-D 结构化部分失败、进度/取消/重试 |
| R29 | 04 对比, 03 scroller 的 active 约束 | 07-E 验证两者组合; 页自动对齐/像素 diff 为后续需求 |
| R30 | 01 CSP/capability, 05 S3 认证/updater | 07-A 更新重启与编辑协调, 07-E 三平台核验 |

原计划 S0-S4 的拆分和清理落在各文件负责人包内, 仅在回归保护下进行; S5 和 README 能力矩阵落在 07. AES、新的有损压缩、真脱敏、自动远端版本清理、页自动对齐与像素 diff 保持原计划中的另行设计边界, 不作为本轮暗中增加的范围.

## 6. 所有 PR 的交付内容

- 对应 R 编号和本 PR 完成的具体子项, 尚未完成的子项及接手包.
- 修改文件与对外接口影响, 搜索调用点的结果, 是否引入公共依赖.
- 可复现样本、测试命令、结果与必要 GUI/独立阅读器证据; 不提交私人文档、凭证或带密码日志.
- 同步到的 main SHA、测试环境和残余限制. 新测试必须被实际测试入口执行, 不能只创建测试文件.
- 每个文件独立提交, 完整功能作为一个 PR 验证. 更新本包文档/专属证据文件, 不多人勾改总审查表.

当前没有代替实施者运行新的业务验证. 原审查结果是历史基线, 不能复制成修复后的验收证据.
