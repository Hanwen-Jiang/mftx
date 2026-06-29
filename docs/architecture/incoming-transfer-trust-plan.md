# MFTX Incoming Transfer + Trust Protocol 计划

> 文档状态：协议层改造实施前设计  
> 目标版本：MFTX Desktop v0.2  
> 适用仓库：`E:\jhw\mftx-project`  
> 当前基线：`PeerNode`、UDP discovery、password handshake、主动 push/pull、Tauri Desktop GUI 已存在  
> 目标形态：局域网直连主流程改成“主动发送 offer + 被动接收确认 + 按设备信任”

---

## 1. 设计结论

当前实现仍是旧模型：

- `Frame::Hello/HelloAck/Auth/AuthOk` 通过 `PasswordRecord` 派生会话密钥。
- `upload_paths` 在认证后直接发送 `PutFileStart`，接收端立即写入 inbox。
- `send_paths` / `pull_from_peer` Tauri command 都要求前端传 `password`。
- `DiscoveryBeacon.session_id` 是每次启动生成的临时 ID，不能作为“信任此设备”的稳定身份。
- `AcceptPolicy::Ask` 已存在于 `PeerConfig`，但没有接到 transfer responder 和 GUI。

新模型：

1. 每台设备生成并保存持久 `device_id`。
2. discovery、hello、auth、Tauri DTO 都带 `device_id`。
3. 发送方向接收方发起 `TransferOffer`，包含发送设备身份、设备名、manifest 摘要、总文件数和总字节数。
4. 接收方如果已经信任该 `device_id`，自动接受；否则 emit `incoming request` 到 GUI。
5. GUI 展示“接收 / 拒绝 / 信任此设备并接收”。
6. 接收方只有在接受后才允许发送方继续上传文件。
7. 信任对象必须是 `device_id`，未来可升级为设备公钥指纹；不能是 IP、端口、设备名或 session id。
8. 局域网直连 GUI 主流程不显示传输密码。
9. `pull_from_peer` 和 password handshake 保留为 legacy/高级兼容能力，不作为 Desktop 主流程。

---

## 2. 非目标

本阶段不做：

1. 公网中继、OSS 上传下载、账号系统。
2. 端到端公钥加密的完整 PKI 体系。
3. 远程共享目录浏览器。
4. 多人群发。
5. 删除 CLI 旧命令。
6. 一次性迁移所有 legacy password API。

---

## 3. 核心概念

### 3.1 `device_id`

`device_id` 是稳定设备身份：

- 首次初始化配置时生成 UUID v4。
- 保存到 `AppConfig`。
- discovery beacon、handshake、offer、Tauri DTO 都携带它。
- 同一设备重启后 `device_id` 不变。
- 设备名可以改，IP 可以变，端口可以变，`device_id` 不变。

建议字段：

```rust
pub struct AppConfig {
    pub device_id: Uuid,
    pub device_name: String,
    pub listen_addr: SocketAddr,
    ...
}
```

兼容旧配置：

- 反序列化旧 `config.json` 时如果缺少 `device_id`，生成一个新的并回写。
- 不破坏现有 `device_name`、目录、password 配置。

### 3.2 Trust Store

信任设备表单独落盘，避免和主配置膨胀在一起。

推荐文件：

```text
<base_dir>\trusted-devices.json
```

推荐类型：

```rust
pub struct TrustedDevices {
    pub devices: Vec<TrustedDevice>,
}

pub struct TrustedDevice {
    pub device_id: Uuid,
    pub display_name: String,
    pub first_trusted_at_ms: i64,
    pub last_seen_at_ms: Option<i64>,
}
```

规则：

- 以 `device_id` 为唯一键。
- 设备名只作为显示名，可随最后一次 beacon / offer 更新。
- 不保存 IP 作为信任依据。
- 设置页必须能列出和移除信任设备。

### 3.3 Incoming Request

接收方收到 offer 后创建 `IncomingTransferRequest`：

```rust
pub struct IncomingTransferRequest {
    pub request_id: Uuid,
    pub device_id: Uuid,
    pub device_name: String,
    pub peer_addr: SocketAddr,
    pub manifest: Manifest,
    pub files: usize,
    pub bytes: u64,
    pub created_at_ms: i64,
}
```

注意：

- `request_id` 是本次请求 ID，用于 GUI 接收/拒绝。
- `device_id` 才是信任判断 ID。
- request 应有超时，避免 GUI 永久堆积。

---

## 4. 协议设计

### 4.1 Frame 扩展

建议把协议版本升级到 `PROTOCOL_VERSION = 2`，但保留 v1 兼容路径。

新增/修改 frame：

```rust
pub enum Frame {
    Hello {
        device_id: Option<Uuid>,
        device_name: String,
        version: u16,
        client_nonce: [u8; 32],
    },
    HelloAck {
        device_id: Option<Uuid>,
        device_name: String,
        version: u16,
        server_nonce: [u8; 32],
        password_salt_hex: String,
    },
    Auth {
        device_id: Option<Uuid>,
        device_name: String,
    },
    AuthOk {
        manifest: Manifest,
    },
    TransferOffer {
        offer_id: Uuid,
        device_id: Uuid,
        device_name: String,
        manifest: Manifest,
        files: usize,
        bytes: u64,
    },
    TransferDecision {
        offer_id: Uuid,
        accepted: bool,
        message: Option<String>,
    },
    ...
}
```

兼容原则：

- v1 peer 仍走 password handshake 和 `PutFileStart`。
- v2 GUI 主流程优先走 `TransferOffer`。
- 为降低兼容风险，首版建议保留 `password_salt_hex: String` 和现有 password handshake 字段；新 GUI 主流程可以用内部生成/配置的兼容密钥过渡，但不能向用户索要传输密码。
- 过渡期内部兼容密钥只用于加密通道建立，不作为用户授权依据；是否允许写入 inbox 必须由 `TransferDecision` / trust store 决定。

### 4.2 发送流程

```text
发送方 GUI
  -> 选择文件/文件夹、目标 peer
  -> build manifest
  -> TCP connect
  -> v2 handshake with device_id
  -> send TransferOffer
  -> wait TransferDecision
  -> accepted: send PutFileStart/FileChunk/FileDone
  -> rejected: emit failed/cancelled
```

### 4.3 接收流程

```text
接收方 PeerNode
  -> inbound connection
  -> v2 handshake reads sender device_id
  -> receive TransferOffer
  -> if trust_store.contains(device_id): send TransferDecision accepted
  -> else emit incoming-requested to Tauri GUI and wait decision
  -> GUI accepts/rejects
  -> accepted: receive files into inbox
  -> rejected: send TransferDecision rejected and close
```

### 4.4 超时和错误

要求：

- incoming request 等待用户决策时必须有超时。
- 超时向发送方返回 rejected decision，message 可为 `request expired`。
- 用户拒绝时不创建目标文件，不留下 `.part`。
- 接收中失败时清理 `.part`，保留可诊断错误。

---

## 5. Core 改造

### 5.1 配置与身份

新增：

- `AppConfig.device_id`
- `AppConfig::load` 旧配置迁移
- `TrustedDevices` 读写 API
- `TrustedDeviceStore`

建议文件：

```text
crates/core/src/device_id.rs
crates/core/src/trust_store.rs
```

### 5.2 Discovery

`DiscoveryBeacon` 增加 `device_id`：

```rust
pub struct DiscoveryBeacon {
    pub device_id: Uuid,
    pub session_id: Uuid,
    pub device_name: String,
    ...
}
```

规则：

- `device_id` 来自配置。
- `session_id` 仍用于本次进程去重。
- 前端列表以 `device_id` 识别同一设备，以 `session_id` 辅助调试。

### 5.3 PeerConfig / PeerNode

`PeerConfig` 增加：

```rust
pub device_id: Uuid,
pub trust_store: Arc<dyn TrustStore>,
pub incoming_tx: Option<IncomingTransferSender>,
```

或先使用 concrete store，后续再抽 trait。

`AcceptPolicy` 语义：

- `Always`：仅用于测试或 legacy。
- `Reject`：拒绝所有陌生 incoming。
- `Ask`：默认 GUI 策略。

Desktop 默认应为 `Ask`。

### 5.4 Transfer Responder

当前 `receive_upload` 在 `PutFileStart` 后立即写文件。需要在它之前增加 offer decision gate。

推荐拆分：

```rust
async fn handle_transfer_offer(...) -> anyhow::Result<TransferDecision>;
async fn wait_for_incoming_decision(...) -> IncomingDecision;
async fn receive_upload_after_accept(...);
```

不要让 Tauri 后端通过轮询文件夹模拟接收确认；确认必须在 core responder 允许写入前完成。

### 5.5 Transfer Initiator

新增：

```rust
pub async fn offer_paths(
    addr: SocketAddr,
    local_identity: DeviceIdentity,
    paths: &[PathBuf],
) -> anyhow::Result<TransferReport>;
```

兼容：

- `push_paths(addr, password, paths)` 保留。
- Desktop 主流程改用 `offer_paths`。
- 旧 CLI 可暂时继续走 `push_paths`，或新增 `send-v2` 后再切换。

---

## 6. Tauri Desktop 改造

### 6.1 DTO

新增字段：

- `AppConfigDto.deviceId`
- `AppStateDto.trustedDevices`
- `PeerDto.deviceId`

新增类型：

```ts
type IncomingTransferRequest = {
  id: string;
  deviceId: string;
  deviceName: string;
  peerAddr: string;
  files: number;
  bytes: number;
  pathsPreview: string[];
  createdAt: number;
};

type IncomingTransferDecision = {
  id: string;
  accepted: boolean;
  trustDevice: boolean;
};

type TrustedDevice = {
  deviceId: string;
  displayName: string;
  firstTrustedAt: number;
  lastSeenAt: number | null;
};
```

### 6.2 Commands

新增：

```rust
#[tauri::command]
async fn respond_incoming_transfer(input: IncomingTransferDecisionDto) -> Result<(), String>;

#[tauri::command]
async fn list_trusted_devices(...) -> Result<Vec<TrustedDeviceDto>, String>;

#[tauri::command]
async fn untrust_device(device_id: String) -> Result<AppStateDto, String>;
```

调整：

- `send_paths` 不再接收 `password`。
- `pull_from_peer` 保留但移动到 advanced compatibility。
- `complete_setup` 不再要求用户提供 password；如果 legacy password 仍需要，可自动生成随机内部兼容密码，并不在 UI 暴露，也不能代替 offer/accept 授权。

### 6.3 Events

新增事件：

```text
mftx://transfer/incoming-requested
mftx://transfer/incoming-expired
mftx://trust/changed
```

事件必须来自 core responder 的真实状态。

### 6.4 Frontend UI

主传输页：

- 保留发送模式：选择目标设备、添加文件/文件夹、开始发送。
- 删除主流程 `拉取` segment。
- 删除主流程传输密码输入。
- 右侧栏显示待发送内容和发送确认。
- incoming request 可在右侧栏或活动面板中显示：
  - 设备名
  - 文件数量和总大小
  - 路径预览
  - `接收`
  - `拒绝`
  - `信任此设备`

设置页：

- 监听端口。
- 收件箱/共享目录。
- 信任设备列表。
- 移除信任设备。
- legacy password 可隐藏到高级折叠区，默认不显示。

活动页：

- direction 增加 `incoming` / `outgoing` 或保留 push 但 UI 文案改为发送/接收。
- rejected / expired 状态可见。

---

## 7. OSS 中继密码结论

未来使用 OSS/对象存储作为远程中继时，不应复用 LAN 传输密码。

推荐模型：

- 登录态或设备配对建立身份。
- 服务端签发短期 STS / 预签名 URL。
- transfer token 限定一次传输、对象 key、过期时间和大小。
- 可选端到端加密使用设备密钥或用户输入的一次性口令。

密码适用场景：

- 临时分享链接的访问口令。
- 用户不信任 OSS 存储方时的 E2E 加密口令。

密码不适用场景：

- OSS access key。
- LAN 直连主流程。
- 设备信任依据。

---

## 8. 实施阶段

### Phase 1：持久设备身份

输出：

- `AppConfig.device_id`
- 旧配置迁移
- `DeviceIdentity` DTO
- discovery beacon 携带 `device_id`
- peer 去重仍保留 session 去重，同时支持 device 去重

验收：

- 旧配置加载后自动生成 device_id。
- 重启 peer 后 device_id 不变，session_id 改变。
- GUI 设备列表拿到 deviceId。

### Phase 2：Trust Store

输出：

- `trusted-devices.json`
- `TrustedDeviceStore`
- list / trust / untrust API
- tests 覆盖去重、更新显示名、删除

验收：

- 信任同一 device_id 不重复。
- 删除后再次 incoming 会询问。

### Phase 3：TransferOffer 协议

输出：

- Frame 扩展
- v2 initiator offer flow
- v2 responder decision gate
- v1 password flow 兼容

验收：

- 未接受前不写 inbox。
- 接受后正常传输。
- 拒绝后发送方收到可读错误。
- v1 wrong-password 测试仍通过。

### Phase 4：Incoming Request Event Bridge

输出：

- core incoming request channel
- DesktopRuntime 保存 pending incoming
- Tauri events
- `respond_incoming_transfer` command

验收：

- CLI/测试发送 v2 offer 时 GUI 收到 incoming-requested。
- 接受/拒绝能驱动原始 TCP session 继续或退出。
- request 超时有事件。

### Phase 5：Desktop UI 主流程

输出：

- TransferPanel 删除主流程 password 输入。
- TransferPanel 删除主流程 pull segment。
- incoming request UI。
- 信任设备 checkbox / switch。
- Settings trusted devices list。

验收：

- 发送无需输入密码。
- 收到陌生设备 offer 才显示接收确认。
- 信任设备后下次自动接收。
- pull 只在高级/兼容入口出现。

### Phase 6：验证与打包

输出：

- Rust tests
- frontend typecheck/build
- Tauri build
- 文档更新

验收：

```powershell
cargo test --workspace
cd apps\desktop
npm run typecheck
npm run build
npm run tauri:build
```

---

## 9. 测试矩阵

Core tests：

- old config migration creates device_id。
- trust store add/update/remove。
- discovery beacon includes stable device_id。
- v2 transfer offer accept writes inbox。
- v2 transfer offer reject does not write inbox。
- trusted device auto-accepts。
- untrusted device asks。
- request timeout rejects。
- v1 wrong password still fails clearly。

Desktop tests/manual verification：

- setup page only shows listen port, not listen address。
- transfer page no password in primary flow。
- transfer page no primary pull mode。
- incoming request displays device name, file count, size。
- trust device persists across restart。
- settings can untrust device。

---

## 10. Completion Criteria

本计划完成时必须同时满足：

1. 每台设备有持久 `device_id`，重启不变。
2. 信任设备绑定 `device_id`，不是 IP、端口、设备名或 session id。
3. 陌生设备发送文件时，接收端先显示 incoming request。
4. 用户拒绝时不写入 inbox。
5. 用户接受时才开始写入 inbox。
6. 用户信任设备后，该设备后续发送自动接收。
7. Desktop 主流程不再要求传输密码。
8. Desktop 主流程不再展示 pull 作为同级模式。
9. Legacy password push/pull 流程仍可用于 CLI/高级兼容，旧测试不回退。
10. 计划中的验证命令有实际结果。
