# 02 PDF 后端实施证据

## 变更

- `pdf_ops.rs`: `Contents` 递归解引用并扁平追加; PDF 保存统一写入同目录临时文件, `sync_all`, 重新解析后平台替换; 所有后端读改写命令使用进程内互斥. 页树重排要求页号完整唯一, 合并取 lopdf 重编号后的实际页序, 插页/重排固化继承属性并重建一致的 `Parent`, `Kids`, `Count`.
- `pdf_ops.rs`: PNG 像素流按实际压缩结果声明 FlateDecode, alpha 使用灰度 SMask; JPEG 保留原 DCT 流并按灰度/RGB 类型声明色彩空间. 页面资源复制到目标页并保留既有类别/名称, 新资源分配未占用名称.
- `pdf_ops.rs`: AcroForm 支持内联字典和直嵌套字段提升, 继承字段类型, 按 Widget `/AP /N` 的真实状态更新 checkbox/radio `/V` 与 `/AS`. 书签递归计数修正; 对不能由旧输入结构无损表达的既有目录拒绝写入.
- `pdf_ops.rs`: OCR 每次取得唯一后端拥有目录, 只接受该任务目录中的图像, OCR 成功或失败后清理该目录; 页失败返回错误. 拆分页范围严格验证, 输出冲突预检, 中途失败移除本次已生成结果.
- `security.rs`: 加解密沿用 RC4-128 V2/R3, 递归处理字典/数组/流字典字符串和流内容, 文件 ID 使用操作系统随机源; 规范化只在内存进行且明文缓冲退出时清零. 不支持的加密版本与错误密码分别报错.
- 可见页面文字目前明确限制 ASCII. 注释/字段/书签 PDF 字符串使用 UTF-16BE BOM 编码; 没有加入未经许可核验的 CJK 字体.

## 回归与命令

合成回归包含混合 `Contents` 连续编辑、RGBA PNG 与灰度 JPEG、逆序 ID/非零 generation 合并、三层页树插页与继承 MediaBox、共享资源复制、并发同文件编辑、写入替换失败、内联表单/自定义 Widget 外观状态、加密嵌套字符串与流字典、以及 R08 浮点/非零原点继承几何样本. 测试样本由测试即时生成, 不含用户 PDF.

```sh
cargo test --manifest-path src-tauri/Cargo.toml --locked
rustfmt --edition 2021 --check src-tauri/src/commands/pdf_ops.rs src-tauri/src/commands/security.rs
git diff --check
```

结果: Rust 全量测试 65 passed, 0 failed; 测试包含 Poppler 对新生成 PDF 与 RC4 加密 PDF 的渲染, 并检查输出像素. rustfmt 与 diff whitespace 检查通过. Poppler 报告系统 Fontconfig 配置缺失, 但渲染和像素断言通过. 构建保留一个既有 `AppError::Pdf` 未使用 warning.

## 语义与残余验收

- R08 只记录现状, 未改变坐标约定: 水印尺寸读取页对象直接 `MediaBox` 的整数右上坐标并忽略原点/继承值; 缺失或实数值落回 612x792. 合成样本确认继承浮点非零原点 MediaBox 时旧水印中心仍为 `(306, 396)`. 裁剪请求使用 PDF points、左下原点, 目前只解析页对象直接 MediaBox; 旋转/CropBox 和视口逆变换统一交 07-B.
- `pdftoppm` 与 `pdfinfo` 可用. Rust 回归实际以 Poppler 渲染普通与加密样本, 检查颜色像素; 应用内 pdfjs 及真实用户阅读器尚未验证, Widget/目录跨实现视觉验收仍待 07-E.
- 加密回归由 lopdf 密码验证、嵌套对象往返及 Poppler 打开本实现输出构成; 未取得独立实现生成的加密样本, RC4 外部输入兼容验收仍待 07-E. Web Search MCP 当前不可用, 本轮依据锁定的 lopdf 0.34 源码检查其解密支持范围, 未对 AES 自行实现.
- Windows `MoveFileExW` 替换路径未运行验证. 临时写入失败和替换失败都有原目标保护/临时文件清理回归.
- R06 只完成后端保存边界; 前端 `writeFile` 撤销/重做仍由 07-A 接入, 外部进程修改检测及版本 token 也尚未加入. 进程内互斥不等同于跨进程文件锁.
- R10 仅更新旧请求可表达的字段和现有 Widget 状态; 可编辑能力响应、清空语义及外观重建由 07-C 接通. R11 只接受旧结构可无损表示的本地目录.
- R13 任务目录/所有权和失败清理已处理; 有序页清单、逐页结果、取消及渲染失败全路径清理由 07-D 联调. R28 旧返回值无法报告部分失败, 本轮失败时回滚已生成的拆分页并返回错误; 结构化结果交 07-D.
