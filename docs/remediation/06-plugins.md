# 06 插件运行可靠性

状态: 已实施, PR #7 已合并. 先读 [分工与合并规则](README.md). 对应 R23.

## 1. 写入范围与独立合并条件

只修改 `src-tauri/src/commands/plugins.rs`、`docs/plugins.md`、`docs/plugin-example/`. 内部子模块放 `commands/plugins/`, 测试与合成插件放 `tests/remediation-06/` 或现有 Rust 测试模块. 不改 Tools、命令注册或公共依赖文件.

保留 `list_plugins` 和 `run_plugin`、`RunPluginRequest`、`PluginRunResult` 的既有消费契约. 需要额外取消/进度协议时交 07-D, 不能让旧 Tools 无法显示结果. 新平台进程管理依赖走 01 前置 PR.

## 2. 实施清单

- [ ] 阅读 manifest 发现、read_manifest、truncate_output、进程启动/管道读取/超时和产物返回完整调用链. 保留无 shell 的参数直传方式.
- [ ] 截断基于有效 UTF-8 字符边界, 对可能被管道分块切开的多字节字符和非 UTF-8 输出明确采用一致的转换策略. 不再按字符串字节索引直接切割.
- [ ] stdout/stderr 持续排空但只保留限定字节, 明确预算是分别限制还是合计限制. 不 `read_to_end` 后才截断; 输出耗尽预算后仍避免子进程因管道满而死锁.
- [ ] 超时覆盖整个进程树和收尾过程. macOS/Linux 进程组、Windows 作业等机制按实际平台资料核验; 子进程/孙进程持有管道时也必须在收尾上限内结束读取与 join.
- [ ] 插件目录身份作为稳定 ID, 校验 manifest ID 与目录约定. 拒绝目录穿越、绝对路径/越界解析和不合法模式; 验证符号链接边界, 不只做字符串包含判断.
- [ ] file 输出必须存在且符合声明后才能返回完成, 失败/超时不能把旧文件当本次新产物. 记录实际输出路径与错误, 不泄漏输入正文或敏感参数.
- [ ] 同步插件文档和示例, 明确可信本地插件可执行外部程序, 不描述成安全沙箱. 只更新与本次行为变化有关的文档内容.

## 3. 验收

| 合成插件 | 必须证明 |
| --- | --- |
| 连续输出 70000 个中文字符 | UTF-8 截断不 panic, 显示截断状态 |
| 大量 stdout 和 stderr | 保留字节不超预算, 进程不因管道阻塞 |
| 孙进程持管道/父进程先退出 | 超时连同收尾有效, 无悬挂进程或线程 |
| manifest ID 不一致/穿越/目录外软链 | 启动前明确拒绝或按文档稳定 ID 处理, 不执行错误目录程序 |
| 输出文件不存在/仅有旧文件/退出失败 | 不报告本次成功产物 |
| 正常示例插件 | 旧 Tools 可发现、执行并展示结果 |

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked
```

测试通过实际注册的 Rust 测试入口运行. 进程组/作业和管道收尾需要各平台验证, 不能只因 macOS 通过就标 Windows 完成. 回归样本自身有资源上限, 不用无限制输出把开发机器耗尽.

## 4. 分支与交接

```sh
git fetch origin
git switch -c codex/remediation-06 origin/main
```

逐文件提交、验证后:

```sh
git push -u origin codex/remediation-06
```

交付各平台收尾策略、输出预算、ID 迁移/兼容规则和真实测试结果. 进度/取消新接口如有需求交 07-D, 07-E 复核三平台插件冒烟与文档支持矩阵.
