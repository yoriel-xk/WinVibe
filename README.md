# WinVibe

**你的 Agent 在工作。你也该在工作。**

WinVibe 是一个 32px 的悬浮 HUD，常驻屏幕顶部。多个 AI 编程 Agent 的实时状态、权限审批、任务进度——一眼尽收，无需切换终端。

Windows 和 Linux，现在有了 [Vibe Island](https://vibeisland.app) 的对等工具。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
&nbsp;
[![Platform](https://img.shields.io/badge/Windows%20%7C%20Linux-lightgrey)](#兼容性)

<br>

---

## 它长这样

```
● Claude Code  编辑中  fix auth bug  │  ●² Codex  等待中  db query
```

*折叠态。32px，始终在顶部，从不遮挡你的工作。*

<br>

Agent 请求权限审批时，HUD 展开：

```
┌──────────────────────────────────────────────────┐
│  ● Codex   db/queries.ts                  ⤴ 跳转 │
│  ─────────────────────────────────────────────── │
│  权限请求：Edit                            4:23  │
│                                                  │
│  + return await db.query(sql, params);           │
│  - return db.query(sql + userInput);             │
│                                                  │
│  [ Approve ]   [ Deny ]   Feedback...            │
│  ─── 队列 (2) ──────────────────  全部 Deny ─── │
│  │● Claude  Edit auth.ts                  查看  │
│   ● Gemini  Write tests/auth.test.ts      查看  │
└──────────────────────────────────────────────────┘
```

*一键 Approve 或 Deny。不离开当前窗口。*

<br>

---

## 为什么是 WinVibe

AI Agent 越来越普遍地在后台持续运行。开发者面临三个痛点：

- **切换成本高** — 频繁切换到终端查看进度，打断心流
- **审批时机难把握** — Agent 等待权限时你不知道，流程悄悄卡死
- **多 Agent 无全局视角** — 同时跑三个 Agent，靠记忆维护状态

WinVibe 把这三个问题压缩进 32px。

<br>

---

## 核心能力

**多 Agent，统一视角**
同时追踪 Claude Code、Codex、Gemini CLI 及任意支持 hook 的 Agent。状态、活动、任务摘要，折叠态一览无余。

**权限审批，零延迟**
Agent 发出 PreToolUse 请求的瞬间，HUD 橙色闪烁。展开即见 diff，`Y` 通过，`N` 拒绝。多条请求自动排队，支持批量 Deny。

**终端跳转，一键聚焦**
点击 ⤴ 跳转，WinVibe 直接把焦点切到 Agent 所在的终端窗口，不用鼠标翻找。

**零配置接入**
首次启动自动检测已安装的 Agent，生成认证 Token，写入 hook 配置。重启 Agent 即生效，无需手动编辑任何文件。

**全屏感知**
检测到全屏应用（游戏、PPT、视频会议）时自动隐藏 HUD，改用系统 toast 通知，不遮挡演示内容。

**用量追踪**
实时累计工具调用次数与字符量。可设置会话预算，进度条在 70% / 90% 时分级变色预警。

**像素猫**（可选）
16×16px 的像素猫 mascot，与 Agent 状态联动 — 思考时左右张望，审批等待时伸爪催你，任务完成时尾巴竖起。默认关闭，在设置中开启。

<br>

---

## 键盘快捷键

| 按键 | 动作 |
|------|------|
| `Y` | Approve |
| `N` | Deny |
| `Esc` | 折叠 HUD |
| `Tab` | 在审批按钮间循环 |
| `Ctrl+Shift+A` | 切换折叠 / 展开 |

<br>

---

## 安装

> **当前处于设计阶段，尚未发布。** 正式版本将通过 GitHub Releases 分发。

| 平台 | 格式 |
|------|------|
| Windows 10 21H2+ | `.exe` 安装包 |
| Ubuntu 22.04 / Fedora 38+ | AppImage · `.deb` |

macOS 已有 [Vibe Island](https://vibeisland.app)，WinVibe 不计划支持。

<br>

---

## Hook 配置

WinVibe 首次启动会自动完成配置。如需手动接入，在 `~/.claude/settings.json` 中添加：

```json
{
  "hooks": {
    "PreToolUse":  [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli pre-tool-use"}]}],
    "PostToolUse": [{"matcher": ".*", "hooks": [{"type": "command", "command": "winvibe-hookcli post-tool-use"}]}],
    "Stop":        [{"hooks": [{"type": "command", "command": "winvibe-hookcli stop"}]}],
    "Notification":[{"hooks": [{"type": "command", "command": "winvibe-hookcli notification"}]}]
  }
}
```

`winvibe-hookcli` 随主程序分发，< 2MB，自动处理短轮询、断连重试与休眠唤醒恢复。

<br>

---

## 技术

Tauri 2.0 · Rust · React · Axum

短轮询 + 幂等重提交协议，防止系统休眠或代理断连时审批请求丢失。Hook Server 仅监听 `127.0.0.1`，Bearer Token 认证，审批决策写入审计日志。

<br>

---

## 文档

[PRD v0.9](docs/PRD.md) · [UI-SPEC v0.2](docs/UI-SPEC.md) · [Design Reference](docs/DESIGN-apple.md)

<br>

---

## 贡献

新 Agent 适配通过实现 `AgentAdapter` trait 提交。欢迎在 [Issues](https://github.com/yoriel-xk/WinVibe/issues) 和 [Discussions](https://github.com/yoriel-xk/WinVibe/discussions) 参与讨论。

[MIT License](LICENSE)
