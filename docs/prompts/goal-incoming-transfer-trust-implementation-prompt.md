# Goal 模式 Prompt：实现 MFTX Incoming Transfer + Trust Protocol

请在 `E:\jhw\mftx-project` 仓库中继续工作，目标是把 MFTX Desktop 的局域网直连主流程从“共享密码 + 主动 push/pull”改造成“主动发送 offer + 被动接收确认 + 按设备信任”。

## 必读资料

开始前必须阅读：

```text
E:\jhw\mftx-project\docs\architecture\incoming-transfer-trust-plan.md
E:\jhw\mftx-project\docs\architecture\desktop-gui-plan.md
E:\jhw\mftx-project\docs\architecture\peernode-p2p-architecture.md
```

还要检查当前代码：

```text
crates/protocol/src/frame.rs
crates/core/src/app_config.rs
crates/core/src/discovery.rs
crates/core/src/transfer.rs
crates/core/src/peer/config.rs
crates/core/src/peer/node.rs
apps/desktop/src-tauri/src/models.rs
apps/desktop/src-tauri/src/commands.rs
apps/desktop/src-tauri/src/runtime.rs
apps/desktop/src/lib/types.ts
apps/desktop/src/lib/api.ts
apps/desktop/src/components/panels/TransferPanel.tsx
apps/desktop/src/components/panels/SettingsPanel.tsx
```

## 当前问题

当前实现仍然存在这些旧模型痕迹：

- TCP 传输通过 `PasswordRecord` 握手。
- `PutFileStart` 到达后接收端直接写 inbox。
- GUI `send_paths` 和 `pull_from_peer` 请求都带 `password`。
- GUI 仍把 pull 放在主传输流程里。
- discovery 的 `session_id` 是本次进程 ID，不能作为信任设备依据。
- `AcceptPolicy::Ask` 存在但没有接入 responder 和 Tauri GUI。

## 总目标

最终必须做到：

1. 新增持久 `device_id`，并迁移旧配置。
2. discovery beacon、peer DTO、app state 都携带 `device_id`。
3. 新增 trust store，以 `device_id` 为键保存信任设备。
4. 新增 `TransferOffer` / `TransferDecision` 或等价协议帧。
5. 接收端在接受 offer 前不得写入 inbox。
6. 陌生设备发送文件时，Desktop GUI 收到 incoming request 事件。
7. GUI 可以接收、拒绝、信任此设备并接收。
8. 被信任设备再次发送时自动接收。
9. Desktop 主传输流程移除密码输入。
10. Desktop 主传输流程移除 pull 同级模式；pull 只可作为高级/兼容入口保留。
11. legacy password push/pull 仍保留给 CLI/兼容路径，不破坏现有测试。
12. 测试、typecheck、build 有实际结果。

## 产品语义

不要再实现“自动拉取”。正确语义是：

```text
别人向本机发送文件
  -> 本机收到 TransferOffer
  -> 如果设备未信任，显示是否接收
  -> 用户可选择信任此设备
  -> 下次这个 device_id 发送时自动接收
```

信任此设备必须绑定：

- `device_id`，或未来设备公钥指纹。

不得绑定：

- IP。
- 端口。
- 设备名。
- `session_id`。

## 协议要求

实现时可以保留旧 frame variant，但必须增加 v2 offer/decision 能力。

建议新增：

```rust
Frame::TransferOffer {
    offer_id: Uuid,
    device_id: Uuid,
    device_name: String,
    manifest: Manifest,
    files: usize,
    bytes: u64,
}

Frame::TransferDecision {
    offer_id: Uuid,
    accepted: bool,
    message: Option<String>,
}
```

`Hello` / `HelloAck` / `Auth` 应携带 `device_id`，可用 `Option<Uuid>` 兼容 v1。

原则：

- v2 GUI 主流程不向用户索要 password。
- v1 password flow 保留，旧 CLI 和测试继续可用。
- 过渡期如继续使用内部兼容 password 建立加密通道，它不能作为用户授权依据；是否写入 inbox 必须由 `TransferDecision` / trust store 决定。
- 如果升级 `PROTOCOL_VERSION`，必须处理兼容错误和测试。

## Core 实施要求

### Device ID

- `AppConfig` 增加 `device_id: Uuid`。
- 旧配置缺少 `device_id` 时自动生成并保存。
- `PeerConfig` 和 `PeerNode` 持有 `device_id`。
- `DiscoveryBeacon` 携带 `device_id`。

### Trust Store

新增 trust store：

```text
crates/core/src/trust_store.rs
```

建议落盘：

```text
<base_dir>\trusted-devices.json
```

必须支持：

- list。
- trust / upsert。
- untrust。
- contains。
- 以 `device_id` 去重。

### Incoming Decision Gate

在 responder 收到 `TransferOffer` 后：

- 如果 trust store 命中，自动接受。
- 如果未命中，创建 pending request 并通知 GUI。
- 等待 GUI decision。
- 接受后才允许 `PutFileStart`。
- 拒绝或超时不写 inbox。

不要在 Tauri 后端通过观察文件系统模拟接收确认。gate 必须在 core 接收文件前发生。

## Desktop/Tauri 实施要求

### DTO

新增或调整：

- `AppConfigDto.deviceId`
- `PeerDto.deviceId`
- `AppStateDto.trustedDevices`
- `IncomingTransferRequestDto`
- `IncomingTransferDecisionDto`
- `TrustedDeviceDto`

### Commands

新增：

```rust
respond_incoming_transfer(input)
list_trusted_devices()
untrust_device(device_id)
```

调整：

- primary `send_paths` 不再接收 `password`。
- `complete_setup` 不再要求用户输入 password。若 legacy 仍需要内部 password，可自动生成，不在 UI 暴露，也不能代替 offer/accept 授权。
- `pull_from_peer` 保留为 advanced/compatibility，不在主流程。

### Events

新增：

```text
mftx://transfer/incoming-requested
mftx://transfer/incoming-expired
mftx://trust/changed
```

### Frontend

TransferPanel：

- 删除主流程 `password` state 和输入框。
- 删除主流程 pull segment。
- 保留发送：选设备、选文件、发送。
- 增加 incoming request UI。
- incoming request UI 包含接收、拒绝、信任此设备。

SettingsPanel：

- 增加信任设备列表。
- 可移除信任设备。
- 只显示监听端口，不显示监听地址。
- legacy password 如必须保留，放到高级区域，不能作为主流程字段。

Activity：

- 能显示 incoming accepted/rejected/expired。
- 文案使用“发送/接收”，不要强迫用户理解 push/pull。

## HeroUI Pro 要求

如果修改 UI，必须使用 HeroUI/HeroUI Pro 既有组件：

- `@heroui/react`：Button、Tooltip、Badge/Chip、Form/Input 等。
- `@heroui-pro/react`：AppLayout、Sidebar、Navbar、ListView、DropZone、EmptyState、Segment 等。

遵守：

- Button 用 `onPress`。
- 图标按钮有 `aria-label` 和 Tooltip。
- 不做卡片套卡片。
- 不用不存在的旧组件名。
- 表单和列表保持紧凑清晰。

## 实施阶段

按阶段推进，每阶段都要尽量保持测试可运行。

### Phase 1：Device ID

完成：

- config 字段和迁移。
- discovery 携带 device_id。
- DTO 携带 deviceId。
- tests。

验证：

```powershell
cargo test -p mft-core
```

### Phase 2：Trust Store

完成：

- trust store 类型和落盘。
- Tauri list/untrust/trust DTO 基础。
- tests。

验证：

```powershell
cargo test -p mft-core trust
```

### Phase 3：TransferOffer/Decision

完成：

- protocol frame 扩展。
- initiator offer flow。
- responder decision gate。
- legacy v1 path 保留。
- tests。

验证：

```powershell
cargo test -p mft-protocol
cargo test -p mft-core transfer
```

### Phase 4：Incoming Tauri Bridge

完成：

- DesktopRuntime pending incoming。
- incoming-requested / expired events。
- respond_incoming_transfer command。
- trust device option wiring。

验证：

```powershell
cargo check -p mft-desktop
```

### Phase 5：Frontend 主流程

完成：

- TransferPanel 移除主流程密码。
- TransferPanel 移除主流程 pull。
- incoming request UI。
- SettingsPanel trusted devices。
- Activity 文案更新。

验证：

```powershell
cd apps\desktop
npm run typecheck
npm run build
```

### Phase 6：完整验证和打包

完成：

- Rust workspace tests。
- Desktop typecheck/build。
- Tauri build。
- 文档更新。

验证：

```powershell
cd E:\jhw\mftx-project
cargo test --workspace

cd E:\jhw\mftx-project\apps\desktop
npm run typecheck
npm run build
npm run tauri:build
```

## 验收标准

完成后必须满足：

1. 旧配置能迁移出持久 device_id。
2. 重启后 device_id 不变，session_id 可变。
3. discovery peer DTO 有 deviceId。
4. trust store 按 device_id 信任，不按 IP。
5. 陌生设备发送文件时 GUI 显示 incoming request。
6. 用户拒绝时 inbox 不出现文件。
7. 用户接收后文件写入 inbox。
8. 用户选择信任设备后，同一 device_id 后续发送自动接收。
9. TransferPanel 主流程没有传输密码输入。
10. TransferPanel 主流程没有 pull 同级模式。
11. pull/password 兼容路径仍可用于 legacy CLI 或高级入口。
12. `cargo test --workspace`、`npm run typecheck`、`npm run build` 至少有实际结果；无法运行时必须说明具体原因。

## 输出要求

最终用简体中文汇报：

1. 改了哪些协议/core/desktop 文件。
2. 新的 incoming transfer 流程如何工作。
3. trust store 文件位置和信任依据。
4. GUI 主流程删掉了哪些旧字段。
5. legacy push/pull/password 如何保留。
6. 测试和构建结果。
7. 仍未完成的风险。

不要只改 UI。必须让接收确认发生在 core responder 写文件之前。
