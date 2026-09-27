# PDF Seeker 插件系统（v1：外部命令插件）

插件系统允许把任意外部命令行工具接入 PDF Seeker 工具箱。每个插件是一个包含 `plugin.json` 清单的目录，放在应用配置目录的 `plugins/` 下：

| 平台 | 插件目录 |
|------|---------|
| macOS | `~/Library/Application Support/com.pdfseeker.app/plugins/` |
| Windows | `%APPDATA%\com.pdfseeker.app\plugins\` |
| Linux | `~/.config/com.pdfseeker.app/plugins/` |

放入后重启应用（或在工具箱重新进入「插件」页）即可被发现，出现在 工具箱 → 插件 分区。

## plugin.json 清单格式

```json
{
  "id": "my-tool",
  "name": "My Tool",
  "version": "1.0.0",
  "description": "一句话描述",
  "command": "./run.sh",
  "args": ["--input", "{input}", "--out", "{output}"],
  "output": { "mode": "text", "extension": "txt" },
  "timeoutSecs": 120
}
```

| 字段 | 说明 |
|------|------|
| `id` | 可省略，默认取目录名。目录名是插件的稳定 ID；若显式填写，必须与目录名一致 |
| `name` | 必填，工具箱中显示的名称 |
| `command` | 必填。相对路径（`./run.sh`、`bin/tool`）相对插件目录解析；绝对路径原样使用；裸命令名（`qpdf`、`python3`）走 PATH 查找 |
| `args` | 参数模板，支持占位符 |
| `output.mode` | `text`（默认）：插件向 stdout 输出，结果显示在面板；`file`：插件写文件到 `{output}`，运行前会弹出保存对话框。文件模式必须在参数中使用 `{output}` |
| `output.extension` | `file` 模式下保存对话框的扩展名；插件选择的输出路径扩展名必须匹配 |
| `timeoutSecs` | 运行超时，默认 120 秒。到期后会终止插件并限时完成管道收尾 |

## 占位符

| 占位符 | 含义 |
|--------|------|
| `{input}` | 输入 PDF 路径（当前打开的 PDF，或在面板中另行选择） |
| `{output}` | 输出文件路径（仅 `file` 模式，运行前由保存对话框确定） |
| `{dir}` | 插件目录绝对路径 |

参数直接传给子进程（不经过 shell），因此没有命令注入面；需要 shell 特性请自行包一层脚本。

## 安全模型

- 插件是**你本人放进本机目录的可信可执行文件**——请只安装来源可信的插件。
- 插件可启动任意外部程序，并以当前用户权限运行；应用不提供安全沙箱。
- 插件目录必须是 `plugins/` 下的真实目录，不能用目录软链接指向目录外；相对可执行文件必须解析在该插件目录内。绝对可执行路径和 PATH 命令仍可使用。
- 插件参数直接传给进程，不经过 shell。应用持续排空 stdout/stderr，每个流各自最多保留 200 KiB，超出的部分继续排空但不保留。
- 输出按 UTF-8 解码；无效或被预算边界截开的字节使用 U+FFFD 替代。截断标记包含在每个流的 200 KiB 上限内。
- macOS/Linux 使用独立进程组停止仍留在该组中的后代；Windows 使用 `taskkill /T /F` 尝试结束后代。进程组外逃逸和 Windows 作业收尾尚未完成跨平台验收。
- `file` 模式先写入目标目录中的临时文件；仅在进程以 0 退出且临时文件存在、类型和扩展名有效后发布到所选路径。失败或超时不会把旧文件报告为本次产物。

## 示例

参见 [`docs/plugin-example/file-info/`](./plugin-example/file-info/)：一个零依赖的 Python 示例（打印输入文件的名称/大小/SHA-256），展示 `text` 模式与 `{input}`、`{dir}` 占位符用法。安装：

```bash
cp -r docs/plugin-example/file-info "<你的插件目录>/"
```

然后在 工具箱 → 插件 中选择输入 PDF 运行。
