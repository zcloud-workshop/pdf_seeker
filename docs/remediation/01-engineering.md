# 01 工程基线与交付门禁

状态: 已实施, 待 PR 评审. 先读 [分工与合并规则](README.md). 对应 R25、R26、R28 的目录扫描、R30 的 CSP/capability. 可从统一分发基线独立开工.

## 1. 写入范围与独立合并条件

独占 package/锁文件、工具链配置、`.github/workflows/`、`src-tauri/Cargo.toml`/`Cargo.lock`、`tauri.conf.json`、`capabilities/default.json`、`Home.svelte` 和 `commands/fs_utils.rs`. 新脚本/测试使用 `scripts/remediation-01/`、`tests/remediation-01/`. 本包负责公共测试入口, 不接管其他包的测试实现.

不修改 Tools、PdfScroller、Storage 或配置 IPC. `list_dir_files` 第一轮保留 `Vec<String>` 返回和现有参数; 需要详细截断报告的新协议交 07-D. README 的最终安装说明交 07-E, 本包先在本文件记录验证过的安装命令.

## 2. 现有证据与实施顺序

- [x] 读取 `package.json`、npm lock、release workflow、Tauri hooks 和 Cargo 文件. 当前无 Node 版本文件, CI 用 npm/Node 20, 审查机器的 pnpm 11 与该 Node 不兼容. 选择经过官方资料核实并实测兼容的固定 Node/pnpm/Rust 版本, 不直接把审查机器版本当决策. (依据见 §5.1)
- [x] 添加 `.nvmrc` 或 `.node-version`, 通过 nvm 安装和切换. nvm 不可用时先询问, 不静默换 Node 来源. 使用 pnpm 完成一次明确的锁文件迁移, 验证后移除 npm lock; 本地、CI、Tauri hooks 使用同一安装/构建路径. (`.nvmrc`=24.18.0; `pnpm import` 迁移后删除 `package-lock.json`; `tauri.conf.json` hooks 与 CI 全部走 pnpm)
- [x] 修复 `Home.svelte` 的 dialog 返回类型处理, 保持选文件和最近文件行为. Storage 错误导入由 05 修复, pdfjs 手写声明由 03 修复, 不顺手越界处理. (svelte-check 由 1 error/5 warnings 变为 0 error/5 warnings, 剩余 warnings 均位于 03/05 归属文件)
- [x] 建立最小前端行为测试入口, 使各包测试可被发现并执行. 只选择一个实际需要的测试方案, 不为规划引入多套 runner. 已有 Rust 测试继续使用 Cargo. 新依赖需求逐项评估并通过小 PR 交付, 不与业务修改混成超大 PR. (单一 runner: vitest 5.0.2, `pnpm test` = `vitest run`, `vitest.config.ts` 的 include 覆盖 `tests/**/*.test.ts`, 各包把测试放 `tests/remediation-NN/` 即被自动执行)
- [x] release workflow 的 tag push 使用实际触发 tag. 手动发布解析明确目标 ref 并 checkout 该提交, 不再根据 run number 伪造包版本. 校验 package/Cargo/Tauri 三处版本和 tag 一致, updater 元数据对应同一次构建. (tag 事件用 `github.ref_name`; 手动发布输入必填 tag, 先在仓库内解析该 tag 到提交再 `git checkout --detach` 该提交; `scripts/remediation-01/check-versions.mjs` 校验三处版本)
- [x] PR 和 release 都执行类型检查、实际测试、build 和 Rust test. 先检查后发布, 任一必要检查失败时不能创建 Release 或上传发布产物. 用无发布副作用的用例覆盖错误 tag/ref/version. (新增 `ci.yml`: PR/push 跑 frontend+rust 两个 job; release.yml 拆 `validate`→`release` 两段, `release` 声明 `needs: validate`; 错误用例见 §5.4)
- [x] `fs_utils::walk` 明确软链、目录边界、循环和深度规则. 旧接口无法表达部分结果时返回可读错误, 不静默把截断结果当完整成功; 保留大小写不敏感扩展名筛选和排序. 丰富报告交 07-D. (规则见 §5.3, 5 个 Rust 单测)
- [x] 核验当前锁定 Tauri 版本的 CSP/capability 官方资料, 以实际 PDF worker、blob、文件、插件和 updater 路径设计最小可用权限. 不凭 `csp=null` 就宣称已存在可利用漏洞, 不用过宽通配符让测试表面通过. (Tauri 2.10.3; 依据与取舍见 §5.2)

## 3. 验收

| 场景 | 必须证明 | 状态 |
| --- | --- | --- |
| 干净 clone | 按固定版本和单一锁文件可安装; 无隐式依赖升级 | 本机以 `pnpm install --frozen-lockfile` 通过; CI 将在干净 runner 上复验. 注: pnpm 迁移时部分 caret 依赖解析到 lockfile 内的新版本(如 typescript 5.9.3), 属一次性迁移决策, 之后由 pnpm-lock.yaml 单一固定, 见 §5.1 |
| Home 打开 PDF | check 原类型错误消失, 最近文件仍更新 | `pnpm run check` 0 error; 行为保持(multiple:false 返回 string, 原逻辑等价, 仅删除不可达分支) |
| PR 引入失败测试 | CI 真实失败, release job 不继续 | `ci.yml` 每步显式退出; release `validate` 失败则 `release` job 不启动(needs); 本地以 check-versions v0.5.0 失败用例验证错误通道 |
| tag 与手动 ref | 源提交、三个版本字段、Release 名、更新元数据一致 | validate 输出 tag→commit, release checkout 同一 commit, check-versions 保证三处版本==tag; 本地临时仓库演练了 tag 解析正/反用例(§5.4); 未实际推 tag |
| 目录含深层、循环软链、外部软链 | 终止、范围与错误符合声明, 无静默漏扫 | Rust 单测 5 个全绿(深度超限报错、循环/外链不跟随、文件软链按链接名列出、大小写筛选、排序) |
| CSP 后的桌面 GUI | 阅读、工具预览、选择文件、S3、插件和更新所需功能未被误阻断 | 未完成: CSP 按 §5.2 的实际路径推导, 桌面 GUI 全链路核验留待人工或 07-E; 本包不做发布 |

版本约束落地后记录并执行(2026-09-27, macOS arm64, 实测通过):

```sh
nvm install        # .nvmrc → 24.18.0 (本机已装, nvm 可用)
nvm use
pnpm install --frozen-lockfile   # 通过
pnpm run check                   # 0 errors, 5 warnings (全部在 03/05 归属文件)
pnpm run test                    # vitest: 8 passed (tests/remediation-01)
pnpm run build                   # 通过 (chunk >500kB 警告为既有问题)
cargo test --manifest-path src-tauri/Cargo.toml --locked   # 结果见 §5.5
```

前端测试命令在新增脚本实际存在后补入本文件和 CI, 不把尚不存在的 `test` 脚本记为已通过. Rust 离线检查仅在依赖已经缓存时附加 `--offline`. 本包不推 release tag、不创建正式发布.

## 4. 分支与交接

在已确认分发基线的前提下:

```sh
git fetch origin
git switch -c codex/remediation-01 origin/main
```

完成逐文件提交和验证后:

```sh
git push -u origin codex/remediation-01
```

交付 PR 时记录固定工具链、锁文件迁移依据、测试入口和 CSP 限制. 其他包新增依赖由本包集中处理. CSP 与新 worker 的组合、三平台正式打包和 README 最终说明由 07-E 收口. 同步和 push 按总规则处理.

实际执行的偏差记录: 开工时分发基线尚未公布(`docs/remediation/` 未合入 origin/main, origin/main=6e73574, 本地 main=cd01590 仅多一个文档提交). 从本地 main=cd01590 建分支 `codex/remediation-01`. 原 checkout 上有其他包实施者的未提交修改(02/05/06 归属文件), 按总规则改用独立 worktree, 未触碰他们的改动. `docs/remediation/01-engineering.md` 在本分支作为新文件提交(基线中不存在), 分配者合入文档时如有内容差异以其后版本为准合并.

## 5. 实施记录 (证据)

### 5.1 工具链与锁文件

- Node `24.18.0`(`.nvmrc`): nodejs/release 官方计划中 Node 24 为 Active LTS(2026-10-21 起进维护期, 支持至 2028-04-30); Node 22 已在维护期, Node 26 尚非 LTS. 本机 nvm 已有该版本, 实测通过.
- pnpm `11.9.0`(`packageManager` 字段): npm registry 元数据显示其 `engines.node = ">=22.13"`, 证实审查所述 pnpm 11 与 CI Node 20 不兼容; 与 Node 24.18.0 实测兼容. pnpm 当前 latest 为 12.x, 选 11.9.0 因本机实测可用且仍在维护窗口内.
- Rust `1.95.0`(`rust-toolchain.toml`; CI 用 `dtolnay/rust-toolchain` 的 `toolchain` 输入传入同一版本, 避免该 action 设置的 RUSTUP_TOOLCHAIN 覆盖 pin): 与本机实测一致(59807616e 2026-04-14).
- 锁文件迁移: `pnpm import`(读 package-lock.json v3 生成 pnpm-lock.yaml v9)→ `pnpm add -D vitest@5.0.2` → 删除 `package-lock.json`. 迁移时部分 caret 范围解析到比 npm lock 更新的版本(typescript 5.9.3、svelte-check 4.4.6 等), 之后由 pnpm-lock.yaml 单一固定, 不再有隐式升级.
- pnpm 11 不再读取 package.json 的 `pnpm` 字段, 且被忽略的构建脚本会使 install 失败; 构建许可落在 `pnpm-workspace.yaml`(`allowBuilds: esbuild, es5-ext`).
- Tauri hooks(`beforeDevCommand`/`beforeBuildCommand`)与两个 workflow 统一为 `pnpm run ...`, 与本地一致.

### 5.2 CSP 与 capability (R30)

Tauri 2.10.3(Cargo.lock 锁定). 官方文档(v2.tauri.app/security/csp)要求 IPC 走 `connect-src ipc: http://ipc.localhost`(Windows IPC 机制), Tauri 构建时为本地脚本/样式自动注入 nonce/hash. 源码确认(`tauri-2.10.3/src/manager/mod.rs` 的 `csp()`): dev 使用 `devCsp`, 未设置时回落 `csp`, 因此两者都显式配置; devCsp 额外放开 `ws://localhost:1420 http://localhost:1420`(HMR).

前端实际路径审计: pdfjs worker 经 `new URL("pdf.worker.min.mjs", import.meta.url)` 同源加载(`worker-src 'self' blob:`); 前端无 fetch/XHR、无 asset:/convertFileSrc、无远程资源; `style="..."` 标记属性 6 处(PdfScroller/Tools)→ `style-src 'unsafe-inline'`; S3 与 updater 均在 Rust 侧, 不受 CSP 影响. `img-src blob: data:`、`font-src data: blob:` 为 pdfjs 渲染路径预留(非可执行内容), 未使用通配符主机.

capability 最小化(`src-tauri/capabilities/default.json`):
- 移除 `core:window:allow-set-title/-center/-close`: 全文检索仅 `setFullscreen`/`isFullscreen` 被前端调用.
- fs: 原 `fs:allow-read`/`fs:allow-write` 只启用前端从未调用的 `read`/`write` 原始命令(JS `readFile`→`read_file` 等, 依据 plugin-fs dist-js 与 permissions/autogenerated/commands/*.toml). 改为按实际调用启用 `fs:allow-read-file`/`fs:allow-write-file`/`fs:allow-write-text-file` 三条. scope 显式放宽到 `**`: tauri `Scope::is_allowed` 对空 scope 返回 false(默认拒绝), dialog 插件会把本会话选中路径动态加入 scope(tauri-plugin-dialog 2.7.0 `commands.rs` 的 `allow_file`), 但"最近文件"等跨会话路径不经 dialog, 无 `**` 会被拒. 保留 `fs:default` 以维持其对 Windows webview 数据目录的 deny.
- 保留 `dialog:default`(= open/save/message)+ `allow-ask`、`shell:allow-open`(Tools 打开外链)、`updater:default`、`process:allow-restart`.
- 不凭 `csp=null` 宣称存在可利用漏洞(R30 原文同样要求); 本轮也未验证线上更新可用性.

### 5.3 `fs_utils::walk` 规则 (R28)

- 不跟随任何软链(`DirEntry::file_type()` 不解引用): 目录软链不递归, 循环和外链不可能发生; 文件软链按链接名参与扩展名筛选并被列出(列出的是链接路径, 不越界).
- 深度: 根下最多 8 层(与原阈值边界一致); 遇到第 9 层目录返回可读错误(指明深度限制与目录路径), 不再静默截断. 原静默跳过的深层目录现在显式失败, 调用方(Tools 目录扫描)将看到错误而非不完整列表; 详细的部分成功/跳过报告交 07-D.
- 保留: 大小写不敏感扩展名筛选、`result.sort()` 排序、`Vec<String>` 返回与参数不变.

### 5.4 发布门禁与错误用例 (R25)

- `release.yml`: tag push → `RELEASE_TAG=github.ref_name`; 手动 → 必填输入 tag, 在 `git fetch --tags` 后用 `git rev-parse refs/tags/<tag>^{commit}` 解析, 不存在即 `::error::` 退出; 存在则 `git checkout --detach` 该提交, 版本校验/全部检查/构建矩阵都基于同一提交. 不再用 run number 伪造 tag.
- `validate` job: tag 解析 → 版本一致性(`node scripts/remediation-01/check-versions.mjs <tag>`, 校验 package.json / Cargo.toml 的 [package] / tauri.conf.json 三处 == tag)→ pnpm install --frozen-lockfile → check → test → build → cargo test --locked. `release` job 声明 `needs: validate`, 任一步失败不创建 Release、不上传产物.
- 无发布副作用的错误用例(本地执行, 均符合预期):
  - `check-versions.mjs v0.5.0`(版本不一致)→ 退出码 1 并逐条列出不一致字段; `v0.4.0` → 退出码 0.
  - 临时 git 仓库: 存在的 tag v0.4.0 解析出 commit; 不存在的 v9.9.9 被拒(对应 workflow 中断路径). 两个用例都不触发任何 Release 副作用.
  - vitest 单测覆盖 normalizeTag 非法输入(缺 v 前缀/残缺/非字符串)、Cargo [package] 版本解析(跳过依赖表 version 行)、三处版本组合一致性, 以及一个常驻断言: 仓库当前三处版本必须互相同步.
- 未实际推 tag、未触发正式发布(按验收要求). updater 元数据(latest.json)由 tauri-action 在同一 tag/commit/版本下生成.

### 5.5 测试与验证结果

- 前端: `pnpm run test` → vitest 8 passed(tests/remediation-01)+ 03 自带 harness 21/21 + 04 node:test 套件全绿; `pnpm run check` → 0 errors(初轮 5 warnings, 03/05 合并后余 3); `pnpm run build` → 成功.
- Rust: `cargo test --manifest-path src-tauri/Cargo.toml --locked`(依赖已缓存时加 `--offline`)→ 合并基线上 48 passed / 0 failed(含 fs_utils 新增 5 个单测: 排序/大小写、空与不存在根、深度超限报错、目录软链循环与外链不跟随、文件软链按链接名).
- 启动冒烟: `pnpm tauri dev` 以新 CSP/capability 正常完成编译并拉起 `target/debug/pdf_seeker`, 运行 70 秒无 panic/报错后手动终止. webview 内逐功能点检仍留待人工/07-E.
- CI: `ci.yml`(pull_request + push main)与 `release.yml`(tag/dispatch)YAML 已用本地解析验证; PR #8 的 Actions 首跑暴露了 §5.7 记录的入口冲突, 修复分支已验证并重跑.

### 5.7 测试入口与既有包 runner 的合并 (PR #8 后跟进)

PR #8 合入后, 03/04 的 PR 也在 main(它们的实施在 01 之前开始, 均自带零依赖 runner: 03 用 node 类型剥离自写 harness(`node tests/remediation-03/run.ts`), 04 用 node:test + esbuild 打包(`node tests/remediation-04/run.mjs`), 并在文件头注明工具链统一归属 01). 合并结果暴露两个问题, 已在 `codex/remediation-01-fix` 修复:

1. `vitest.config.ts` 的 `tests/**/*.test.ts` 把非 vitest 的 `.test.ts` 一并收走, vitest 收集 0 个用例判失败. 修复: exclude `tests/remediation-03/**`、`tests/remediation-04/**`; 这两个包的文件按其自带入口执行, 不改写它们的测试实现.
2. pnpm 严格 node_modules 布局下, 04 的 `run.mjs` 直接 `import "esbuild"` 不再可解析(npm 扁平布局曾使其恰好可见). 修复: 按依赖卫生显式声明 `esbuild@0.25.12` 为 devDependency(版本与 lockfile 中 vite 所用一致, 不新增拷贝).

统一入口现状: `pnpm test` = `vitest run && node tests/remediation-03/run.ts && node tests/remediation-04/run.mjs`, ci.yml 与 release validate 均经此单一命令覆盖全部前端测试. 04 注明"01 落地后这些文件应迁入统一入口", 该迁移属其测试实现, 由 04 自行决定时机, 本包不代改.

### 5.6 未完成与移交

- 桌面 GUI 在新 CSP/capability 下的全链路人工核验(阅读、工具预览、选文件、S3、插件、更新)未执行 → 07-E(CSP 与新 worker 组合、三平台打包由 07-E 收口).
- README 最终安装说明(pnpm/nvm/rust 固定版本) → 07-E; 本文件 §3 已记录验证过的命令.
- check 剩余 5 个 a11y warnings 在 03/05 归属文件 → 对应包.
- `dist/` 未被 .gitignore(构建产物入库)属既有状态, 本轮未处理.
