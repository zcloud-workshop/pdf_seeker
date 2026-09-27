# 05 设置、S3 与更新

状态: 部分实现. 2026-09-27 更新. Rust 与前端门禁通过, 真实 S3 服务验收和系统凭证迁移待完成. 先读 [分工与合并规则](README.md). 对应 R01、R16-R22、R26 的 Storage 导入、R27 的 Toolbar/Storage、R28 的下载、R30 的认证/updater.

## 1. 写入范围与独立合并条件

独占 Settings、Storage、Input、配置类型、i18n 目录、Toolbar、main.ts、updater.ts, Rust 的 config 模块、config/recent/s3_ops 命令及 `src-tauri/src/lib.rs`. 新模块用 `src/lib/settings/`、`src/lib/storage/`, 测试用 `tests/remediation-05/`.

S3 的前后端同一负责人、同 PR 修改, 不再拆成先改 DTO 和后改消费者两条依赖分支. `lib.rs` 只用于配置启动/恢复接入, 保留其他包命令注册. 不改 stores 的导出、Tauri 配置/CSP、Cargo/package 依赖和 Tools.

保留旧消费者读取的 `general` 配置与最近文件接口. 主题通过现有 `isDark` store 接入, 不要求 03 改它的定义. `checkForUpdates()` 保留现有消费形式; 可在本包内扩展 updater 状态展示, 编辑 busy/重启协调等到 07-A.

## 2. 实施清单

- [ ] R01: 原生 input 绑定组件 value 并保留事件透传. 验证 text/password/number、父级更新、用户输入、保存和回显, 不逐表单绕开公共组件修复.
- [ ] R16: Rust 序列化与 Storage 类型选择唯一字段命名, 同 PR 更新全部调用点. 删除错误 `@/lib/types` 导入和 ts-ignore. 用真实 Rust 序列化样本验证前端消费, 不能仅依赖 `invoke<T>`.
- [ ] R17: 明确完整 objectKey 与相对 folder, 仅一个边界拼接 root_prefix. 空 root 用空前缀, 列表 key 不重复拼接. 上传、下载、删除、版本、文件夹、签名 URL 和配置备份逐条检查, 不把 key 命名范围说成权限边界.
- [ ] R18: 版本按精确 key 过滤, 完整支持分页. 将 max_versions 的展示限制与保留策略区分, 移除/重命名未实现 TTL/保留承诺. 保留旧配置兼容, 不在查询时偷偷清理远端版本.
- [ ] R19: 配置就绪立即加载目录, 区分未加载/加载/空/失败; 修正面包屑索引. 请求绑定目录身份, 过期响应不能覆盖当前目录. 删除确认展示准确文件和版本, 取消不发删除请求.
- [ ] R20: 默认备份不含密钥/token, 最近路径默认不带或明确选择. 本机凭证迁到系统凭证存储, 普通配置保存引用; 先制定兼容迁移与回退, 凭证写入/读取失败不删除唯一旧凭证. 恢复普通备份不覆盖本机密钥. 新依赖先由 01 前置交付; 加密跨设备密钥导出仍是另行需求.
- [ ] R21: 配置、最近文件、恢复走同一个受锁保护的事务并原子落盘, 内存与磁盘同时成功才提交. 使用自包含、带格式版本和校验的备份对象, 上传成功才更新成功时间. 恢复先校验再替换并保留回退; 损坏本地 TOML 有恢复入口. 不把 checksum 当来源认证.
- [ ] R21: 恢复后根据真实结果完整重建 UI, `s3=null` 应禁用 S3, 不强行开启. 旧备份读兼容与本机凭证保留有独立测试.
- [ ] R22: 建立单一配置应用入口供启动、保存和恢复调用, 同步 locale、html lang、主题 class 及 system 监听. Toolbar 切换明确为持久偏好或临时覆盖, 说明选择并测试重启行为; 监听不重复注册且可释放.
- [ ] R27: 暂不可用的保存/导出入口隐藏或解释原因, 不在缺少文档保存模型时做假按钮. Storage 弹窗补标签、键盘与焦点恢复. 维护本包新增中英字典键; 全应用剩余硬编码交 07-E.
- [ ] R28: S3 下载流式写临时文件, 成功后替换, 中断不留下伪装完整产物. 错误展示实际目标, 不把整个对象先收进内存. 统一进度/取消/重试模型交 07-D, 本包可先完成自有下载链路.
- [ ] R30 认证: 按锁定 AWS SDK 官方资料和真实服务验证 none/static/env、默认凭证链等待、自定义 endpoint/path-style. 认证模式分别构建客户端, 不凭名称假设 ANONYMOUS 就是未签名请求.
- [ ] R30 更新: 启动/手动检查复用一个在途任务, 提供可读失败原因、下载进度及资源清理. 离线、错误签名、拒绝安装、重复点击均有确定结果. 保持启动失败不阻塞应用; 与编辑互斥的重启决策交 07-A.

本包可拆成 Input、S3 浏览、配置事务/凭证、主题语言、updater 多个完整 PR, 但同一文件不要交给多人并行改. SDK/keychain API 不熟悉时先核实官方资料, 不猜测实现.

## 本轮实现记录

- 已修复 Input 的原生双向 value 绑定, 保留其余 input 属性和事件透传.
- Storage DTO 使用 Rust 实际序列化的 snake_case 字段. 前端从 unknown 校验响应结构, Rust 契约样本覆盖中文 key 和目录项.
- Storage 目录使用相对 folder, 列表返回完整 objectKey, 下载/删除/版本/签名 URL 使用该完整 key. Root prefix 只用于目录、上传和配置备份拼接. 版本列表精确匹配 key 并遍历分页, max_versions 只限制展示, 旧 version_ttl_days 继续读写但不再显示为保留承诺.
- 配置、最近文件、备份时间和恢复使用受锁事务与同目录原子替换. 新备份是 TOML 对象, 含 [backup] 格式版本和 checksum 元数据; 旧版配置备份仍可读取, 新对象仍能被旧 AppConfig 忽略未知元数据后读取. 普通备份排除访问密钥、会话令牌、最近文件路径和本机恢复路径. 恢复保留本机最近路径, 并仅在 endpoint、region、bucket 相同时复用本机凭证.
- 无效本地 TOML 会改名保留, 设置页显示恢复副本路径. 保存或云恢复根据返回的真实配置重建表单, s3 = null 保持停用.
- 设置应用入口同步 locale、html lang、主题 class/store. Toolbar 主题选择作为持久偏好保存. System 监听注册一次并在 App 卸载时释放.
- Toolbar 隐藏尚无文档写入模型的保存/导出按钮. Storage 版本弹窗提供标签、Esc/Tab 焦点管理和关闭后的焦点恢复. S3 下载逐块写临时文件, 完成后替换目标, 失败时清理临时文件并显示远端 key 与本机目标.
- 更新检查共享一个在途任务, 展示错误原因与下载字节进度, 并释放 Update 资源. none 使用 AWS SDK 的 no-auth fallback; static 使用显式静态凭证构建客户端; env 使用默认凭证链.

验证结果: cargo test --manifest-path src-tauri/Cargo.toml 通过, 72 tests passed. pnpm run check 为 0 errors/0 warnings. pnpm test 全通过, 包含 Vitest 11 tests、remediation-03 21 tests、remediation-04 26 tests. pnpm run build 成功, 仍有超过 500 kB chunk 的告警. rustfmt --check 和 git diff --check 通过. Rust 序列化 DTO fixture、前端 DTO 消费、Input 绑定结构、备份校验、旧备份读取、本机凭证保留和配置原子替换都有测试.

仍待验证或交接: 没有隔离的 MinIO/S3 测试服务, 所以 none/static/env 请求行为、跨页版本和中断下载未做真实服务验收. 更新签名和安装拒绝路径也未连接发行服务实测. R20 系统凭证存储迁移依赖 01 交付的凭证库, 本轮未把密钥从本地配置迁走; 旧本机凭证不会被删除, 云备份不会包含它们. 主题重启、系统主题切换和 Storage 键盘操作还需桌面端验证.

## 3. 验收

| 场景 | 必须证明 |
| --- | --- |
| 修改并重进设置 | Input 正确绑定, 密码不进入日志 |
| 空 root、pdf、pdf/、中文/嵌套 key | 上传到删除始终指向同一对象, 不误删重复前缀同名对象 |
| a.pdf 与 a.pdf.old, 多页版本 | 精确过滤、分页完整, 删除目标准确 |
| 快速目录切换与面包屑 a/b/c | 不闪回旧目录, 每层到达正确位置 |
| 备份原始字节 | 不含测试 access key/secret/token, 路径按用户选择 |
| 断网、坏备份、坏 TOML、并发恢复/保存/最近文件 | 时间真实, 原配置可恢复, 内存磁盘一致 |
| 凭证迁移失败、旧备份恢复 | 原连接能力可恢复, 本机密钥不被普通备份覆盖 |
| 英文/深色/system/云恢复 | 立即生效、重启保持、系统变化正确 |
| none/static/env 和 endpoint | 与测试服务观察的请求行为一致 |
| 下载中断与更新失败 | 临时产物清理, 签名失败不安装, 状态可解释 |

执行前端 check/build/组件和 DTO 契约测试、Rust test, 再用专用测试桶或 MinIO 做真实链路. 测试删除仅用明确隔离的临时对象, 不对真实资料做清理. 无服务或签名环境时标为待验证, 不用 mock 通过代替实测完成.

## 4. 分支与交接

```sh
git fetch origin
git switch -c codex/remediation-05 origin/main
```

逐文件提交、验证后:

```sh
git push -u origin codex/remediation-05
```

交付配置/备份格式和回退说明、DTO 样本、测试桶契约、凭证存储依赖与平台限制. 把 updater 的在途状态入口交给 07-A, 把全应用字典剩余工作与支持矩阵交给 07-E.
