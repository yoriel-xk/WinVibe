# WinVibe

> Windows / Linux 平台的 AI Agent 监控 HUD — [Vibe Island](https://vibeisland.app) 的对等工具

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20Linux-lightgrey)](#兼容性)
[![Stack](https://img.shields.io/badge/stack-Tauri%202.0%20%2B%20Rust%20%2B%20React-orange)](#技术栈)

---

## 是什么

WinVibe 是一个常驻屏幕顶部的半透明悬浮 HUD，实时显示所有在后台运行的 AI 编程 Agent（Claude Code、Codex、Gemini CLI 等）的状态，并在 Agent 请求权限审批时立即弹出提示——无需切换到终端，不打断心流。

macOS 有 Dynamic Island 和 Vibe Island，Windows / Linux 开发者此前没有对等工具，WinVibe 填补这一空白。

---

## 演示

```
折叠态（32px，常驻屏幕顶部）
┌──────────────────────────────────────────────────────────────┐
│  ● Claude Code  编辑中  fix auth bug  │  ●² Codex  等待中  db query  │
└──────────────────────────────────────────────────────────────┘

展开态（审批视图）
┌──────────────────────────────────────────────────┐
│  ● Codex   db/queries.ts                  ⤴ 跳转 │
│  ─────────────────────────────────────────────── │
│  权限请求：Edit                            4:23  │
│  ┌──────────────────────────────────────────────┐│
│  │ + return await db.query(sql, params);        ││
│  │ - return db.query(sql + userInput);          ││
│  └──────────────────────────────────────────────┘│
│  [ Approve ]  [ Deny ]   Feedback...             │
│  ─── 队列 (2) ─────────────────  全部 Deny       │
│  │● Claude  Edit auth.ts                  查看  │
│   ● Gemini  Write tests/auth.test.ts      查看  │
└──────────────────────────────────────────────────┘
```

---

## 核心功能

| 功能 | 说明 |
|------|------|
| **多 Agent 监控** | 同时追踪 Claude Code、Codex、Gemini CLI 等，统一视角 |
| **实时审批** | Agent 请求 PreToolUse 权限时 HUD 立即闪烁，一键 Approve / Deny |
| **审批队列** | 多 Agent 同时请求时排队展示，支持批量 Deny |
| **零配置接入** | 首次启动自动检测已安装 Agent 并写入 hook 配置 |
| **终端跳转** | 一键聚焦到 Agent 运行所在的终端窗口 / Tab |
| **用量监控** | 追踪工具调用次数与字符量，可设置会话预算 |
| **全屏检测** | 全屏应用运行时自动隐藏 HUD，改用系统 toast 通知 |
| **像素猫彩蛋** | 可选的 16×16px 像素猫 mascot，与 Agent 状态联动 |

---

## 技术栈

- **Shell**：[Tauri 2.0](https://tauri.app)（Rust 后端 + WebView 前端）
- **后端**：Rust + Axum（Hook HTTP Server）
- **前端**：React + CSS Custom Properties（Apple 玻璃风格 HUD）
- **Hook 协议**：短轮询 + 幂等重提交（防休眠断连丢失审批）

---

## 兼容性

| 平台 | 最低版本 |
|------|----------|
| Windows | Windows 10 21H2 |
| Linux | Ubuntu 22.04 / Fedora 38 |

> macOS 不在计划内，请使用 [Vibe Island](https://vibeisland.app)。

---

## 安装

> **当前状态：设计阶段，尚未发布正式版本。**

正式发布后将提供：

- Windows：`.exe` 安装包（GitHub Releases）
- Linux：AppImage / `.deb`（GitHub Releases）

---

## Hook 配置（Claude Code 示例）

WinVibe 首次启动会自动完成以下配置，也可手动写入 `~/.claude/settings.json`：

```json
{
  "hooks": {
    "PreToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli pre-tool-use"}]}],
    "PostToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli post-tool-use"}]}],
    "Stop":        [{"hooks": [{"type": "command", "command": "winvibe-hookcli stop"}]}],
    "Notification":[{"hooks": [{"type": "command", "command": "winvibe-hookcli notification"}]}]
  }
}
```

`winvibe-hookcli` 是随主程序分发的轻量客户端（< 2MB），负责实现短轮询协议和幂等重提交逻辑。

---

## 键盘快捷键

| 快捷键 | 功能 |
|--------|------|
| `Ctrl+Shift+A` | 切换 HUD 折叠 / 展开 |
| `Y` | Approve（展开态聚焦时） |
| `N` | Deny（展开态聚焦时） |
| `Esc` | 折叠 HUD |
| `Tab` | 在审批按钮间循环 |

---

## 文档

| 文档 | 说明 |
|------|------|
| [PRD.md](docs/PRD.md) | 产品需求文档 v0.9（功能、交互、安全） |
| [UI-SPEC.md](docs/UI-SPEC.md) | 视觉与交互规范 v0.2（设计 Token、组件规格） |
| [DESIGN-apple.md](docs/DESIGN-apple.md) | Apple HIG 设计参考 |

---

## 贡献

欢迎通过 [Issues](https://github.com/yoriel-xk/WinVibe/issues) 和 [Discussions](https://github.com/yoriel-xk/WinVibe/discussions) 参与讨论。

新 Agent 适配可通过实现 `AgentAdapter` trait 贡献支持。

---

## License

[MIT](LICENSE)
