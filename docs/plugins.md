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
| `id` | 可省略，默认取目录名 |
| `name` | 必填，工具箱中显示的名称 |
| `command` | 必填。相对路径（`./run.sh`、`bin/tool`）相对插件目录解析；绝对路径原样使用；裸命令名（`qpdf`、`python3`）走 PATH 查找 |
| `args` | 参数模板，支持占位符 |
| `output.mode` | `text`（默认）：插件向 stdout 输出，结果显示在面板；`file`：插件写文件到 `{output}`，运行前会弹出保存对话框 |
| `output.extension` | `file` 模式下保存对话框的默认扩展名 |
| `timeoutSecs` | 超时强杀，默认 120 |

## 占位符

| 占位符 | 含义 |
|--------|------|
| `{input}` | 输入 PDF 路径（当前打开的 PDF，或在面板中另行选择） |
| `{output}` | 输出文件路径（仅 `file` 模式，运行前由保存对话框确定） |
| `{dir}` | 插件目录绝对路径 |

参数直接传给子进程（不经过 shell），因此没有命令注入面；需要 shell 特性请自行包一层脚本。

## 安全模型

- 插件是**你本人放进本机目录的可信可执行文件**——请只安装来源可信的插件。
- 插件以当前用户权限运行；应用不做沙箱隔离（v1 范围）。
- 运行有超时保护；stdout/stderr 各截断至 200 KB 显示。

## 示例

参见 [`docs/plugin-example/file-info/`](./plugin-example/file-info/)：一个零依赖的 Python 示例（打印输入文件的名称/大小/SHA-256），展示 `text` 模式与 `{input}`、`{dir}` 占位符用法。安装：

```bash
cp -r docs/plugin-example/file-info "<你的插件目录>/"
```

然后在 工具箱 → 插件 中选择输入 PDF 运行。
