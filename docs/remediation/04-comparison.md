# 04 文档对比

状态: 已实施, PR #5 已合并; 证据见 [evidence/04-comparison.md](evidence/04-comparison.md). 先读 [分工与合并规则](README.md). 对应 R29.

## 1. 写入范围与独立合并条件

只修改 `src/views/Compare.svelte`、`src/lib/utils/diff.ts`, 可新增 `src/lib/comparison/` 和 `tests/remediation-04/`. 不修改 PdfScroller、stores、PDF 后端、公共字典或依赖清单.

保持现有 `extract_page_texts(path)` 返回 `string[]` 的消费, 复用 PdfScroller 现有 `tab`、`active`、`initialPage` 和 `getScrollContainer()`. 不能依赖 03 新增但尚未合并的文档会话模块. 新 worker 必须验证与当前 CSP/打包兼容, 不为一个 worker 单方面放宽安全策略.

## 2. 实施清单

- [ ] 阅读 `analyze`、`swap`、`pagePairs`、`syncFrom` 和 `diffLines` 的调用关系. 当前换文件可并发分析, 旧结果/旧 finally 都能覆盖新状态, LCS 使用主线程全矩阵.
- [ ] 以文件对和 requestId 固定一次分析的身份, 只有最新请求能更新文本、错误和 analyzing. 换 A/B、交换、组件销毁后丢弃旧结果, 清理相关监听和任务.
- [ ] 分配 LCS 矩阵前按输入规模校验预算. 在实际需求下选择更省内存算法或 worker; worker 也必须限制内存和任务长度, 新请求能够取消旧任务. 超预算给出明确降级/未比较状态, 不伪装为相同.
- [ ] 明确文字层比较和同页号对齐的现有边界. 双方无文字层不显示成“视觉相同”; 插页导致页序错位时展示限制. 不在本包增加自动页对齐或像素 diff.
- [ ] 通过鼠标和键盘焦点维护唯一活动面板, 不再传两个 `active={true}`. inactive 仍显示页面; 窗口级快捷键只触发一次. 比例滚动同步不制造循环, 回调使用当前面板实例.
- [ ] 已有 diff API 若变更, 检索所有调用点并同包更新; 公共 scroller API 不变. 最终完整翻译交 07-E, 不引用不存在的字典键.

## 3. 验收

| 场景 | 必须证明 |
| --- | --- |
| 请求 A1/B1 晚于 A2/B2 返回 | 标题、结果、错误及 loading 全属于最新文件对 |
| 请求中交换文件/离开视图 | 旧结果不回写, worker/listener 释放 |
| 双方扫描 PDF | 提示文字层不足, 不声称视觉相同 |
| 超长文本、极不平衡文本 | 分配前触发预算, 窗口保持可响应, 取消有效 |
| 左右面板交替焦点 | 翻页/全屏只按定义触发一次, 另一面板仍正常显示 |
| 比例滚动/跳页 | 无反馈循环, 不把比例同步描述成内容对齐 |

执行前端 check、build、diff 行为测试及受控请求顺序测试. 使用 01 已落地的环境和测试入口, 入口未落地则先记录该前置需求. 保留短文本原有差异行为, 为长文本记录输入规模、用时和资源上限, 不凭空承诺性能数值. 在旧 scroller 和 03 合入后的 scroller 上各完成一次组合验收, 后者由 07-E 复核.

## 4. 分支与交接

```sh
git fetch origin
git switch -c codex/remediation-04 origin/main
```

逐文件提交、验证后:

```sh
git push -u origin codex/remediation-04
```

向 07-E 交付算法预算、降级行为、文字层限制和焦点验证记录. 若发现 scroller 无法遵守原有 active 语义, 记录具体复现交 03 修复, 不越界修改同一文件; 在该缺口解决前不关闭 R29 的快捷键子项.
