# 04 文档对比 — 实施证据

分支: `codex/remediation-04` (基于分发基线 `origin/main` = `6e73574f53f7d7d621c4c71806d76eb85be6f3e5`)
对应整改项: R29 (对比请求生命周期, diff 预算, 面板焦点)
实施文档: [04-comparison.md](../04-comparison.md)。该文档在开工时尚未合入 `origin/main`(分发者的文档合并仍在进行), 勾选记录以本文件为准, 不在分支内复制实施文档以免与文档 PR 冲突。

## 1. 变更文件

| 文件 | 变更 |
| --- | --- |
| `src/lib/comparison/diff-core.ts` | 新增。线性空间 Myers O(ND)(Myers 1986 §4b, diff-match-patch 形式)替换主线程全矩阵 LCS; 显式步数预算, 超限抛 `DiffBudgetError`。内存 O(n+m)/层, 不再有 O(n·m) 分配。 |
| `src/lib/comparison/page-diff.ts` | 新增。按页号配对; `noText`/`skipped` 降级状态; 页/运行两级预算(分配前校验规模); 协作式取消钩子; worker 消息协议。 |
| `src/lib/comparison/diff-worker.ts` | 新增。worker 内以 `WORKER_LIMITS` 执行 `diffPagesCore`; 页间经 `setTimeout` 让出以排空消息队列, 新 `run`/`cancel` 在下一页边界生效。 |
| `src/lib/comparison/diff-service.ts` | 新增。单一 worker、单一在途 run; 新 run 取消旧 run(旧 promise 解析 null); worker 构造失败或中途出错时回退主线程(更严预算, `engineFailed` 标记); `dispose()` 终止 worker。 |
| `src/lib/comparison/analyze-controller.ts` | 新增。文件对 + 请求 token 固定一次分析身份; 仅最新请求可写 texts/错误/analyzing; `dispose()` 作废在途请求并释放 diff 引擎。 |
| `src/lib/utils/diff.ts` | 改为兼容层: 保留 `diffLines`/`splitLines`/`opsHaveChanges`/`DiffOp` 导出, 算法委托 diff-core。调用点检索: `diffLines`/`splitLines`/`opsHaveChanges` 仅 `src/views/Compare.svelte` 使用, 已同包更新。 |
| `src/views/Compare.svelte` | 接入 controller; 降级/无文字层显示; 唯一活动面板; 界限提示。 |
| `tests/remediation-04/` | `run.mjs`(测试入口), `diff-behavior.test.ts`, `page-diff.test.ts`, `controller.test.ts`, `bench.mjs`(性能记录)。 |

未修改: PdfScroller、stores、PDF 后端、公共字典、依赖清单。PdfScroller 继续以原语义使用 `tab`/`active`/`initialPage`/`getScrollContainer()`; `extract_page_texts(path) -> string[]` 消费不变; 未引用 03 未合并的会话模块。

## 2. 验收场景映射

| 场景 | 机制 | 证明 |
| --- | --- | --- |
| 请求 A1/B1 晚于 A2/B2 返回 | controller token: 每次 `run` 自增; extract/diff 两阶段均校验后才写状态 | `controller.test.ts` "A1/B1 returning later than A2/B2 cannot overwrite the newer pair"(旧对 extract 晚 resolve 不触发 diff, 全部状态属于新对) |
| 请求中交换文件/离开视图 | 新 run 取消旧 run(service 层 + worker cancel 页边界生效); `dispose()` 终止 worker、作废 token | `controller.test.ts` "superseded/canceled/dispose"; `page-diff.test.ts` "cancellation between pages aborts with canceled outcome"。滚动监听经 `$effect` 清理随组件销毁释放 |
| 双方扫描 PDF | 双侧空白文本 → `noText` 状态, 不产生 "same"; 全部 `noText` 时汇总栏显示文字层不足横幅(明示不代表视觉相同); "identical" 文案仅在全页 `same` 时出现 | `page-diff.test.ts` "both sides blank text becomes noText, never same"; 单侧空白仍正常 diff 为 changed |
| 超长文本、极不平衡文本 | 分配前预算: 页字符上限(char 校验先行, 不 split 不分配)→ 行数上限 → 步数预算 → 运行总步数/时限; 超限页标记 `skipped` 并计数, 不伪装相同。窗口不冻结: diff 全部在 worker; 主线程回退预算为 worker 的 1/16 | `page-diff.test.ts` size/steps/total/time 四个预算测试; `diff-behavior.test.ts` "step budget throws…"/"extremely unbalanced inputs stay bounded and valid"; 见 §3 实测 |
| 左右面板交替焦点 | `focusSide` 唯一活动面板(`active={focusSide === 'a'/'b'}`), pointerenter/focusin 维护; PdfScroller 窗口级 keydown 仅在 `active` 时挂载 → 快捷键只触发一次; inactive 面板照常渲染页面 | 代码路径: PdfScroller `$effect` 以 `active` 门控 keydown/stores 写入/全屏轮询(未改动该文件)。GUI 级组合验收见 §5 限制 |
| 比例滚动/跳页 | `syncFrom` 保留 `syncing` 门闩 + rAF 复位(程序化滚动触发的回流事件被丢弃, 无反馈环); 回调按 `scrollerA/B` 当前实例解析; 视图横幅明示"按比例同步, 非内容对齐" | 既有机制保留; 文案不再把比例同步描述成内容对齐 |

## 3. 算法预算与实测记录

预算(worker / 主线程回退):

| 项 | worker | 主线程回退 |
| --- | --- | --- |
| 页字符上限(分配前) | 4,000,000 | 400,000 |
| 页行数上限(两侧行和) | 200,000 | 20,000 |
| 页算法步数 | 8,000,000 | 500,000 |
| 运行总步数 | 64,000,000 | 2,000,000 |
| 运行时限 | 15,000 ms | 4,000 ms |

内存形态: 每次 bisection 层分配两个 `Int32Array(2·ceil((n+m)/2))`(瞬态), 即 O(n+m); 递归共用外部输入, 不存在 n·m 矩阵。

实测(`node tests/remediation-04/bench.mjs`, 本机 Apple Silicon, Node 24, 单次运行):

```
scenario                                  | linesA | linesB | steps    | ms
identical 20k lines                       | 20000  | 20000  | 0        | 2.0
20k lines, one changed line               | 20000  | 20000  | 2        | 2.3
5k vs 5k fully different lines            | 5000   | 5000   | BUDGET   | 69.1  ← 8M 步预算触发, 该页降级
60k vs 3 lines (extremely unbalanced)     | 60000  | 3      | 300000   | 6.0
interleaved random 2k lines (3-symbol)    | 2000   | 2000   | 960359   | 22.8
```

要点: 常见情形(相同页、局部小改)接近零开销; 最坏情形(全不同)在一个页预算内(69ms, worker 线程)触发降级, 主线程永不执行该量级; 极不平衡输入有界。

## 4. 边界声明(文字层与页对齐)

- 文字比较依赖 `extract_page_texts` 的文字层; 双侧均无文字(空白)只表示"未比较", 不是"视觉相同"。
- 按相同页号配对; 页数不同或存在插页时, 后续页按页号错位对比。UI 在页数不同处显示固定提示; 并排视图的滚动同步是比例同步, 不做内容对齐。
- 页自动对齐与像素 diff 不在本包范围(按原始计划另行设计)。

## 5. 验证命令与结果

- `npm run check`: 1 error + 5 warnings, 与基线(origin/main 未修改状态)完全一致; 存量问题位于 Home/Tools/Storage/Tooltip(01/03/05 归属), 本包文件零新增。
- `npx tsc --noEmit --strict --skipLibCheck …`(对 `src/lib/comparison/*.ts` 与 `utils/diff.ts` 单独全严格检查): 通过。
- `npm run build`: 成功; `dist/assets/diff-worker-*.js`(3.69 kB, 自包含 IIFE, 无裸导入)作为独立 chunk 产出, 主 chunk 正确引用其 URL。CSP 现状为 `null`(R30 记录), 未为 worker 单方面放宽任何安全策略; 与 pdfjs worker(既有先例)同样走同源资源加载。
- `node tests/remediation-04/run.mjs`: 26/26 通过(controller 5, diff 行为 10, 页配对 11)。
- `node tests/remediation-04/bench.mjs`: 见 §3。

限制: 本环境无法运行 Tauri GUI 完成人工组合验收(焦点切换、滚动同步的实机操作)。已做代码路径级验证: PdfScroller 的 keydown/stores/全屏轮询均由 `active` 门控, 唯一 active 传入后这些全局副作用只存在一份; 未发现 scroller 违反原 active 语义的问题(无需向 03 移交缺口)。03 合入后的 scroller 组合验收与三平台 worker/CSP 复核由 07-E 执行。

## 6. 前置依赖与移交

1. **01 测试入口未落地**(分发基线无前端测试配置, `package.json` 归 01): `tests/remediation-04/run.mjs` 以 node + esbuild(既有依赖, 经 vite 间接安装)自建入口, 无新增依赖、无锁文件改动。01 交付统一入口后应将本目录迁入, 此为记录在案的前置需求。
2. **01 CSP**: 未来设置 CSP 时必须放行本 worker 的同源模块资源(现 CSP 为 null, 无需动作)。
3. **07-E i18n 收口**: 新增 UI 文案暂以组件内双语字面量(`lt()`)呈现, 未调用不存在的字典键、未修改 05 独占的字典文件。待收口清单:
   - 无文字层页状态: "No text layer" / "无文字层"
   - 未比较页状态: "Not compared" / "未比较"
   - 规模超限页: "Page text exceeds the size budget — not compared." / "本页文本超出规模预算,未比较。"
   - 预算/时限页: "Comparison budget or time limit reached — not compared." / "达到比较预算或时间上限,未比较。"
   - 无文字层页正文: "No extractable text on either side — visual equality was not assessed." / "两侧均无可提取文字,未评估视觉是否相同。"
   - 全无文字层横幅: "Neither file has an extractable text layer (scanned PDFs?). Text comparison is unavailable — this does not mean the files look the same." / "两个文件都没有可提取的文字层(扫描件?)。无法进行文字比较——这不代表两者视觉相同。"
   - 降级汇总: "{n} page(s) not compared (over size/budget/time limit)." / "已跳过 {n} 页(超出规模/预算/时间上限)。"
   - worker 回退: "Diff worker unavailable; compared on the main thread with stricter limits." / "比较 worker 不可用,已在主线程按更严格预算完成比较。"
   - 页数不同: "Page counts differ; pages are paired by page number, so inserted or removed pages shift later pages." / "两文件页数不同;按相同页号配对,插入或删除页会使后续页错位。"
   - 并排横幅: "Scroll sync is proportional, not content-aligned; shortcuts act on the highlighted panel." / "滚动按比例同步,不代表内容对齐;快捷键作用于高亮面板。"
4. **07-E 组合复核**: 03 合入后的 scroller 上的组合验收; Linux/webkitgtk 模块 worker 兼容(运行时有主线程回退兜底)与三平台核验。
5. **对 03 的移交**: 无。未发现 scroller 无法遵守 active 语义的缺口, R29 的快捷键子项在本侧条件已满足, 最终关闭按总表由 07-E 组合验证。
