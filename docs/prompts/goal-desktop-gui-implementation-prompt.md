# Goal 模式 Prompt：实现 MFTX Desktop GUI 首版

请在 `E:\jhw\mftx-project` 仓库中继续工作，目标是为现有 MFTX Rust P2P 文件传输工具实现一个 Tauri 桌面 GUI 首版。

## 背景

当前仓库是 Rust workspace：

- `crates/protocol`：帧格式、manifest、crypto、path。
- `crates/core`：discovery、app_config、fs_manifest、transfer、peer。
- `crates/mac-cli`：Mac CLI/TUI，命令名 `mft`。
- `crates/win-peer`：Windows CLI，命令名 `mft-win-peer`。

当前 core 已具备：

- `PeerNode` 常驻 peer。
- UDP discovery + probe。
- 手动 `ip:port` 兜底。
- 双向 push/pull。
- pull `.part` 断点续传。
- push 写入 inbox。
- Argon2id + ChaCha20Poly1305 加密会话。
- BLAKE3 完整性校验。

产品纠偏：

- 旧 core 有主动 push/pull 和共享密码握手，但桌面 GUI 主流程必须改为“主动发送 + 被动接收”。
- 接收方只有在收到其他设备发送请求时才显示接收确认。
- 接收确认提供 `信任此设备` 选项；信任对象必须是稳定设备身份，例如持久 `device_id` 或设备公钥，不是 IP、端口、设备名或本次 `session_id`。
- 局域网直连主流程不要求用户输入传输密码；现有 `PasswordRecord` 只作为 legacy CLI / 兼容层保留。
- `pull_from_peer` 只作为高级/兼容能力，不放在主传输流程。
- 设置页只暴露监听端口，后端固定 bind `0.0.0.0:<port>`。
- 未来 OSS/对象存储中继应使用登录态、设备身份、短期 STS/预签名 URL 或一次性 transfer token；密码只适合作为可选的端到端加密口令/邀请口令。

GUI 详细计划文档已经写好，请先阅读并以它为准：

```text
E:\jhw\mftx-project\docs\architecture\desktop-gui-plan.md
```

同时阅读现有 P2P 架构文档，避免破坏已完成的 CLI/core 设计：

```text
E:\jhw\mftx-project\docs\architecture\peernode-p2p-architecture.md
```

## 总目标

实现一个可日常使用的 MFTX Desktop v0.1：

1. 新增 Tauri v2 桌面应用。
2. 使用 React/Vite/TypeScript + HeroUI Pro 实现前端。
3. Rust Tauri 后端复用现有 `mft_core`。
4. 首次启动可完成设备名、监听端口、收件箱、共享目录配置。
5. GUI 可以启动/停止常驻 `PeerNode`。
6. GUI 可以发现附近设备。
7. GUI 可以拖拽文件/文件夹发送到选中设备。
8. GUI 收到陌生设备发送请求时显示接收/拒绝确认。
9. GUI 支持信任设备，并对同一 `device_id` 后续发送自动接收。
10. GUI 显示传输进度、完成、失败。
11. GUI 可以打开 inbox。
12. 支持系统托盘/菜单栏常驻。
13. 关闭主窗口后 peer 继续运行。
14. 不破坏现有 CLI、core API 和测试。

## 推荐技术栈

使用：

```text
Tauri v2
React
Vite
TypeScript
HeroUI Pro
@heroui-pro/react
@heroui/react
@heroui/styles
@gravity-ui/icons
zustand
Tailwind v4
```

必须使用 HeroUI Pro 作为前端组件系统，不要退回自写 plain CSS 组件。界面应是安静、清楚、工具型，不做 landing page、不做营销首页、不做大面积装饰背景。

本 prompt 是实施代理的执行规范，不只是方案说明。开始写代码前必须先确认当前工作树状态、阅读下方资料源，并在最终汇报里说明哪些资料源实际可访问、哪些因凭据/MCP/本机环境不可访问。

## HeroUI Pro 资料源

必须结合这些资料工作：

1. 组件文档仓库：

```text
https://github.com/Hanwen-Jiang/hp-component-docs
```

优先读取 `react/*.md`，尤其是：

```text
react/app-layout.md
react/sidebar.md
react/navbar.md
react/drop-zone.md
react/list-view.md
react/data-grid.md
react/empty-state.md
react/segment.md
react/sheet.md
```

2. 模板仓库：

```text
https://github.com/Hanwen-Jiang/heroui-v3-templates
```

重点参考：

```text
react/template-dashboard
react/template-email
```

`template-dashboard` 是 Next.js 16 + HeroUI Pro 模板。MFTX Desktop 仍使用 Vite/Tauri，不要照搬 Next.js App Router；只迁移 shell、sidebar、navbar、settings、表格/list 的组件组合和视觉密度。

3. 本地 Figma kit：

```text
E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Figma Kit V3-v1.0.4.fig
E:\figma\HeroUI Pro Figma Kit V3-v1.0.4.fig\HeroUI Pro Native Figma Kit-v1.0.2.fig
```

MFTX Desktop 是 Web/Tauri GUI，优先使用 Web kit；Native kit 只作未来移动端参考。

## HeroUI Pro 使用方法

在 `apps/desktop` 基础 Vite/Tauri 项目创建后，安装普通依赖并运行 `hpsetup` 安装 Pro 依赖：

```powershell
cd E:\jhw\mftx-project\apps\desktop
npm install
npx -y hpsetup@latest <HP_KEY>
```

或使用 pnpm：

```powershell
cd E:\jhw\mftx-project\apps\desktop
pnpm install
pnpm dlx hpsetup@latest <HP_KEY>
```

注意：

- `hpsetup` 使用 HP Key。
- MCP 和 Skills 使用 Personal Token。
- 两者不是同一种凭据，不要混用。
- 凭据不要写入仓库、脚本或文档。
- 如 `collectui.pro` 被安全浏览拦截，使用备用域名 `collectui.vip` 查看文档/配置凭据。
- 如果 pnpm 报 `ERR_PNPM_IGNORED_BUILDS`，运行 `pnpm approve-builds`。

全局 CSS 至少引入：

```css
@import "tailwindcss";
@import "@heroui/styles";
@import "@heroui-pro/react/css";
```

也可参考 `template-dashboard`：

```css
@import "@heroui/styles/css";
@import "@heroui-pro/react/css";

@source "../**/*.{ts,tsx}";
```

最终以 `npm run build` / `npm run tauri dev` 能通过为准。

## 现有项目使用方法和 CLI 验收命令

实现 GUI 时要持续用现有 CLI/core 做对照，确保 GUI 只是包装现有能力，不改变协议语义。

基础验证：

```powershell
cd E:\jhw\mftx-project
cargo test --workspace
cargo build --release -p mft-win-peer
```

源码内两个 CLI 包：

```text
cargo run -p mft -- <command>
cargo run -p mft-win-peer -- <command>
```

常用命令：

```powershell
# Windows 端启动常驻 peer，收到的文件落到 E:\jhw
New-Item -ItemType Directory -Force E:\jhw
$env:MFTX_PASSWORD = "test123"
cargo run -p mft-win-peer -- peer --name Lou-Win --listen 0.0.0.0:48151 --inbox E:\jhw

# 发现局域网 peer
cargo run -p mft-win-peer -- discover --seconds 5

# 从 Windows CLI 发送文件到名为 Haven-Mac 的 peer
cargo run -p mft-win-peer -- send --to Haven-Mac .\example.txt

# 从 Windows CLI 拉取对方 share 到本地目录
cargo run -p mft-win-peer -- pull --from Haven-Mac --out .\received

# 发现失败时使用手动地址
cargo run -p mft-win-peer -- send --connect 192.168.1.10:48151 .\example.txt
```

Mac/Linux shell 形态：

```bash
MFTX_PASSWORD=test123 cargo run -p mft -- peer --name Haven-Mac --listen 0.0.0.0:48151 --inbox /tmp/mft-inbox /tmp/share
MFTX_PASSWORD=test123 cargo run -p mft -- discover --seconds 5
MFTX_PASSWORD=test123 cargo run -p mft -- send --to Lou-Win /tmp/example.txt
MFTX_PASSWORD=test123 cargo run -p mft -- pull --from Lou-Win --out /tmp/received
MFTX_PASSWORD=test123 cargo run -p mft -- send --connect 192.168.1.20:48151 /tmp/example.txt
```

如使用接手文档里已经部署到 PATH 的命令，可以把源码命令替换为：

```powershell
mft-win-peer.exe peer --name Lou-Win --listen 0.0.0.0:48151 --password test123 --inbox E:\jhw
```

```bash
MFTX_PASSWORD=test123 mftx discover --seconds 5
MFTX_PASSWORD=test123 mftx send --to Lou-Win /tmp/example.txt
MFTX_PASSWORD=test123 mftx send --connect 192.168.1.20:48151 /tmp/example.txt
```

GUI 的 e2e 验收必须覆盖 GUI 与这些 CLI 命令互通：CLI 能发现 GUI、GUI 能发现 CLI、GUI 能发给 CLI、CLI/GUI 发给 GUI 时触发 incoming request、信任同一设备后可自动接收；pull 只作为高级兼容入口验收。

## Skills 和 MCP 要求

实现前先检查是否有 HeroUI Pro skills / MCP：

- 如果有 `heroui-react-pro` skill，必须使用它。
- 如果有 `heroui-pro-design-taste` skill，必须使用它校准视觉。
- 如果有 HeroUI MCP，优先用它查询 `list_components`、`get_component_docs`、`get_css`、`get_theme_variables`。
- 如果当前会话没有暴露这些 MCP/skills，使用 `hp-component-docs`、`heroui-v3-templates` 和 `E:\figma` 本地文件作为 authoritative fallback，并在汇报里说明。
- 如果有 Figma MCP，优先读取本地 Figma kit 对应 frame、component、tokens 和截图；没有 Figma MCP 时，说明只能用本地 `.fig` 文件和截图/人工核对流程。

当前已知 fallback 资料：

- `hp-component-docs/react` 中已确认存在 `app-layout.md`、`sidebar.md`、`navbar.md`、`drop-zone.md`、`list-view.md`、`data-grid.md`、`empty-state.md`、`segment.md`、`sheet.md`。
- `heroui-v3-templates/react` 中已确认存在 `template-dashboard`、`template-email`、`template-chat`、`template-finances`。
- `template-dashboard/package.json` 使用 `@heroui-pro/react`、`@heroui/react@3.0.5`、`@heroui/styles@3.0.5`、Tailwind v4、React 19，并以 `@gravity-ui/icons` 作为模板图标库。

## HeroUI v3 硬规则

实现时必须遵守：

- Tailwind CSS v4；不要使用 Tailwind v3 写法。
- 不需要 `<HeroUIProvider>`。
- 复合组件使用点语法，例如 `Sheet.Content`、`Card.Header`、`Sidebar.MenuItem`。
- `Button` 交互使用 `onPress`，不要用 `onClick`。
- 基础组件从 `@heroui/react` 导入，Pro 组件从 `@heroui-pro/react` 导入。
- 不要使用不存在或旧版名称：`Divider`、`SelectItem`、`Progress`、直接导入的 `CardHeader` / `CardContent` / `CardFooter`。
- 分隔线使用 `Separator`；进度展示如需组件，先通过 MCP/文档确认 `ProgressBar` 或对应 Pro 组件是否存在。
- `Switch` 使用 v3 点语法 anatomy：`Switch.Content > Switch.Control > Switch.Thumb`。
- 颜色使用 HeroUI token，例如 `bg-background`、`bg-surface`、`text-foreground`、`text-muted`、`text-success`、`text-warning`、`text-danger`；不要使用 v2 的 `default-100` / `primary-500` 这类编号 token。
- 图标体系保持单一。默认沿用 `template-dashboard` 的 `@gravity-ui/icons`；如果当前 HeroUI MCP/文档明确要求 `@iconify/react` + gravity-ui 图标集，则以 MCP/文档为准并在汇报中说明。不要引入 lucide 或多套图标库混用。
- 图标按钮必须有 `aria-label` 和 `Tooltip`。
- 不要给 HeroUI `Card` 叠加额外阴影；不要卡片套卡片。
- 可滚动列表必须有明确的 `max-height` / flex 约束和 overflow，不要让长列表撑坏窗口。

## 推荐目录结构

新增：

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

把 `apps/desktop/src-tauri` 加入 root `Cargo.toml` workspace members，包名可用 `mft-desktop`。

## Tauri v2 桌面能力要求

首版需要这些桌面能力：

- dialog：选择文件、文件夹、输出目录。
- notification：传输完成/失败通知。
- opener/shell：打开 inbox、reveal path、复制诊断路径。
- autostart：设置页开机启动。
- single-instance：避免多个 GUI 实例抢端口。
- tray：关闭窗口后常驻、显示状态和常用动作。

实现原则：

- `tauri.conf.json` / capabilities 只开放必要权限。
- 前端不要直接获得无限文件系统权限；文件/目录访问尽量通过受控 command 或用户选择结果。
- 如果需要 JS 插件包，在 `apps/desktop/package.json` 显式加入对应 `@tauri-apps/plugin-*` 依赖；Rust 侧在 `src-tauri/Cargo.toml` 加入匹配插件 crate。
- 所有 Tauri command 返回可序列化 DTO；不要让前端依赖 Rust core 内部类型。

## 实施原则

- 不要推翻现有传输协议；允许在其上扩展 `TransferOffer`、incoming decision、trusted device 等协议/事件能力。
- 不要把 GUI 逻辑塞进 `mac-cli` 或 `win-peer`。
- 不要删除或破坏现有 CLI 命令。
- 不要自写 HeroUI Pro 已提供的基础组件。
- 从 `@heroui-pro/react` 和 `@heroui/react` 直接导入组件，不做无意义 barrel wrapper。
- 默认使用 `@gravity-ui/icons` 作为主要图标来源；如果 MCP/组件文档给出更新图标方案，以 MCP/组件文档为准并保持单一图标体系。
- 优先复用：
  - `mft_core::app_config::AppConfig`
  - `mft_core::peer::PeerNode`
  - `mft_core::peer::initiator::{push_paths, pull_all}` 作为兼容层
  - `mft_core::discovery::discover_for`
  - `mft_protocol::crypto::PasswordRecord` 作为 legacy CLI / 兼容层
- 如果 GUI 需要进度事件，优先在 core 增加 progress API，并让旧 API 继续可用。
- 所有 Tauri command 返回 DTO，不要把复杂 core 内部类型直接暴露给前端。
- 错误信息要可读、可行动。

## HeroUI Pro 组件映射

必须优先使用：

| MFTX UI | 组件 |
|---|---|
| 桌面 shell | `AppLayout` + `Sidebar` + `Navbar` |
| 顶部状态栏 | `Navbar` + `Chip` + `Button` + `Tooltip` |
| 附近设备列表 | `ListView` |
| 拖拽发送区 | `DropZone` |
| 传输队列 | `ListView`；需要排序/历史筛选时用 `DataGrid` |
| 空状态 | `EmptyState` |
| 视图切换 | `Segment` 或 `Sidebar.MenuItem` |
| 设置表单 | `Form`、`TextField`、`Select`、`Switch`、`Button` |
| 错误/诊断 | `Alert`、`Sheet`、`Tooltip` |

## 产品形态

主窗口第一屏就是工具界面：

```text
顶部状态栏：本机名称、监听端口、在线状态、接收开关
左侧：附近设备列表
右侧：拖拽发送区、待发送文件、incoming request 确认、发送按钮
底部：传输队列
```

一级视图：

1. `Transfer`
   - 默认视图。
   - 设备列表、拖拽发送、传输队列。

2. `Inbox`
   - 最近收到的文件。
   - 打开文件/打开所在目录。

3. `Settings`
   - 设备名、监听端口、inbox、share dir、信任设备、开机启动、接收确认策略。

4. `Diagnostics`
   - 本机地址、监听状态、发现状态、最近错误、复制诊断信息。

托盘菜单：

```text
MFTX: Online
Open MFTX
Open Inbox
Receiving: On
Copy Local Address
Nearby Devices: N
Settings
Quit
```

## Tauri Commands

首版至少实现：

```rust
get_app_state() -> AppStateDto
complete_setup(input: SetupInput) -> AppStateDto
start_peer() -> PeerStatusDto
stop_peer() -> ()
discover_peers(seconds: u64) -> Vec<PeerDto>
send_paths(input: SendPathsInput) -> TransferDto
respond_incoming_transfer(input: IncomingDecisionInput) -> ()
trust_device(input: TrustDeviceInput) -> AppStateDto
untrust_device(input: TrustDeviceInput) -> AppStateDto
pull_from_peer(input: PullInput) -> TransferDto // advanced / compatibility
update_settings(input: SettingsInput) -> AppStateDto
open_inbox() -> ()
reveal_path(path: String) -> ()
copy_local_address() -> String
```

后续可做：

```rust
cancel_transfer(id: String)
retry_transfer(id: String)
clear_transfer_history()
test_manual_connection(addr: String)
export_diagnostics()
```

## Tauri Events

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

事件 payload 使用可序列化 DTO，例如：

```json
{
  "id": "uuid",
  "direction": "push",
  "peer": "Haven-Mac",
  "path": "a.zip",
  "written": 73400320,
  "total": 209715200,
  "bytesPerSecond": 48123904
}
```

## Core 进度 API

如果当前 core 还不能持续上报进度，请新增兼容 API：

```rust
pub type ProgressSender = tokio::sync::mpsc::UnboundedSender<ProgressEvent>;

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

- 现有 `push_paths`、`pull_all` 继续存在。
- 现有 `upload_paths`、`download_all` 继续存在。
- 新 GUI 主流程不能要求用户输入共享密码。
- 旧 API 作为 CLI/兼容 facade 保留。
- `cargo test --workspace` 必须继续通过。

## 推荐实施阶段

### Phase 0：脚手架

完成：

- 创建 `apps/desktop`。
- 初始化 Tauri v2 + React + TypeScript + Vite。
- 安装并配置 HeroUI Pro、HeroUI、Tailwind v4、`@gravity-ui/icons`。
- 使用 `hpsetup` 安装 Pro 依赖，或明确记录当前环境为什么无法运行。
- 引入 `@heroui/styles` / `@heroui-pro/react/css`。
- `src-tauri` 依赖 `mft-core`、`mft-protocol`。
- 使用 HeroUI Pro `AppLayout`、`Sidebar`、`Navbar` 搭出基础窗口。
- 系统托盘能显示并打开窗口。

验证：

```powershell
cd apps\desktop
npm install
npx -y hpsetup@latest <HP_KEY>
npm run tauri dev
```

以及：

```powershell
cargo test --workspace
```

### Phase 1：首次启动和配置

完成：

- Setup wizard。
- 设备名。
- 监听端口。
- inbox 目录。
- share 目录。
- 默认接收确认策略。
- 配置保存和重启读取。

验证：

- 首次启动能完成 setup。
- 删除/指定测试配置目录后可以重新 setup。
- UI 不暴露 `0.0.0.0`，只暴露端口。

### Phase 2：Peer runtime

完成：

- `DesktopState`。
- `PeerRuntime`。
- `start_peer`。
- `stop_peer`。
- 在线状态。
- 关闭窗口不退出应用。

验证：

- GUI peer 可被 CLI `discover` 发现。
- CLI 可连接 GUI peer。
- 端口占用时 UI 有可读错误。
- 用源码命令验证时，至少运行：

```powershell
cargo run -p mft-win-peer -- discover --seconds 5
cargo run -p mft-win-peer -- send --connect <GUI_IP:PORT> .\example.txt
```

### Phase 3：发现设备

完成：

- `discover_peers` command。
- 前端设备列表，使用 HeroUI Pro `ListView`。
- 选中目标设备。
- 手动地址输入。
- peer last seen / capabilities 显示。

验证：

- CLI peer 出现在 GUI。
- 多个 peer 时可以选择目标。
- 找不到 peer 时可使用手动地址。
- 用 CLI 侧启动对照 peer：

```powershell
$env:MFTX_PASSWORD = "test123"
cargo run -p mft-win-peer -- peer --name Lou-Win --listen 0.0.0.0:48151 --inbox E:\jhw
```

### Phase 4：发送和接收

完成：

- 拖拽文件/文件夹，使用 HeroUI Pro `DropZone`。
- 选择文件/文件夹按钮。
- 待发送列表。
- `send_paths` command 或新的 `offer_paths` command。
- 入站 `TransferOffer` 显示。
- 接收 / 拒绝操作。
- `信任此设备` 操作，信任键为稳定 `device_id` / 公钥指纹，不是 IP。
- 完成后 notification。
- 打开 inbox。

验证：

- GUI -> CLI push 成功。
- CLI/GUI -> GUI 首次发送时显示接收确认。
- 信任同一设备后再次发送自动接收。
- 拒绝发送请求时不写入 inbox。
- 文件夹传输成功。
- 中文/空格路径成功。
- 兼容 CLI -> GUI push 使用：

```powershell
$env:MFTX_PASSWORD = "test123"
cargo run -p mft-win-peer -- send --connect <GUI_IP:PORT> .\example.txt
```

### Phase 5：传输进度

完成：

- core progress API。
- Tauri event bridge。
- 前端 transfer queue，优先使用 HeroUI Pro `ListView`，需要表格能力时使用 `DataGrid`。
- 速度、百分比、完成、失败。

验证：

- 大文件有持续进度。
- 被拒绝/未受信任设备显示可行动错误。
- 连接失败显示地址/防火墙提示。

### Phase 6：高级 Pull / 兼容

完成：

- `pull_from_peer` command 仅作为高级入口。
- `Pull all` 操作不放在主流程。
- 选择输出目录。
- pull resume 进度展示。

验证：

- 高级入口 GUI pull CLI share 成功。
- `.part` 续传仍可用。
- CLI pull 对照命令：

```powershell
$env:MFTX_PASSWORD = "test123"
cargo run -p mft-win-peer -- pull --connect <GUI_IP:PORT> --out .\received
```

### Phase 7：打包

完成：

- `npm run tauri build`。
- Windows 构建产物。
- macOS app bundle 如当前环境可用则验证。
- app icon 可先使用简洁临时图标，但不要用空白图标。

验证：

```powershell
cd apps\desktop
npm run tauri build
```

## 测试要求

每个阶段后至少运行相关测试。

最终必须通过：

```powershell
cargo test --workspace
```

如果前端工具链可用，也必须通过：

```powershell
cd apps\desktop
npx -y hpsetup@latest <HP_KEY>
npm run typecheck
npm run lint
npm run build
```

如果 Tauri dev/build 可用：

```powershell
npm run tauri dev
npm run tauri build
```

## 视觉验收

必须实际查看界面，不要只看编译。

检查：

- 1200x760 窗口。
- 900x640 小窗口。
- 长文件名不溢出。
- 长设备名不挤坏布局。
- 没有 UI 文字重叠。
- 传输队列多条记录仍能扫描。
- 按钮文字不被挤压。
- 拖拽区域明显但不过度装饰。
- HeroUI Pro Figma Kit V3 的 spacing、radius、surface、foreground/muted/accent token 大体一致。
- `AppLayout`、`Sidebar`、`Navbar`、`DropZone`、`ListView`、`EmptyState` 不应被自写 CSS 复刻。

如能使用 Playwright 或浏览器截图，请截图验证。

如果使用 Browser skill / in-app browser：

- 打开 Tauri/Vite dev 地址或 Tauri 窗口可访问页面。
- 至少保存或查看 1200x760、900x640 两个尺寸截图。
- 检查拖拽区、传输队列、设置页、空状态、长文本样例。
- 最终汇报里说明是否实际看过界面，以及发现/修复了哪些视觉问题。

## 安全要求

- LAN GUI 主流程不要求传输密码。
- 设备信任必须绑定稳定 `device_id` / 公钥指纹，不绑定 IP、端口、设备名或 session id。
- 信任设备列表必须可查看、可移除。
- 现有 `PasswordRecord` 只用于 legacy CLI / 兼容路径，不写入前端 localStorage。
- 文件写入必须走 core 的路径安全逻辑。
- 不允许前端构造危险相对路径绕过 `clean_relative_path`。
- Tauri permissions 只开必要能力。
- 打开文件/目录通过受控 command。

## MVP 验收标准

完成后必须满足：

1. `cargo test --workspace` 通过。
2. `apps/desktop` 可启动 Tauri dev。
3. HeroUI Pro 样式和组件可正常编译。
4. 首次启动可创建配置。
5. GUI 可启动常驻 peer。
6. GUI 可被 CLI discover。
7. GUI 可 discover CLI peer。
8. GUI 可拖拽发送文件到 CLI peer。
9. 陌生设备发送到 GUI 时显示接收/拒绝确认。
10. GUI 可信任某台设备，并对同一 `device_id` 后续发送自动接收。
11. GUI 显示传输进度、完成、失败。
12. GUI 可打开 inbox。
13. 关闭主窗口后应用仍在托盘，peer 不退出。
14. 不破坏 `mft` 和 `mft-win-peer` 现有命令。

## 输出要求

完成后请用简体中文汇报：

1. 新增了哪些目录和文件。
2. HeroUI Pro 依赖如何安装，`hpsetup` 是否成功。
3. 使用了哪些 HeroUI Pro 组件和模板参考。
4. GUI 如何启动。
5. 首次启动如何配置。
6. GUI 和 CLI 如何互传，以及 incoming request / 信任设备如何验证。
7. 测试、typecheck、build 结果。
8. 哪些功能已完成，哪些留到下一版。
9. 如果某个验证因本机环境缺失、HP Key、MCP 或 Figma 访问能力缺失无法运行，请明确说明原因。

不要只交脚手架；至少要做到 GUI 能启动 peer、发现设备、发送文件、显示传输结果。

## 完成前自检

不要在证据不足时声称完成。最终汇报前逐项核对：

- 当前 prompt、GUI 计划文档、README、P2P 架构文档都已阅读。
- `apps/desktop` 已加入 workspace 或明确说明为什么还未加入。
- HeroUI Pro 组件不是自写替代品，且关键组件用法已按 MCP/文档核对。
- HP Key、Personal Token、Figma MCP、HeroUI MCP 的可用性已记录。
- `cargo test --workspace` 有实际结果。
- 前端 `typecheck` / `lint` / `build` 有实际结果，或明确缺失原因。
- GUI 与 CLI 互通有实际命令和结果。
- 视觉检查有截图/人工检查记录。
