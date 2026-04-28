# KBIntake

KBIntake 是一个面向 Windows 的本地知识库导入工具。它可以把文件或文件夹从 PowerShell 或 Windows Explorer 右键菜单导入到本地 vault，并用 SQLite 记录每一次导入、去重、失败和撤销信息。

当前版本：`v2.0.0`（开发中，分支 `v2.0`）

- 下载地址：<https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0>
- 英文 README：[README.md](README.md)

## 这个项目解决什么问题

如果你经常把 Markdown 笔记、PDF、截图、导出文件、参考资料放进 Obsidian 或其他本地知识库，手动复制文件会遇到几个问题：

- 不知道是否已经导入过同一个文件
- 目标文件夹和文件名容易混乱
- 导入失败后不好追踪
- 想撤销一次导入时，需要自己找文件
- Explorer 右键导入通常会闪出控制台窗口

KBIntake 把这些步骤变成可追踪的导入任务。它会扫描输入路径、校验文件、计算 SHA-256、按目标 vault 去重、安全复制文件、记录 manifest，并提供 jobs 命令查看状态。

默认 vault 路径类似：

```text
C:\Users\<你>\Documents\KBIntakeVault
```

所有运行状态默认保存在：

```text
%LOCALAPPDATA%\kbintake
```

## 安装

### 推荐方式

1. 打开 [v1.0.0 Release 页面](https://github.com/GeziP/windows-rightclick-vault-import/releases/tag/v1.0.0)。
2. 下载 `KBIntake-Setup.exe`。
3. 运行安装包。
4. 打开新的 PowerShell，执行：

```powershell
kbintake doctor
```

安装包会完成这些事情：

- 安装 `kbintake.exe`
- 安装无控制台窗口版本 `kbintakew.exe`
- 安装图标 `kbintake.ico`
- 把安装目录加入当前用户的 `PATH`
- 注册 Explorer 文件和文件夹右键菜单
- 在 Windows 设置里写入卸载项

安装目录：

```text
%LOCALAPPDATA%\Programs\kbintake
```

### winget 状态

项目已经准备并验证了 winget manifest，位置在：

```text
installer\winget\1.0.0
```

社区仓库 PR 已提交：<https://github.com/microsoft/winget-pkgs/pull/364698>

在 PR 合并到 public winget source 前，请先使用 GitHub Release 里的安装包。

发布到 winget 后，预计使用：

```powershell
winget install GeziP.KBIntake
```

## 快速开始

### Explorer 右键方式

1. 在资源管理器中右键一个文件或文件夹。
2. 选择 KBIntake 菜单项。
3. 导入完成后会出现 Windows toast 通知。
4. 打开 PowerShell 查看任务：

```powershell
kbintake jobs list
```

### 命令行方式

```powershell
kbintake doctor --fix
kbintake import --process C:\path\to\note.md
kbintake jobs list
```

## 已有功能

- Explorer 文件和文件夹右键导入
- Windows 11 原生顶级右键菜单（COM DLL）
- Explorer 导入不弹控制台窗口
- 成功、重复、失败场景的 Windows toast 通知
- PowerShell 导入命令
- SQLite 队列、批次、条目、manifest 和事件记录
- SHA-256 哈希和按目标 vault 去重
- 文件名冲突时安全改名，不覆盖已有文件
- 多 vault target 管理
- 导入模板系统（变量插值、条件渲染、继承）
- v2 多条件路由规则，绑定模板
- 每个目标可配置默认子文件夹
- Markdown 导入时自动写入 frontmatter（支持模板自定义字段）
- `--tags` 快速标签注入，与模板标签合并
- `--clipboard` 从 Windows 剪贴板读取文件路径并导入
- 可关闭 Markdown frontmatter 注入
- dry-run 预览，支持表格和 JSON
- jobs list/show/retry/undo
- 基于哈希的安全撤销，避免删除被修改过的文件
- vault stats 统计
- `vault audit` 审计命令（检测孤立、缺失、重复、异常 frontmatter）
- Watch Mode：监控目录自动导入新文件
- TUI 交互式设置界面（`kbintake tui`）
- Obsidian URI 集成（导入后自动打开笔记）
- 简体中文本地化（zh-CN），包括 CLI 输出、toast 通知、TUI 界面和 Explorer 右键菜单文字
- Windows Service 后台处理队列
- NSIS 单文件安装包
- GitHub Actions release 构建和发布资产
- winget manifest 草稿和本地验证

## 常用命令

```text
kbintake --version
kbintake doctor [--fix] [--migrate]
kbintake import [--target <target>] [--template <name>] [--tags "a,b"] [--clipboard] [--process] [--dry-run] [--json] [--open] <path...>
kbintake jobs list [--status <status>] [--limit <n>] [--json] [--table]
kbintake jobs show <batch-id> [--json] [--table]
kbintake jobs retry <batch-id>
kbintake jobs undo <batch-id> [--force]
kbintake targets list [--include-archived]
kbintake targets add <name> <path>
kbintake targets set-default <target>
kbintake vault stats [--target <target>] [--json]
kbintake vault audit [--target <target>] [--fix] [--json]
kbintake watch [--path <dir>]
kbintake tui
kbintake obsidian open --vault <name> <note-path>
kbintake explorer install
kbintake explorer uninstall
kbintake service install
kbintake service start
kbintake service stop
kbintake service uninstall
kbintake service status
```

## 配置

配置文件：

```text
%LOCALAPPDATA%\kbintake\config.toml
```

主要配置：

- `[[targets]]`：导入目标 vault（支持 `default_subfolder`、`obsidian_vault`）
- `[[templates]]`：导入模板（子文件夹、标签、frontmatter）
- `[[routing_rules]]`：v2 多条件路由，绑定模板
- `[[routing]]`：v1 按扩展名路由（仍支持）
- `[[watch]]`：自动监控导入的目录配置
- `[import].max_file_size_mb`：最大文件大小限制
- `[import].inject_frontmatter`：是否给 Markdown 注入 metadata
- `[import].language`：输出语言（`"en"` 或 `"zh-CN"`），同时控制 Explorer 右键菜单文字
- `[import].auto_open_obsidian`：导入后自动在 Obsidian 中打开
- `[agent].poll_interval_secs`：后台服务轮询间隔

查看当前配置：

```powershell
kbintake config show
```

完整说明见：[docs/CONFIGURATION.md](docs/CONFIGURATION.md)

## 后台服务

如果只想导入一次，可以使用：

```powershell
kbintake import --process C:\path\to\file.md
```

如果想让队列在后台持续处理，可以用 Windows Service。需要管理员 PowerShell：

```powershell
kbintake service install
kbintake service start
kbintake service status
```

停止并移除服务：

```powershell
kbintake service stop
kbintake service uninstall
```

Service 的 install/start/自动处理队列/日志/stop/uninstall 已验证。重启后自动恢复仍保留为发布检查清单里的人工验证项。

## 故障排查

优先运行：

```powershell
kbintake doctor
```

常见处理方式：

- target 文件夹缺失：`kbintake doctor --fix`
- 想切换默认 vault：`kbintake config set-target <path>`
- Explorer 菜单缺失：`kbintake explorer install`
- 升级后 schema 需要迁移：`kbintake doctor --migrate`
- 安装后找不到 `kbintake`：重新打开 PowerShell
- service install/start 权限不足：用管理员 PowerShell

## 从源码构建

安装 Rust 后执行：

```powershell
cd kbintake
cargo build --release --locked --bins
```

本地构建安装包需要 NSIS：

```powershell
cd E:\gezi\windows-rightclick-vault-import
New-Item -ItemType Directory -Force .\dist | Out-Null
Copy-Item .\kbintake\target\release\kbintake.exe .\dist\kbintake.exe -Force
Copy-Item .\kbintake\target\release\kbintakew.exe .\dist\kbintakew.exe -Force
Copy-Item .\kbintake\assets\kbintake.ico .\dist\kbintake.ico -Force
& "C:\Program Files (x86)\NSIS\makensis.exe" .\installer\kbintake.nsi
```

## 待开发和后续计划

- v2.0 发布：安装包更新、版本号升级、CHANGELOG、发布说明
- Windows 11 COM DLL 物理机验证
- 跟进 `microsoft/winget-pkgs` PR 合并
- 给发布二进制做 Authenticode 签名，降低 SmartScreen 提示
- 文档更新（模板示例库、配置参考）
- 补做 service reboot-resume 验证

更多状态见：[docs/PROJECT_STATUS.md](docs/PROJECT_STATUS.md)
