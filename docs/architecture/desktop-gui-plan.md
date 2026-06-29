# MFTX Desktop GUI 计划文档

> 文档状态：GUI 实施前产品与技术设计  
> 目标版本：MFTX Desktop v0.1  
> 适用仓库：`E:\jhw\mftx-project`  
> 当前基线：Rust workspace 已具备 `PeerNode`、发现、双向 push/pull、CLI/TUI 基础能力  
> 推荐技术栈：Tauri v2 + React/Vite/TypeScript + HeroUI Pro + 现有 `mft_core`

---

## 1. 设计结论

MFTX 最适合做成一个“托盘常驻的局域网文件传输工具”，而不是传统管理后台或营销式首页。前端应走 **Tauri v2 + React + HeroUI Pro**：Tauri 负责桌面壳和 Rust bridge，React 负责交互，HeroUI Pro 负责 app shell、列表、拖拽、表格、空状态和设置表单的设计系统。

推荐界面形态：

```text
系统托盘 / macOS 菜单栏
  ├─ 在线状态
  ├─ 打开主窗口
  ├─ 打开收件箱
  ├─ 暂停/恢复接收
  ├─ 复制本机地址
  └─ 退出

主窗口
  ├─ 顶部状态栏：本机名称、监听端口、peer 状态、接收开关
  ├─ 左侧设备列表：附近设备、地址、最近在线、能力
  ├─ 右侧发送区：拖拽文件/文件夹、选择目标、发送
  ├─ 底部传输队列：发送中、接收中、完成、失败
  └─ 设置页：设备名、监听端口、收件箱、共享目录、信任设备、开机启动
```

### 1.1 2026-06-24 产品纠偏

GUI 主流程必须从“主动 pull + 共享密码”改为“主动发送 + 被动接收”：

- 发送方选择文件和目标设备后发起 `TransferOffer`。
- 接收方收到 offer 时才弹出确认：接收 / 拒绝。
- 接收确认里提供 `信任此设备` 选项；信任对象必须是稳定设备身份，例如持久 `device_id` 或设备公钥，不是 IP，也不是本次 `session_id`。
- 被信任设备下次向本机发送文件时可自动接收，仍写入 inbox/接收目录。
- 局域网直连主流程不应要求用户输入传输密码。当前 `PasswordRecord` 和 password handshake 是 legacy/兼容实现，不能继续作为 GUI 的核心心智模型。
- `pull_from_peer` 保留为高级/兼容能力，不放在主传输流程里强迫普通用户理解“来源/目标”。
- 设置页只暴露“监听端口”；底层永远 bind `0.0.0.0:<port>`。
- 未来 OSS/对象存储作为远程中继时，优先使用登录态、设备身份、短期 STS/预签名 URL 或一次性 transfer token；密码只适合作为可选的端到端加密口令/邀请口令，不应复用为局域网传输密码。

第一版目标不是“功能最多”，而是把核心工作流做顺：

1. 用户启动应用后 peer 自动在线。
2. 局域网里的设备自动出现。
3. 把文件拖到窗口里，选设备，发送。
4. 收到其他设备的发送请求时，先让用户确认是否接收。
5. 用户可信任某台设备；下次该设备发送时自动接收。
6. 收到文件后有通知，能一键打开收件箱。
7. 出错时能看懂原因，比如找不到设备、防火墙、连接中断或设备未受信任。

---

## 2. 调研依据与设计来源

Tauri v2 适合本项目的原因：

- Tauri 使用 Rust 后端和系统 WebView，适合复用当前 Rust core。
- Tauri command 支持前端调用 Rust 异步函数，可封装 discovery、send、pull、config 等操作。
- Tauri event/channel 支持 Rust 向前端推送状态，适合传输进度、peer 上下线、错误事件。
- Tauri 支持 system tray、notification、autostart、updater、single instance 等桌面软件能力。

Tauri 官方资料：

- Tauri 架构：https://v2.tauri.app/concept/architecture/
- 前端调用 Rust command：https://v2.tauri.app/develop/calling-rust/
- Rust 调前端事件：https://v2.tauri.app/develop/calling-frontend/
- System tray：https://v2.tauri.app/learn/system-tray/
- Notification plugin：https://v2.tauri.app/plugin/notification/
- Autostart plugin：https://v2.tauri.app/plugin/autostart/
- Updater plugin：https://v2.tauri.app/plugin/updater/
- File system plugin：https://v2.tauri.app/plugin/file-system/

HeroUI Pro 资料来源：

- 组件文档仓库：https://github.com/Hanwen-Jiang/hp-component-docs
  - 已确认 `main` 分支包含 `react/*.md` 和 `native/*.md` MDX 文档。
  - MFTX Desktop 优先使用 `react/` 文档。
  - 关键组件：`app-layout`、`sidebar`、`navbar`、`drop-zone`、`list-view`、`data-grid`、`empty-state`、`segment`、`progress-button`、`sheet`。
- 模板仓库：https://github.com/Hanwen-Jiang/heroui-v3-templates
  - 已确认包含 `react/template-dashboard`、`react/template-email`、`react/template-chat`、`react/template-finances` 和 `native/crypto-wallet`。
  - `react/template-dashboard` 使用 Next.js 16 + HeroUI Pro，适合作为 MFTX 的 app shell、sidebar、navbar、settings、diagnostics 布局参考。
  - MFTX Desktop 仍推荐 Vite/Tauri，不照搬 Next.js App Router；只迁移模板的组件组合、布局密度、导航结构和样式导入方式。
- 本地 Figma kit：
  - `E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Figma Kit V3-v1.0.4.fig`
  - `E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Native Figma Kit-v1.0.2.fig`
  - 已确认 `.fig` 文件为 zip 容器，包含 `canvas.fig`、`thumbnail.png`、`meta.json` 和 images。`meta.json` 显示 Web kit 文件名为 `HeroUI Pro Figma Kit V3`，Native kit 文件名为 `HeroUI Pro Native Figma Kit`，导出时间均为 2026-05-28。

技能与 MCP 使用策略：

- 使用 GitHub skill / `gh` 读取私有仓库结构、README、模板代码和组件文档。
- 使用 Browser skill / in-app browser 在实现阶段检查 Tauri/Vite 页面、截图和交互状态。
- 使用 `tool_search` 查询 Figma MCP 能力；如果当前会话暴露 Figma MCP，优先用它读取 frame、component、token、截图；如果没有暴露 Figma 专用 MCP，则以 `E:\figma` 本地 `.fig` 文件和 GitHub 组件文档作为设计依据，并在实现记录中说明无法直接读取 Figma 节点。

备选方案结论：

| 方案 | 优点 | 风险 | 结论 |
|---|---|---|---|
| Tauri v2 + React + HeroUI Pro | 复用 Rust，UI 灵活，桌面能力完整，设计系统和 Figma kit 对齐 | 需要 HeroUI Pro npm registry 权限 | 首选 |
| Slint | Rust 集成干净，轻量 native UI | 复杂交互和生态弱一些 | 适合未来极简版 |
| egui/eframe | 开发快，适合工具 | 消费级文件传输体验偏粗糙 | 不推荐做主 GUI |
| Flutter | UI 能力强，跨平台成熟 | Rust core 集成成本高，引入 Dart 生态 | 不作为首选 |

---

## 3. 项目使用方法

本节用于指导后续实现者如何使用 HeroUI Pro 相关资源，而不是 MFTX 最终用户的使用说明。

### 3.1 凭据和入口

HeroUI Pro 的安装和 AI 工具凭据是两套体系：

- `hpsetup` 使用 **HP Key**，用于安装 Pro 依赖。
- MCP 和 Skills 使用 **Personal Token**，用于让 AI 读取组件文档、源码和设计原则。
- 凭据仅通过链接配置，不再手动输入；已配置凭据只能删除后重新配置。
- 备用域名：`collectui.vip`。如果 `collectui.pro` 被安全浏览拦截，使用备用域名查看文档和配置凭据。

### 3.2 安装 HeroUI Pro 依赖

在 `apps/desktop` 创建完成并能运行基础 Vite/Tauri 项目后，再安装 HeroUI Pro：

```powershell
cd E:\jhw\mftx-project\apps\desktop
npm install
npx -y hpsetup@latest <HP_KEY>
```

如果使用 pnpm：

```powershell
cd E:\jhw\mftx-project\apps\desktop
pnpm install
pnpm dlx hpsetup@latest <HP_KEY>
```

注意：

- 文档显示 `hpsetup` 支持 npm / pnpm / bun / yarn 等包管理器。
- 项目首选 npm 或 pnpm；如果使用 pnpm 遇到 `ERR_PNPM_IGNORED_BUILDS`，运行 `pnpm approve-builds` 后继续。
- `hpsetup` 会访问 npm registry 检查版本，并通过 CDN 下载 Pro tarball；本地缓存命中时不会重复下载。
- 不要把 HP Key 写入仓库、脚本或文档。

### 3.3 引入样式

React + Vite / Tauri 前端的全局 CSS 需要引入 Tailwind 和 HeroUI 样式。根据附件中的 React + Vite 用法，推荐：

```css
@import "tailwindcss";
@import "@heroui/styles";
@import "@heroui-pro/react/css";
```

如果采用 `heroui-v3-templates/react/template-dashboard` 的 Tailwind v4 写法，也可以使用：

```css
@import "@heroui/styles/css";
@import "@heroui-pro/react/css";

@source "../**/*.{ts,tsx}";
```

最终以安装后能通过 Vite build 的写法为准。若 `@source` 路径不生效，按 `apps/desktop/src/styles/globals.css` 的实际位置调整到覆盖 `src/**/*.{ts,tsx}`。

### 3.4 使用组件文档

优先读取 `hp-component-docs` 的 React 文档：

```powershell
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/hp-component-docs/contents/react/app-layout.md
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/hp-component-docs/contents/react/drop-zone.md
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/hp-component-docs/contents/react/list-view.md
```

使用规则：

- 先读组件 Anatomy 和 API Reference，再写组件。
- 首选 `@heroui-pro/react` 的复合组件，不自己重造已有组件。
- 组件导入保持清晰：基础组件从 `@heroui/react`，Pro 组件从 `@heroui-pro/react`。
- 如果文档和模板代码冲突，以组件文档为准，模板只做组合参考。

### 3.5 使用模板仓库

读取模板：

```powershell
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/heroui-v3-templates/contents/react/template-dashboard/README.md
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/heroui-v3-templates/contents/react/template-dashboard/src/components/app-shell.tsx
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/heroui-v3-templates/contents/react/template-dashboard/src/components/dashboard-sidebar.tsx
gh api -H "Accept: application/vnd.github.raw" repos/Hanwen-Jiang/heroui-v3-templates/contents/react/template-dashboard/src/components/dashboard-navbar.tsx
```

迁移原则：

- 只借 shell、sidebar、navbar、settings 页面结构。
- 不复制 Next.js App Router、`next/navigation`、server component 写法。
- 把 `router.push` 改成本地 view state、React Router 或 TanStack Router。
- 将 dashboard nav 改成 MFTX 的 `Transfer`、`Inbox`、`Settings`、`Diagnostics`。

### 3.6 使用 Figma kit

本地设计资产：

```text
E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Figma Kit V3-v1.0.4.fig
E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Native Figma Kit-v1.0.2.fig
```

使用规则：

- MFTX Desktop 是 Web/Tauri GUI，优先使用 `HeroUI Pro Figma Kit V3`，Native kit 只作移动端或 future native 参考。
- 若当前环境有 Figma MCP，先用 MCP 读取目标组件 frame、tokens 和截图。
- 若没有 Figma MCP，用 Figma 桌面端打开本地 `.fig` 文件，人工核对 AppLayout、Sidebar、Navbar、DropZone、ListView、DataGrid、EmptyState 等组件样式。
- 实现完成后用浏览器/Tauri 截图与 Figma kit 的 spacing、radius、surface、font weight、状态色做人工对照。

### 3.7 使用 Skills 和 MCP

建议在实现前安装或确认这些技能：

- `heroui-react-pro`：React Pro 组件模式、API、CSS。
- `heroui-pro-design-taste`：HeroUI Pro 设计原则。
- 如后续做移动端，再使用 `heroui-native-pro`。

MCP 使用策略：

- `list_components`：查可用组件。
- `get_component_docs`：查 Pro + OSS 组件文档。
- `get_component_source_code`：查 OSS 组件源码。
- `get_css` / `get_theme_variables`：查样式和 token。

当前会话如未暴露 HeroUI Pro MCP，也要在 prompt 中保留此流程，后续接入 MCP 时优先使用 MCP 数据。

### 3.8 本地运行命令

开发期：

```powershell
cd E:\jhw\mftx-project
cargo test --workspace

cd E:\jhw\mftx-project\apps\desktop
npm install
npm run tauri dev
```

验证期：

```powershell
cd E:\jhw\mftx-project\apps\desktop
npm run typecheck
npm run lint
npm run build
npm run tauri build
```

如果使用 pnpm，将 `npm run <script>` 替换为 `pnpm <script>`。

---

## 4. 用户与场景

目标用户：

- 同一局域网里频繁在 Mac 和 Windows 之间传文件的人。
- 不想搭 SMB、网盘、账号系统或公网中继的人。
- 不想记共享密码，只希望像 AirDrop 一样确认接收或信任设备的人。

核心场景：

1. Mac 发文件到 Windows。
2. Windows 发文件到 Mac。
3. 对方发来文件时，本机确认接收。
4. 信任某台设备后，下次该设备发送文件自动接收。
5. 临时收一个文件，直接落到固定 inbox。
6. 发现失败时使用手动地址兜底。
7. 应用关闭窗口后仍在托盘常驻接收。

非目标：

1. 公网穿透、账号系统、云同步。
2. 多人团队文件库。
3. 聊天、剪贴板同步、远程桌面。
4. 首版不做移动端。
5. 首版不做复杂权限模型。

---

## 5. 信息架构

### 5.1 主窗口结构

```text
┌─────────────────────────────────────────────────────────────┐
│ MFTX   Lou-Win online   port 48151   Receiving: on          │
├──────────────────────┬──────────────────────────────────────┤
│ Nearby Devices       │ Send                                 │
│                      │                                      │
│ Haven-Mac            │ Drop files or folders here            │
│ 192.168.1.10:48151   │ Selected target: Haven-Mac            │
│ seen now             │                                      │
│                      │ [Choose files] [Choose folder] [Send] │
│ Another-PC           │                                      │
│ offline 42s          │ Incoming requests                    │
│                      │ Haven-Mac wants to send 3 files      │
│                      │ [Reject] [Accept] [Trust this device]│
├──────────────────────┴──────────────────────────────────────┤
│ Transfers                                                   │
│ a.zip -> Haven-Mac        62%   48 MB/s                     │
│ photos <- Lou-Win         Done                              │
└─────────────────────────────────────────────────────────────┘
```

布局原则：

- 第一屏就是可用工具，不做 landing page。
- 不做大卡片套小卡片。
- 操作区紧凑、清楚、适合重复使用。
- 文件传输是任务型界面，视觉要安静、稳定、信息密度适中。

### 5.2 页面/视图

主窗口包含四个一级视图：

1. `Transfer`
   - 默认视图。
   - 设备列表、拖拽发送、传输队列。

2. `Inbox`
   - 显示最近收到的文件。
   - 支持打开文件、打开所在文件夹、清除历史记录。

3. `Settings`
   - 设备名。
   - 监听端口。
   - 收件箱目录。
   - 共享目录。
   - 信任设备列表。
   - 接收确认策略。
   - 开机启动。
   - 自动接收开关。
   - 发现开关。

4. `Diagnostics`
   - 本机地址。
   - TCP 监听状态。
   - UDP discovery 状态。
   - 最近错误。
   - 复制诊断信息。
   - 防火墙提示。

### 5.3 托盘菜单

Windows 托盘和 macOS 菜单栏共用语义：

```text
MFTX: Online
Open MFTX
Open Inbox
Receiving: On
Copy Local Address
Nearby Devices: 2
Settings
Quit
```

托盘状态：

| 状态 | 含义 |
|---|---|
| Online | PeerNode 正常运行 |
| Offline | PeerNode 未运行或启动失败 |
| Transferring | 有活跃传输 |
| Attention | 有失败或需要用户处理的问题 |

---

## 6. 首次启动体验

首次启动不要直接展示空主界面，应显示简短 setup wizard。

步骤：

1. 设备名称
   - 默认使用 hostname。
   - 允许用户改成 `Lou-Win`、`Haven-Mac` 这种可读名称。

2. 监听端口
   - 默认 `48151`。
   - UI 只显示端口，不显示 `0.0.0.0`。
   - 后端固定 bind `0.0.0.0:<port>`。

3. 收件箱目录
   - Windows 默认：`%USERPROFILE%\Downloads\MFTX Inbox`
   - macOS 默认：`~/Downloads/MFTX Inbox`

4. 共享目录
   - 可选。
   - 首版允许为空；为空时仍能接收别人 push。

5. 启动偏好
   - 开机启动。
   - 启动后自动在线。
   - 关闭窗口后留在托盘。
   - 默认收到陌生设备发送请求时询问。

完成后：

- 保存配置。
- 启动 `PeerNode`。
- 进入主窗口。

---

## 7. 关键工作流

### 7.1 发送文件

```text
用户拖入文件/文件夹
  -> 前端读取 path 列表
  -> 用户选择目标 peer
  -> 调用 send_paths command
  -> Rust 后端启动 transfer task
  -> 后端持续 emit transfer events
  -> 前端更新进度条和速度
  -> 完成后显示 Done，可打开目标说明
```

要求：

- 支持多文件和文件夹。
- 拖拽后不立刻发送，先进入待发送列表。
- 如果没有选中目标，`Send` 按钮禁用。
- 如果发现列表为空，显示手动连接入口。

### 7.2 接收文件

```text
PeerNode 常驻
  -> 对端发起 TransferOffer
  -> 如果设备未受信任，后端 emit incoming request
  -> 前端显示接收 / 拒绝，并可勾选信任此设备
  -> 用户接收后 core 写入 inbox
  -> 后端 emit receive started/progress/finished
  -> 前端显示入站传输
  -> 完成后系统通知
```

要求：

- 陌生设备默认询问。
- 信任必须绑定稳定设备身份，不绑定 IP。
- 被信任设备可自动接收。
- 接收目录由设置页控制。
- 收到文件后提供 `Open Inbox`。

### 7.3 拉取共享内容

`pull_from_peer` 是高级/兼容能力，不是桌面 GUI 主流程。只有在明确保留共享目录浏览/拉取时才暴露，并应放到设备详情或高级操作中。

```text
选择 peer
  -> 点击 Pull
  -> 选择输出目录
  -> 调用 pull_from_peer command
  -> 使用现有 pull resume 逻辑
  -> 显示传输进度
```

首版可做简单的 `Pull all`，不必先做远程文件浏览器。

### 7.4 手动连接

当 discovery 找不到设备时：

```text
输入 ip:port
  -> 测试连接
  -> 如果成功，临时加入目标列表
  -> 可发送或 pull
```

手动连接是防火墙、广播受限、跨子网等场景的必要兜底。

---

## 8. HeroUI Pro 视觉与交互设计

整体风格：

- 静、清楚、工具感，不做花哨插画。
- 遵循 HeroUI Pro Figma Kit V3 的 spacing、radius、surface、foreground、muted、accent、success、warning、danger token。
- 色彩不要单一蓝紫，也不要满屏深色；以 HeroUI 的 neutral surface 为主，只在状态和主要按钮上使用 accent。
- 推荐浅色默认，支持跟随系统深色。
- 强调状态色：在线绿、传输蓝、警告黄、失败红，但面积要克制。

推荐组件映射：

| MFTX UI | HeroUI Pro / HeroUI 组件 | 来源 |
|---|---|---|
| 整体 shell | `AppLayout` + `Sidebar` + `Navbar` | `hp-component-docs/react/app-layout.md`、`sidebar.md`、`navbar.md`、`template-dashboard/src/components/app-shell.tsx` |
| 左侧导航 | `Sidebar.MenuItem` | `template-dashboard/src/components/dashboard-sidebar.tsx` |
| 顶部状态栏 | `Navbar` + `Chip` + `Button` + `Tooltip` | `template-dashboard/src/components/dashboard-navbar.tsx` |
| 附近设备列表 | `ListView` | `hp-component-docs/react/list-view.md` |
| 拖拽发送区 | `DropZone` | `hp-component-docs/react/drop-zone.md` |
| 传输队列 | `DataGrid` 或 `ListView` + progress | `hp-component-docs/react/data-grid.md` |
| 空状态 | `EmptyState` | `hp-component-docs/react/empty-state.md` |
| Transfer / Inbox / Settings 切换 | `Segment` 或 sidebar route | `hp-component-docs/react/segment.md` |
| 设置表单 | `Form`、`TextField`、`Select`、`Switch`、`Button` | `@heroui/react` |
| 错误与诊断 | `Alert`、`Sheet`、`Tooltip` | `@heroui/react`、`@heroui-pro/react` |
| 图标 | `@gravity-ui/icons` 为主 | `template-dashboard/package.json` |

设计约束：

- 优先使用 HeroUI Pro 组件，不手写已有组件的替代品。
- 从 `@heroui-pro/react` 和 `@heroui/react` 直接导入组件，不创建无意义 barrel wrapper。
- 可写少量 app-specific 组合组件，例如 `PeerList`、`TransferQueue`、`SendDropZone`、`DesktopShell`。
- 样式使用 Tailwind v4 utility + HeroUI token，不写大段自定义 CSS。
- 组件 class 自定义优先使用 HeroUI 文档里的 BEM class 和 CSS variables。
- 图标按钮必须有 `aria-label` 和 tooltip。
- 卡片半径遵循 HeroUI token，不额外做大圆角装饰。
- 页面 sections 不做层层嵌套卡片；列表、表格、设置分区才使用 surface/card。
- Figma 对齐顺序：先看本地 Web kit，再看 `hp-component-docs/react/*.md`，最后看模板代码。

文本原则：

- 不用大段说明。
- 错误文案要可行动。
- 不在界面里解释技术实现。

示例错误文案：

| 错误 | 文案 |
|---|---|
| 找不到 peer | `No device named Lou-Win found. Try manual address.` |
| 设备未受信任 | `Lou-Win wants to send files. Accept this transfer or trust this device.` |
| 连接失败 | `Could not connect to 192.168.1.20:48151. Check firewall or address.` |
| 校验失败 | `Integrity check failed. The partial file was not completed.` |
| 目标已存在 | `File already exists in inbox. Rename or change overwrite policy.` |

---

## 9. 技术架构

### 9.1 目录结构

推荐新增：

```text
apps/
  desktop/
    package.json
    index.html
    src/
      main.tsx
      App.tsx
      api/
      components/
        shell/
        transfer/
        peers/
        inbox/
        settings/
        diagnostics/
      state/
      styles/
    src-tauri/
      Cargo.toml
      tauri.conf.json
      src/
        lib.rs
        main.rs
        commands.rs
        runtime.rs
        events.rs
        models.rs
```

并把 `apps/desktop/src-tauri` 加入 root Cargo workspace members。

选择原因：

- Tauri 标准项目结构更自然。
- 不污染现有 CLI crates。
- Rust GUI 后端可以用 path dependency 复用 `mft-core`、`mft-protocol`。
- 前端可以迁移 `heroui-v3-templates/react/template-dashboard` 的 shell 结构，但保留 Vite/Tauri 的客户端路由。

### 9.1.1 前端依赖

推荐 `apps/desktop/package.json` 依赖：

```json
{
  "dependencies": {
    "@gravity-ui/icons": "2.18.0",
    "@heroui-pro/react": "latest",
    "@heroui/react": "3.0.5",
    "@heroui/styles": "3.0.5",
    "@tauri-apps/api": "latest",
    "react": "19.2.6",
    "react-dom": "19.2.6",
    "zustand": "latest"
  },
  "devDependencies": {
    "@tailwindcss/postcss": "4.2.2",
    "@types/react": "19.2.14",
    "@types/react-dom": "19.2.3",
    "@vitejs/plugin-react": "latest",
    "postcss": "8.5.9",
    "tailwindcss": "4.2.2",
    "typescript": "5.9.3",
    "vite": "latest"
  }
}
```

注意：

- `@heroui-pro/react` 需要 npm registry 权限。若安装失败，先检查 `.npmrc`、token 或私有 registry 配置，不要降级到自写组件。
- `heroui-v3-templates/react/template-dashboard` 是 Next.js 模板；MFTX Desktop 使用 Vite 时不能直接复制 `next/navigation`、App Router 目录和 server-only 写法。
- 全局 CSS 需包含：

```css
@import "@heroui/styles/css";
@import "@heroui-pro/react/css";

@source "../**/*.{ts,tsx}";
```

Vite 项目中的 `@source` 路径需要按最终 CSS 文件位置调整，确保 `apps/desktop/src/**/*.{ts,tsx}` 被 Tailwind v4 扫描。

### 9.2 Rust 后端模块

```text
src-tauri/src/
  main.rs        # Tauri entry
  lib.rs         # builder setup
  commands.rs    # #[tauri::command] facade
  runtime.rs     # PeerRuntime manager
  events.rs      # event payloads and emit helpers
  models.rs      # DTOs shared with frontend
```

`PeerRuntime` 负责：

- 持有当前 `AppConfig`。
- 启停 `PeerNode`。
- 管理正在运行的 transfer task。
- 把 `ProgressEvent` 转换为 Tauri event。
- 保留传输历史。
- 给托盘菜单提供当前状态。

建议类型：

```rust
pub struct DesktopState {
    runtime: tokio::sync::Mutex<PeerRuntime>,
}

pub struct PeerRuntime {
    config: Option<AppConfig>,
    node: Option<PeerNode>,
    transfers: HashMap<Uuid, TransferHandle>,
}
```

### 9.3 Tauri commands

第一版 commands：

```rust
#[tauri::command]
async fn get_app_state(...) -> Result<AppStateDto, String>;

#[tauri::command]
async fn complete_setup(input: SetupInput) -> Result<AppStateDto, String>;

#[tauri::command]
async fn start_peer(...) -> Result<PeerStatusDto, String>;

#[tauri::command]
async fn stop_peer(...) -> Result<(), String>;

#[tauri::command]
async fn discover_peers(seconds: u64) -> Result<Vec<PeerDto>, String>;

#[tauri::command]
async fn send_paths(input: SendPathsInput) -> Result<TransferDto, String>;

#[tauri::command]
async fn respond_incoming_transfer(input: IncomingDecisionInput) -> Result<(), String>;

#[tauri::command]
async fn trust_device(input: TrustDeviceInput) -> Result<AppStateDto, String>;

#[tauri::command]
async fn untrust_device(input: TrustDeviceInput) -> Result<AppStateDto, String>;

#[tauri::command]
async fn pull_from_peer(input: PullInput) -> Result<TransferDto, String>; // advanced / compatibility

#[tauri::command]
async fn update_settings(input: SettingsInput) -> Result<AppStateDto, String>;

#[tauri::command]
async fn open_inbox(...) -> Result<(), String>;

#[tauri::command]
async fn reveal_path(path: String) -> Result<(), String>;

#[tauri::command]
async fn copy_local_address(...) -> Result<String, String>;
```

后续 commands：

```rust
cancel_transfer(id)
retry_transfer(id)
clear_transfer_history()
test_manual_connection(addr)
export_diagnostics()
```

### 9.4 Tauri events

Rust 后端向前端推送：

```text
mftx://peer/status
mftx://peer/discovered
mftx://peer/expired
mftx://transfer/incoming-requested
mftx://transfer/incoming-expired
mftx://transfer/started
mftx://transfer/progress
mftx://transfer/finished
mftx://transfer/failed
mftx://config/changed
mftx://diagnostics/error
```

事件 payload 应可序列化为 JSON，避免直接暴露 core 内部类型。

示例：

```json
{
  "id": "0d1e...",
  "direction": "push",
  "peer": "Haven-Mac",
  "file": "a.zip",
  "written": 73400320,
  "total": 209715200,
  "bytesPerSecond": 48123904
}
```

### 9.5 前端状态

推荐：

- React + TypeScript + Vite。
- `zustand` 管全局状态。
- `@tauri-apps/api/core` 调 command。
- `@tauri-apps/api/event` 订阅事件。
- `@heroui-pro/react` 和 `@heroui/react` 做 UI。
- `@gravity-ui/icons` 做主要图标。
- Tailwind v4 + HeroUI styles 做样式。

状态分层：

```text
appStore
  ├─ setupComplete
  ├─ config
  ├─ peerStatus
  ├─ nearbyPeers
  ├─ selectedPeerId
  ├─ pendingPaths
  ├─ transfers
  └─ diagnostics
```

### 9.6 HeroUI Pro 组件落地顺序

实施时先从模板和文档落地这些组合：

1. `DesktopShell`
   - 参考 `react/template-dashboard/src/components/app-shell.tsx`。
   - 使用 `AppLayout`、`Sidebar`、`Navbar`。
   - 将 Next.js `router.push` 换成本地 view state 或 React Router/TanStack Router。

2. `DesktopSidebar`
   - 参考 `dashboard-sidebar.tsx`。
   - nav items 改成 `Transfer`、`Inbox`、`Settings`、`Diagnostics`。
   - footer 放 `Help`、`Quit` 或诊断入口。

3. `DesktopNavbar`
   - 参考 `dashboard-navbar.tsx`。
   - 标题显示当前 view。
   - 右侧显示 `Online/Offline` chip、`Receiving` switch、`Open Inbox`、`Copy Address`。

4. `PeerList`
   - 用 `ListView` 显示附近设备。
   - 每行显示设备名、地址、last seen、capabilities、在线状态点。

5. `SendDropZone`
   - 用 `DropZone`，但语义改为发送文件，不是上传到云。
   - 支持多文件和文件夹路径；Tauri 环境中要验证拖拽事件能拿到真实 path。

6. `TransferQueue`
   - 少量传输用 `ListView`。
   - 需要排序、筛选、历史记录时升级到 `DataGrid`。

7. `EmptyViews`
   - 使用 `EmptyState` 显示无设备、无传输、无收件记录。

8. `SettingsView`
   - 使用 HeroUI 表单组件，不要自写表单控件。

### 9.7 复用 core 的要求

不要在 GUI 后端推翻现有传输协议；需要在 core/protocol 层扩展 offer/accept/trust，而不是只在 Tauri 后端模拟。

必须复用：

- `mft_core::app_config::AppConfig`
- `mft_core::peer::PeerNode`
- `mft_core::peer::initiator::{push_paths, pull_all}` 作为兼容层
- `mft_core::discovery::discover_for`
- `mft_protocol::crypto::PasswordRecord` 作为 legacy CLI / 兼容层

如果现有 core 不够支持 GUI 进度，优先在 core 增加 progress callback/channel，而不是让 Tauri 后端轮询文件大小。

---

## 10. Core 层需要补强

当前 `ProgressEvent` 已存在，但实际 transfer facade 仍主要返回最终 `TransferReport`。GUI 需要事件流。

建议新增：

```rust
pub trait ProgressSink: Send + Sync + 'static {
    fn emit(&self, event: ProgressEvent);
}
```

或者使用 channel：

```rust
pub type ProgressSender = tokio::sync::mpsc::UnboundedSender<ProgressEvent>;
```

新增 API：

```rust
pub async fn offer_paths_with_progress(
    peer: TrustedPeerRef,
    paths: &[PathBuf],
    progress: ProgressSender,
) -> anyhow::Result<TransferReport>;

pub async fn respond_incoming_transfer(
    request_id: Uuid,
    decision: IncomingDecision,
) -> anyhow::Result<()>;
```

兼容要求：

- 保留现有 `push_paths`、`pull_all`、`upload_paths`、`download_all`。
- 旧 API 只作为 CLI/兼容 facade；GUI 主流程不能继续要求用户输入共享密码。
- 现有 CLI 行为在迁移期不能回退，但新 GUI 语义以 offer/accept 为准。

---

## 11. 安全与权限

### 11.1 设备信任

- 每台设备必须有持久 `device_id`，最好升级为设备公钥指纹。
- 信任列表以 `device_id` / 公钥指纹为键，不以 IP、端口、设备名或 session id 为键。
- 陌生设备发送文件时默认询问。
- 勾选 `信任此设备` 后，下次该设备发送文件可自动接收。
- 信任列表需要可在设置页查看和移除。

### 11.1.1 密码与远程中继

- LAN 直连主流程不显示传输密码。
- 现有 `PasswordRecord` 可暂时保留给 legacy CLI 和兼容命令。
- 如果未来引入 OSS/对象存储中继，优先使用登录态、设备身份、短期 STS/预签名 URL、一次性 transfer token。
- 密码只适合作为可选的端到端加密口令或邀请口令，不作为对象存储上传/下载凭证。

### 11.2 文件访问

- 传输目标只能写入配置的 inbox 或用户选择的 pull 输出目录。
- 不允许前端传入任意路径绕过 core 的 `clean_relative_path`。
- 打开文件/目录必须通过受控 command。

### 11.3 Tauri permissions

首版只开启必要插件和权限：

- dialog：选择文件/目录。
- fs：必要时限制到用户选择路径。
- notification：完成/失败通知。
- autostart：设置页开关。
- opener/shell：打开 inbox、reveal path。
- single-instance：避免多个 peer 抢端口。

---

## 12. 平台差异

### 12.1 Windows

注意点：

- Windows 防火墙可能拦截入站 TCP。
- 首次监听失败时要提示用户检查防火墙。
- 托盘是主要常驻入口。
- 默认 inbox 使用 Downloads 下的 `MFTX Inbox`。
- 打包产物应包含 `.msi` 或 `.exe` installer。

### 12.2 macOS

注意点：

- 菜单栏/托盘图标应使用 template icon。
- 关闭主窗口后应用继续运行。
- 默认 inbox 使用 `~/Downloads/MFTX Inbox`。
- 后续分发需要考虑签名、公证。

---

## 13. 测试计划

### 13.1 Core 测试

必须保持：

```powershell
cargo test --workspace
```

新增：

- progress event 顺序。
- push progress。
- pull progress。
- transfer failed event。
- GUI runtime 启停 PeerNode。
- setup config roundtrip。

### 13.2 Tauri 后端测试

建议：

```powershell
cargo test -p mft-desktop
```

覆盖：

- DTO serialization。
- setup input validation。
- command 错误映射。
- manual address parse。
- runtime start/stop。

### 13.3 前端测试

建议：

```powershell
npm test
npm run typecheck
npm run lint
```

覆盖：

- store reducer。
- transfer event merge。
- setup wizard validation。
- device selection。

### 13.4 视觉验证

使用 Playwright 或 Tauri web preview 截图验证：

- 1200x760 主窗口。
- 900x640 小窗口。
- Windows 默认字体。
- macOS 默认字体。
- 长设备名、长文件名不溢出。
- 传输队列多条记录不挤压。

### 13.5 本机 e2e

至少覆盖：

1. GUI 后端启动 peer。
2. CLI peer 与 GUI 互相发现。
3. GUI send 到 CLI/GUI peer。
4. GUI 收到陌生设备 incoming offer 时显示接收确认。
5. 勾选信任设备后，同一 `device_id` 下次发送自动接收。
6. 被拒绝的 incoming offer 不写入 inbox。

### 13.6 跨机器验收

Windows 和 Mac 各运行一个 GUI 或 GUI/CLI 混合：

1. 双方在线。
2. 双方互相 discover。
3. Mac GUI 发文件到 Windows。
4. Windows GUI 发文件到 Mac。
5. 陌生设备首次发送需要接收确认。
6. 信任设备后再次发送自动接收。
7. 手动地址兜底成功。
8. 关闭窗口后仍能接收。

---

## 14. 实施阶段

### Phase 0：脚手架

输出：

- `apps/desktop` Tauri v2 项目。
- React/Vite/TypeScript。
- HeroUI Pro、HeroUI、Tailwind v4、`@gravity-ui/icons` 依赖。
- `hpsetup` 安装 Pro 依赖，或明确记录因缺少 HP Key/凭据而未执行。
- 全局样式引入 `@heroui/styles` / `@heroui-pro/react/css`。
- `src-tauri` 作为 Rust workspace member。
- 使用 `AppLayout`、`Sidebar`、`Navbar` 搭出基础窗口。
- 基础托盘。

验收：

```powershell
cd apps\desktop
npm install
npx -y hpsetup@latest <HP_KEY>
npm run tauri dev
```

### Phase 1：配置和首次启动

输出：

- Setup wizard。
- 读取/写入 `AppConfig`。
- 设备名、监听端口、inbox、share dir。
- 设置页基础表单。

验收：

- 首次启动能完成配置。
- 重启后能读取配置。

### Phase 2：Peer runtime

输出：

- `start_peer` / `stop_peer` command。
- 托盘状态。
- 主窗口在线状态。
- 关闭窗口后保持后台运行。

验收：

- GUI peer 可被 CLI discover。
- 端口占用时有错误提示。

### Phase 3：发现设备

输出：

- `discover_peers` command。
- HeroUI Pro `ListView` 设备列表。
- 手动地址输入。
- peer expired 状态。

验收：

- CLI peer 出现在 GUI 设备列表。
- 同名设备显示 session/address 辅助信息。

### Phase 4：发送和接收

输出：

- HeroUI Pro `DropZone` 拖拽文件/文件夹。
- 选择目标并发送。
- 入站接收显示。
- 完成通知。
- 打开 inbox。

验收：

- GUI -> CLI push 成功。
- CLI -> GUI push 成功。

### Phase 5：进度事件

输出：

- core progress API。
- Tauri event bridge。
- HeroUI Pro `ListView` 或 `DataGrid` 传输队列。
- 速度、进度、完成、失败状态。
- 取消/重试可作为后续增强。

验收：

- 大文件传输能持续更新进度。
- 失败时前端显示具体错误。

### Phase 6：高级 Pull / 兼容

输出：

- `Pull all` 仅作为高级操作。
- 选择输出目录。
- pull resume 状态展示。

验收：

- 高级入口 GUI pull CLI share 成功。
- `.part` resume 不回退。

### Phase 7：打包和发布准备

输出：

- Windows installer。
- macOS app bundle。
- app icon。
- updater 可先预留，不强制开启。

验收：

```powershell
npm run tauri build
```

---

## 15. MVP 验收标准

v0.1 MVP 必须满足：

- `cargo test --workspace` 通过。
- `apps/desktop` 可以 `tauri dev` 启动。
- HeroUI Pro 依赖和样式能正常编译。
- 首次启动可以创建配置。
- GUI 可以启动常驻 peer。
- GUI 可以发现 CLI peer。
- GUI 可以拖拽发送文件到 CLI peer。
- CLI 可以发送文件到 GUI peer。
- GUI 显示传输进度和结果。
- GUI 可以打开 inbox。
- 关闭窗口后 peer 继续运行，托盘可恢复窗口。
- Windows 和 macOS 至少一个平台完成真实打包验证。

---

## 16. 后续增强

v0.2 以后再做：

- 远程 manifest 浏览器，选择性 pull。
- 接收前确认。
- 覆盖策略 UI。
- 传输取消和断点续传增强。
- 多设备群发。
- 速度限制。
- 更完整的日志窗口。
- 自动更新。
- macOS 签名和公证。
- Windows 防火墙规则引导。

---

## 17. 设计结论

MFTX GUI 的最佳第一版不是“重做一个复杂客户端”，而是把已经存在的 Rust P2P 能力包装成一个安静、可靠、常驻的桌面工具：

- Tauri 负责窗口、托盘、通知、系统集成。
- React 负责清楚的文件传输交互。
- `mft_core` 继续负责 discovery、加密、manifest、push/pull。
- GUI 的价值是降低使用门槛，而不是改变协议。

只要第一版把“启动在线、发现设备、拖拽发送、接收提示、打开收件箱”做顺，这个软件就已经从命令行工具变成真正可日常使用的桌面应用。
