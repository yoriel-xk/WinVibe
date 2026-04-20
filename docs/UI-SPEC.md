# WinVibe UI 视觉与交互规范 (UI-SPEC)

**版本**: 0.2
**日期**: 2026-04-20
**配套**: PRD v0.9
**设计参考**: `DESIGN-apple.md`（Apple HIG 灵感 + Dynamic Island 形态）
**v0.2 变更**: HUD 最大宽度从 720px 放宽至 min(960px, viewport × 0.70)，补充像素预算复核；用量相关字段由"百分比进度条"改为"调用计数 + 字符量"以对齐 PRD §2.12

---

## 1. 设计原则

WinVibe 的 HUD 是开发者全天候可见的常驻元素。视觉系统必须兼顾**信息密度**与**克制美感**——既要在 32px 高度内呈现多 Agent 状态，又不能成为视觉噪音。核心原则：

1. **半透明玻璃 > 不透明面板**：HUD 应"漂浮"在桌面之上，而非"贴"在桌面上。直接复用 Apple 导航玻璃配方。
2. **单一强调色**：仅 `#0071e3`（Apple Blue）作为可交互元素强调色，其余依靠中性色 + 状态语义色。
3. **状态依靠形状与动画，不堆砌颜色**：脉冲、闪烁、虚线优先于额外颜色。
4. **等宽字体仅用于跳动数字**：倒计时、token 计数、进度百分比使用 SF Mono，避免视觉抖动。
5. **绝不使用渐变、纹理、emoji 装饰**。

---

## 2. 设计 Token

### 2.1 颜色 Token

```css
/* HUD 容器 */
--wv-glass-bg:           rgba(0, 0, 0, 0.78);
--wv-glass-border:       rgba(255, 255, 255, 0.08);
--wv-glass-blur:         saturate(180%) blur(20px);

/* 文字（深色玻璃上） */
--wv-text-primary:       #ffffff;
--wv-text-secondary:     rgba(255, 255, 255, 0.60);
--wv-text-tertiary:      rgba(255, 255, 255, 0.36);
--wv-text-disabled:      rgba(255, 255, 255, 0.24);

/* 状态语义色（参考 Apple 系统色） */
--wv-status-running:     #30d158;   /* 系统绿 */
--wv-status-waiting:     #ff9f0a;   /* 系统橙 */
--wv-status-error:       #ff453a;   /* 系统红 */
--wv-status-done:        #0a84ff;   /* 系统蓝 */
--wv-status-idle:        rgba(255, 255, 255, 0.32);
--wv-status-stale:       rgba(255, 255, 255, 0.32);  /* 配合虚线边框 */

/* 状态点描边（提升对比） */
--wv-status-dot-stroke:  rgba(0, 0, 0, 0.40);

/* 交互元素（唯一强调色） */
--wv-accent:             #0071e3;
--wv-accent-hover:       #0a84ff;
--wv-accent-pressed:     #006edb;
--wv-link-on-dark:       #2997ff;

/* 按钮 */
--wv-btn-secondary-bg:   transparent;
--wv-btn-secondary-bd:   rgba(255, 255, 255, 0.30);
--wv-btn-danger-text:    #ff453a;

/* Diff 高亮（克制版，避免 GitHub 鲜绿鲜红） */
--wv-diff-add-bg:        rgba(48, 209, 88, 0.14);
--wv-diff-add-text:      #4cd964;
--wv-diff-del-bg:        rgba(255, 69, 58, 0.14);
--wv-diff-del-text:      #ff6961;

/* 用量进度条 */
--wv-usage-normal:       #30d158;   /* < 70% */
--wv-usage-warning:      #ff9f0a;   /* 70-90% */
--wv-usage-critical:     #ff453a;   /* > 90% */
--wv-usage-track:        rgba(255, 255, 255, 0.12);

/* 阴影（仅一层，禁止叠加） */
--wv-shadow-hud:         0 8px 32px rgba(0, 0, 0, 0.32);
--wv-shadow-card:        rgba(0, 0, 0, 0.22) 3px 5px 30px 0px;

/* 焦点环（无障碍） */
--wv-focus-ring:         0 0 0 2px #0071e3;
```

### 2.2 字体 Token

```css
--wv-font-display:   "SF Pro Display", "PingFang SC", "Microsoft YaHei UI", "Helvetica Neue", sans-serif;
--wv-font-text:      "SF Pro Text", "PingFang SC", "Microsoft YaHei UI", "Helvetica Neue", sans-serif;
--wv-font-mono:      "SF Mono", "JetBrains Mono", "Consolas", monospace;
```

| 用途            | Family  | Size | Weight | Line-Height | Letter-Spacing |
| ------------- | ------- | ---- | ------ | ----------- | -------------- |
| Agent 名（折叠态）  | text    | 13px | 600    | 1.2         | -0.08px        |
| 活动子状态标签       | text    | 11px | 500    | 1.2         | 0              |
| 任务摘要（折叠态）     | text    | 12px | 400    | 1.2         | -0.08px        |
| 展开态主标题        | display | 15px | 600    | 1.24        | -0.15px        |
| 展开态正文         | text    | 13px | 400    | 1.47        | -0.08px        |
| 按钮文字          | text    | 13px | 500    | 1.0         | 0              |
| 倒计时 / token 数 | mono    | 12px | 500    | 1.0         | 0              |
| Diff / 代码块    | mono    | 12px | 400    | 1.5         | 0              |
| 队列项次要文字       | text    | 11px | 400    | 1.3         | 0              |

### 2.3 间距与圆角

```css
/* 间距 8 倍数 */
--wv-space-1:  2px;
--wv-space-2:  4px;
--wv-space-3:  6px;
--wv-space-4:  8px;
--wv-space-5:  12px;
--wv-space-6:  16px;
--wv-space-7:  24px;

/* 圆角 */
--wv-radius-dot:    50%;     /* 状态点 */
--wv-radius-chip:   6px;     /* 活动标签 chip */
--wv-radius-btn:    8px;     /* 标准按钮 */
--wv-radius-card:   12px;    /* 卡片 / 队列项 */
--wv-radius-hud:    16px;    /* HUD 容器（药丸形） */
--wv-radius-pill:   980px;   /* Feedback / Learn more 链接 */
```

### 2.4 动画曲线

```css
--wv-ease-standard:   cubic-bezier(0.32, 0.72, 0, 1);   /* Apple 标准 */
--wv-ease-emphasized: cubic-bezier(0.20, 0.00, 0.00, 1);
--wv-ease-spring:     cubic-bezier(0.34, 1.56, 0.64, 1); /* 轻微回弹 */

--wv-dur-instant:  120ms;
--wv-dur-fast:     180ms;
--wv-dur-base:     240ms;
--wv-dur-slow:     300ms;   /* HUD 折叠/展开 */
```

---

## 3. HUD 容器规格

### 3.1 折叠态

| 属性    | 值                                                    |
| ----- | ---------------------------------------------------- |
| 最小高度  | **32px**（逻辑像素，DPI 自适配）                               |
| 最大宽度  | **min(960px, viewport × 0.70)**（见下方像素预算）             |
| 最小宽度  | 200px                                                |
| 圆角    | `--wv-radius-hud` (16px)                             |
| 背景    | `--wv-glass-bg` + `backdrop-filter: --wv-glass-blur` |
| 边框    | `1px solid --wv-glass-border`                        |
| 阴影    | `--wv-shadow-hud`                                    |
| 内边距   | `4px 12px`                                           |
| 卡片间距  | 8px，加 1px `--wv-glass-border` 竖线分隔                   |
| 距屏幕顶部 | 8px（默认）                                              |

##### 像素预算复核（应评审反馈）

旧版 720px 上限在 1280px 屏幕上双 Agent 场景实测：
- 单卡片可用 ≈ (720 − 12×2 − 8 − 1) / 2 ≈ **343px**
- 扣除状态点(6) + 间距(6) + Agent 名(~80, 中文 6 字) + 间距(8) + chip(~52, "编辑中") + 间距(8) + badge(14) + 预留摘要(~160) = **334px**

余量仅 9px，一旦 Agent 名超过 6 字或启用像素猫就会触发 P2 降级。新版上限 960px 后，单卡片 ≈ 466px，可完整承载 P0-P2，P3（进度条/猫）按需显示。viewport < 1370px 时上限 `0.7 × viewport` 自动收缩，小屏会优雅降级到 P1。

### 3.2 展开态

| 属性   | 值                                         |
| ---- | ----------------------------------------- |
| 最大高度 | 400px（PRD §2.1）                           |
| 最大宽度 | 560px                                     |
| 最小宽度 | 360px                                     |
| 圆角   | `--wv-radius-hud` (16px)                  |
| 内边距  | 16px                                      |
| 展开动画 | height/opacity 240ms `--wv-ease-standard` |

### 3.3 折叠态字段优先级（信息密度降级）

当 HUD 宽度不足时，按下表从右往左隐藏字段。**绝不截断 Agent 名前 6 个字符**。

| 优先级   | 字段                  | 默认显示      | 何时隐藏         |
| ----- | ------------------- | --------- | ------------ |
| P0 必现 | 状态点（6px 圆点）         | ✅         | 永不           |
| P0 必现 | Agent 名（最多 12 字）    | ✅         | 永不           |
| P0 必现 | 审批 badge（数字角标）      | 仅有审批时     | 永不           |
| P1    | 活动子状态 chip          | ✅         | 卡片宽度 < 180px |
| P2    | 任务摘要（最多 24 字）       | ✅         | 卡片宽度 < 220px |
| P3    | 用量指示器（调用次数徽标，非百分比） | 仅设置预算时 | 卡片宽度 < 260px |
| P3    | 像素猫                 | 用户开启时     | 卡片宽度 < 240px |

> 多 Agent 并排时，每张卡片宽度 = `(HUD 宽度 - 间距) / Agent 数`。卡片间用 `1px solid --wv-glass-border` 竖线分隔，不用色块。

---

## 4. 核心组件规格

### 4.1 状态点 (Status Dot)

```
直径 6px，radius 50%，1px --wv-status-dot-stroke 描边
```

| 状态               | 颜色                    | 动画                                             |
| ---------------- | --------------------- | ---------------------------------------------- |
| RUNNING          | `--wv-status-running` | scale 1.0→1.08 脉冲，1.6s `--wv-ease-standard` 无限 |
| WAITING_APPROVAL | `--wv-status-waiting` | scale 1.0→1.18 + opacity 1→0.5 脉冲，1.2s 无限（更急促） |
| ERROR            | `--wv-status-error`   | 静止                                             |
| DONE             | `--wv-status-done`    | 进入时 scale 0→1，`--wv-ease-spring` 240ms         |
| IDLE             | `--wv-status-idle`    | 静止                                             |
| STALE            | `--wv-status-stale`   | 静止 + 卡片外层 1px 虚线边框 `rgba(255,255,255,0.24)`    |

### 4.2 活动子状态 Chip

```
高度 16px，padding 0 6px，radius --wv-radius-chip
背景 rgba(255,255,255,0.10)，文字 --wv-text-secondary
字体见 §2.2「活动子状态标签」
```

文案统一中文短词："思考中" / "阅读中" / "编辑中" / "测试中" / "等待中"。

### 4.3 审批 Badge

```
最小尺寸 14×14px（数字 ≤ 9），≥10 时变胶囊形
背景 --wv-status-waiting，文字白色 11px weight 600
位置：状态点右上角偏移 (-2, -2)
出现动画：scale 0→1 spring 200ms
```

### 4.4 按钮规格

| 类型            | 用途                       | 背景            | 文字                     | 边框                                | Radius | Padding  |
| ------------- | ------------------------ | ------------- | ---------------------- | --------------------------------- | ------ | -------- |
| Primary       | Approve                  | `--wv-accent` | `#fff`                 | none                              | 8px    | 8px 16px |
| Secondary     | Deny                     | transparent   | `#fff`                 | `1px solid --wv-btn-secondary-bd` | 8px    | 8px 16px |
| Tertiary Pill | Feedback... / Learn more | transparent   | `--wv-link-on-dark`    | `1px solid --wv-link-on-dark`     | 980px  | 6px 14px |
| Icon Ghost    | 折叠 / 关闭                  | transparent   | `--wv-text-secondary`  | none                              | 50%    | 6px      |
| Danger Text   | 全部 Deny                  | transparent   | `--wv-btn-danger-text` | none                              | 6px    | 6px 10px |

**通用状态：**

- Hover：背景叠加 `rgba(255,255,255,0.08)`
- Pressed：scale(0.97) + 背景叠加 `rgba(0,0,0,0.12)`
- Focus：`--wv-focus-ring` outline，offset 2px
- Disabled：opacity 0.4，cursor not-allowed

**禁忌：** 按钮不使用阴影；不使用渐变背景；不使用图标 + 文字混排（除非图标是状态语义符号）。

### 4.5 倒计时显示

审批超时倒计时永远在按钮上方右对齐显示：

```
4:23  ← SF Mono 12px weight 500，颜色随剩余时间渐变
```

| 剩余比例   | 颜色                                         |
| ------ | ------------------------------------------ |
| > 50%  | `--wv-text-secondary`                      |
| 20-50% | `--wv-status-waiting`                      |
| < 20%  | `--wv-status-error` + scale 1.0→1.05 1s 脉冲 |

### 4.6 Diff 渲染

```
SF Mono 12px / line-height 1.5
+ 行：背景 --wv-diff-add-bg，文字 --wv-diff-add-text，行首 "+" 同色
- 行：背景 --wv-diff-del-bg，文字 --wv-diff-del-text，行首 "-" 同色
未变更行：--wv-text-secondary
```

Diff 容器 `radius 8px`，`padding 8px 12px`，最大高度 200px 后 `overflow-y: auto`。滚动条样式：宽 4px，thumb `rgba(255,255,255,0.2)`。

### 4.7 用量进度条

**折叠态（mini）**：32×4px，`radius 2px`，背景 `--wv-usage-track`，填充按比例使用对应颜色。仅 ≥ 70% 时显示。

**展开态（full）**：100% 宽 × 6px，旁标 `62%` SF Mono。当 `accuracy != exact` 时百分比后加 `~` 前缀（如 `~62%`），并在右侧加 `(估算)` caption。

### 4.8 队列项 (Approval Queue Row)

```
高度 36px，padding 8px 12px，radius --wv-radius-card
hover：背景 rgba(255,255,255,0.06)
当前查看项：左侧 2px 实色 --wv-accent 竖条 + 背景 rgba(0,113,227,0.10)
```

字段：状态点 · Agent 名 · 工具 + 路径（截断） · `查看` 链接（右对齐）

### 4.9 像素猫 Mascot

- 容器：16×16px，position absolute，对齐状态点右侧
- 渲染：CSS sprite + `image-rendering: pixelated`
- 动画：CSS `steps(N)` 4 FPS
- z-index：低于审批 badge
- **演示模式**：当全屏检测命中演示类应用（PowerPoint, Keynote, OBS Studio, Zoom 共享屏幕）时强制隐藏，无视用户开关

---

## 5. 关键场景视觉草图

### 5.1 折叠态（双 Agent，Codex 等待审批）

```
┌────────────────────────────────────────────────────────────┐
│ ● Claude Code 编辑中 fix auth bug │ ● Codex² 等待中 db query │
└────────────────────────────────────────────────────────────┘
   绿点脉冲   chip      摘要        橙点+badge  chip   摘要
```

### 5.2 展开态（审批视图）

```
┌──────────────────────────────────────────────────┐
│ ● Codex   db/queries.ts                  ⤴ 跳转  │
│ ─────────────────────────────────────────────── │
│ 权限请求：Edit                            4:23   │
│ ┌──────────────────────────────────────────────┐ │
│ │ + return await db.query(sql, params);        │ │
│ │ - return db.query(sql + userInput);          │ │
│ └──────────────────────────────────────────────┘ │
│                                                  │
│ [ Approve ]  [ Deny ]   Feedback...              │
│ ─── 队列 (2) ─────────────────  全部 Deny       │
│ │● Claude  Edit auth.ts                   查看   │
│  ● Gemini   Write tests/auth.test.ts     查看   │
└──────────────────────────────────────────────────┘
```

### 5.3 全屏应用 toast（PRD §2.6）

```
┌────────────────────────────────────────────┐
│ ● Codex 等待审批 · Edit db/queries.ts      │
│                          [ 展开 HUD ]      │
└────────────────────────────────────────────┘
```

位于屏幕右下角，复用相同 glass 配方，5s 后淡出。点击后强制取消全屏 → 还原 HUD。

---

## 6. 动画规范

| 触发                 | 属性                       | 时长        | 缓动                   |
| ------------------ | ------------------------ | --------- | -------------------- |
| HUD 折叠↔展开          | height, opacity          | 300ms     | `--wv-ease-standard` |
| 卡片宽度变化（多 Agent 增删） | width                    | 240ms     | `--wv-ease-standard` |
| 状态点出现              | transform scale 0→1      | 200ms     | `--wv-ease-spring`   |
| 审批 badge 出现        | scale 0→1                | 180ms     | `--wv-ease-spring`   |
| 按钮 press           | scale → 0.97             | 120ms     | `--wv-ease-standard` |
| WAITING 脉冲         | scale + opacity          | 1200ms 循环 | `--wv-ease-standard` |
| RUNNING 脉冲         | scale                    | 1600ms 循环 | `--wv-ease-standard` |
| 队列项切换              | 背景渐变                     | 180ms     | `--wv-ease-standard` |
| toast 淡入/淡出        | opacity + translateY 8px | 240ms     | `--wv-ease-standard` |

**减弱动画：** 检测 `prefers-reduced-motion: reduce`，禁用所有脉冲与缩放，仅保留 opacity 渐变。

---

## 7. 无障碍

- 所有交互元素最小命中区域 28×28px（HUD 折叠态）/ 32×32px（展开态）
- 焦点环 `--wv-focus-ring` 不可被禁用
- 键盘快捷键（默认）：
  - `Ctrl+Shift+A` 切换 HUD 折叠/展开
  - `Tab` 在审批按钮间循环
  - `Y` Approve / `N` Deny（仅展开态聚焦时）
  - `Esc` 折叠 HUD
- 状态变化必须有非颜色指示（脉冲 / 形状 / 文字 chip），不依赖纯色区分
- 文字最小尺寸 11px；正文不小于 13px
- 颜色对比度：所有文字相对 `--wv-glass-bg` 满足 WCAG AA（4.5:1）

---

## 8. 禁忌清单

不允许出现的视觉元素：

- ❌ 任何颜色渐变（背景、按钮、文字、边框）
- ❌ 多层阴影或彩色阴影
- ❌ 圆角超过 16px 的矩形（仅 pill 例外）
- ❌ 字重 800/900；同一组件内超过 2 种字重
- ❌ Emoji 作为状态指示（除非是用户自定义任务摘要中包含）
- ❌ 鲜艳的 GitHub 风格 diff 配色
- ❌ 不透明的 HUD 背景（必须保留半透明玻璃）
- ❌ HUD 内嵌图片 / 头像 / Logo（除像素猫外）
- ❌ 居中对齐的代码 / Diff / 长文本
- ❌ 文字阴影（`text-shadow`）
- ❌ 边框宽度 > 1px（除焦点环）

---

## 9. 落地建议（实现层提示）

- 前端使用 CSS Custom Properties 暴露所有 token，便于主题切换
- HUD 容器使用 Tauri `decorations: false` + `transparent: true`，CSS 控制玻璃效果
- Windows：通过 `SetWindowCompositionAttribute` 启用 `ACCENT_ENABLE_BLURBEHIND` 增强玻璃效果（非必需）
- Linux：`backdrop-filter` 在部分 compositor 不生效，降级为 `rgba(0,0,0,0.92)` 不透明背景
- 像素猫 sprite sheet 单文件 ≤ 20KB，PNG-8 调色板模式
- 字体首选系统字体，不打包字体文件（控制安装包体积）
