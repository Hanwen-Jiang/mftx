# MFT PeerNode P2P 架构像素级设计

> 文档状态：实施前架构设计  
> 目标版本：MFT v0.2 P2P 首版  
> 适用仓库：`/Users/haven/Documents/code/file-transfer`  
> 当前基线：`TransferServer + download_all/upload_paths` 的 server-client 结构  
> 目标形态：`PeerNode + peer 命令 + Windows 主动/被动对等传输`

---

## 1. 背景与目标

当前 MFT 已经具备这些能力：

- Rust workspace：
  - `crates/protocol`：帧格式、manifest、加密握手、路径清理。
  - `crates/core`：发现、manifest 构建、TCP 传输。
  - `crates/mac-cli`：Mac 主命令 `mft`，含 `serve/send/receive/tui/discover`。
  - `crates/win-peer`：Windows 最小 CLI 对端，含 `discover/download/upload`。
- TCP 加密传输：
  - 明文长度前缀帧做握手。
  - 固定密码派生会话密钥。
  - `SessionCipher` 加密控制帧和数据帧。
- 文件能力：
  - manifest 描述文件/目录。
  - 下载支持 `.part` 断点续传。
  - 上传写 `.part`，校验成功后重命名。
  - BLAKE3 完整性校验。
- UDP 广播发现：
  - `DiscoveryBeacon` 包含设备名、版本、端口、session id、capabilities。
  - 不暴露文件名。

但当前架构本质仍是 server-client：

- Mac 端 `mft serve/send/tui` 会启动 `TransferServer`。
- Windows 端 `mft-win-peer download/upload` 连接 Mac 服务。
- Windows 端不能长期作为同等 peer 接收来自 Mac 的主动连接。
- Mac 端和 Windows 端命令语义不对称。
- `TransferServer`、`ServerState`、`server_handshake`、`client_handshake` 命名和调用方向强化了 server-client 心智模型。

本设计目标是把 MFT 改成真正的局域网 P2P peer 程序：

- 每台设备都运行同一种 `PeerNode` runtime。
- 每台设备都可以发现别人，也可以被别人发现。
- 每台设备都可以主动发送，也可以被动接收。
- Mac 和 Windows 共享核心逻辑，CLI 只是薄壳。
- 仍只支持同一局域网，不做公网中继、账号系统、NAT 穿透。
- 首版优先完成可运行的 P2P 架构，协议 wire format 尽量兼容，降低改动风险。

---

## 2. 非目标

v0.2 P2P 首版不做：

1. 公网 P2P 打洞、STUN/TURN、relay。
2. 多方群发和群收。
3. 完整 Windows TUI/托盘 UI。
4. iOS/Android 支持。
5. 默认压缩或内容去重。
6. 上传断点续传。
7. 文件同步、双向目录镜像、冲突自动合并。
8. 多用户账号体系。
9. TLS 证书体系。
10. 蓝牙、AirDrop、SMB、WebDAV 兼容层。

---

## 3. 架构总览

### 3.1 新模型

每个设备运行一个 `PeerNode`：

```text
┌─────────────────────────────┐        UDP broadcast         ┌─────────────────────────────┐
│ Mac: mft peer               │ <--------------------------> │ Windows: mft-win-peer peer  │
│                             │                              │                             │
│ ┌─────────────────────────┐ │        TCP encrypted         │ ┌─────────────────────────┐ │
│ │ PeerNode                │ │ <--------------------------> │ │ PeerNode                │ │
│ │ - listener              │ │                              │ │ - listener              │ │
│ │ - discovery announcer   │ │                              │ │ - discovery announcer   │ │
│ │ - discovery listener    │ │                              │ │ - discovery listener    │ │
│ │ - peer table            │ │                              │ │ - peer table            │ │
│ │ - transfer initiator    │ │                              │ │ - transfer initiator    │ │
│ │ - transfer responder    │ │                              │ │ - transfer responder    │ │
│ └─────────────────────────┘ │                              │ └─────────────────────────┘ │
└─────────────────────────────┘                              └─────────────────────────────┘
```

关键点：

- 程序本身没有固定 server/client 身份。
- 每次 TCP 连接里：
  - 主动拨号者是 `initiator`。
  - 被连接者是 `responder`。
- 任意 peer 都可以启动 listener，任意 peer 都可以 dial 另一个 peer。
- 发现层广播的是 peer 能力，不广播文件名。

### 3.2 分层

```text
CLI/TUI layer
  ├─ crates/mac-cli
  └─ crates/win-peer
        │
        ▼
Core Peer layer
  ├─ PeerNode
  ├─ PeerConfig
  ├─ PeerTable
  ├─ DiscoveryService
  ├─ TransferInitiator
  ├─ TransferResponder
  ├─ TransferSession
  ├─ ProgressEvent
  └─ Error taxonomy
        │
        ▼
Protocol layer
  ├─ Frame
  ├─ Manifest
  ├─ PasswordRecord
  ├─ SessionCipher
  ├─ path normalization
  └─ version/capability negotiation
        │
        ▼
OS/File layer
  ├─ async fs
  ├─ path canonicalization
  ├─ .part temp writes
  ├─ BLAKE3 streaming hash
  └─ permission/mtime compatibility fields
```

---

## 4. 用户体验目标

### 4.1 Mac 常驻 peer

```bash
mft peer \
  --name "Haven-Mac" \
  --listen 0.0.0.0:48151 \
  --inbox "$HOME/Downloads/mft-inbox"
```

行为：

- 启动 TCP listener。
- 周期性广播 discovery beacon。
- 监听 discovery beacon，维护 peer table。
- 接收别人上传的文件到 inbox。
- 输出简洁运行状态：

```text
MFT peer online
name: Haven-Mac
listen: 0.0.0.0:48151
inbox: /Users/haven/Downloads/mft-inbox
peers: 1 discovered
press Ctrl+C to stop
```

### 4.2 Windows 常驻 peer

```powershell
mft-win-peer peer `
  --name "Lou-Win" `
  --listen 0.0.0.0:48151 `
  --inbox C:\Users\Lou\Downloads\mft-inbox
```

行为与 Mac 对称。

### 4.3 发现 peer

Mac：

```bash
mft discover --seconds 5
```

Windows：

```powershell
mft-win-peer discover --seconds 5
```

输出示例：

```text
NAME        ADDRESS             CAPS                 SEEN
Lou-Win     192.168.2.31:48151  receive,push,pull    240ms ago
Haven-Mac   192.168.2.12:48151  receive,push,pull    local
```

### 4.4 主动发送到指定 peer

Mac 发给 Windows：

```bash
mft send --to Lou-Win ~/Desktop/a.zip ~/Pictures/demo
```

Windows 发给 Mac：

```powershell
mft-win-peer send --to Haven-Mac C:\Users\Lou\Desktop\a.zip
```

手动地址兜底：

```bash
mft send --connect 192.168.2.31:48151 ~/Desktop/a.zip
```

### 4.5 从指定 peer 拉取共享内容

首版可以保留 pull/download，但语义改为 peer-to-peer：

```bash
mft pull --from Lou-Win --out ~/Downloads/from-lou
```

Windows：

```powershell
mft-win-peer pull --from Haven-Mac --out C:\Users\Lou\Downloads\from-mac
```

为了兼容现有命令，旧命令继续可用：

```bash
mft receive --connect 192.168.2.31:48151 --dir ~/Downloads/from-peer --password xxx
mft-win-peer download --connect 192.168.2.12:48151 --out C:\tmp\from-mac --password xxx
```

---

## 5. CLI 设计

### 5.1 统一命令集

长期目标：Mac 和 Windows CLI 命令尽量一致。

Mac `mft`：

```text
mft init
mft peer
mft discover
mft send
mft pull
mft tui
mft serve        # legacy alias/compat
mft receive      # legacy compat
```

Windows `mft-win-peer`：

```text
mft-win-peer peer
mft-win-peer discover
mft-win-peer send
mft-win-peer pull
mft-win-peer download   # legacy compat
mft-win-peer upload     # legacy compat
```

### 5.2 `peer` 命令

```text
mft peer [OPTIONS] [SHARE_PATHS]...

Options:
  --name <NAME>             Display name. Default: OS hostname.
  --listen <ADDR>           TCP listen address. Default: 0.0.0.0:48151.
  --discovery-port <PORT>   UDP discovery port. Default: 48150.
  --inbox <DIR>             Directory for incoming files. Required unless config exists.
  --password <PASSWORD>     Password for this process. If absent, read config or prompt.
  --no-discovery            Disable UDP discovery; manual connect only.
  --announce-interval <DUR> Default: 2s.
  --peer-ttl <DUR>          Default: 15s.
  --accept <MODE>           ask|always|reject. Default: ask for TUI, always for CLI peer v0.2.
  --overwrite <MODE>        never|ask|always. Default: never.
  --json                    Emit machine-readable events.
```

`SHARE_PATHS` 表示允许别人 pull 的共享路径；可以为空。为空时仍能接收别人 push。

### 5.3 `send` 命令

```text
mft send [OPTIONS] <PATHS>...

Options:
  --to <PEER_NAME_OR_ID>       Discover by peer name/session id.
  --connect <IP:PORT>          Manual address, bypass peer lookup.
  --password <PASSWORD>        Password.
  --discover-seconds <N>       Discovery wait time. Default: 3.
  --overwrite <MODE>           Receiver overwrite request hint. Default: never.
  --dry-run                    Build manifest and print plan only.
  --json                       Machine-readable progress.
```

约束：

- `--to` 和 `--connect` 至少一个。
- 两者都有时 `--connect` 优先，但显示 warning。
- `PATHS` 可以是文件、目录、空目录、中文/空格路径。
- 符号链接默认不跟随；保留当前路径安全策略。

### 5.4 `pull` 命令

```text
mft pull [OPTIONS]

Options:
  --from <PEER_NAME_OR_ID>
  --connect <IP:PORT>
  --out <DIR>
  --password <PASSWORD>
  --discover-seconds <N>
  --resume / --no-resume       Default: resume.
  --json
```

### 5.5 legacy 命令兼容

为了减少破坏：

- `mft serve` 内部可以调用 `PeerNode::start()`，但打印 deprecated 提示。
- `mft receive` 内部映射为 `pull --connect`。
- `mft-win-peer download` 映射为 `pull`。
- `mft-win-peer upload` 映射为 `send --connect`。

兼容期内测试必须覆盖旧命令仍可用。

---

## 6. Core 模块重构设计

### 6.1 新目录结构

目标结构：

```text
crates/core/src/
  lib.rs
  discovery.rs              # 保留 public facade 或迁移入口
  fs_manifest.rs
  transfer.rs               # legacy facade, 调用 peer/session
  peer/
    mod.rs
    config.rs
    node.rs
    table.rs
    discovery_service.rs
    initiator.rs
    responder.rs
    session.rs
    progress.rs
    errors.rs
    limits.rs
```

迁移策略：

- 不一次性删除 `transfer.rs`。
- 先把可复用的低层函数复制/移动到 `peer/session.rs`。
- `transfer.rs` 变成兼容 facade：
  - `TransferServer` 包装 `PeerNode`。
  - `download_all` 调用 `TransferInitiator::pull_all`。
  - `upload_paths` 调用 `TransferInitiator::push_paths`。
- CLI 逐步改用 `PeerNode`。

### 6.2 `PeerConfig`

```rust
pub struct PeerConfig {
    pub device_name: String,
    pub listen_addr: SocketAddr,
    pub discovery_port: u16,
    pub password: PasswordRecord,
    pub inbox_dir: PathBuf,
    pub share_paths: Vec<PathBuf>,
    pub announce_interval: Duration,
    pub peer_ttl: Duration,
    pub accept_policy: AcceptPolicy,
    pub overwrite_policy: OverwritePolicy,
    pub enable_discovery: bool,
    pub capabilities: PeerCapabilities,
    pub limits: TransferLimits,
}
```

默认值：

```text
device_name       = hostname
listen_addr       = 0.0.0.0:48151
discovery_port    = 48150
announce_interval = 2s
peer_ttl          = 15s
accept_policy     = Always for CLI peer v0.2; Ask for future TUI
overwrite_policy  = Never
enable_discovery  = true
chunk_bytes        = 1 MiB
max_frame_bytes    = 8 MiB
max_concurrent_inbound = 4
```

### 6.3 `PeerCapabilities`

```rust
pub struct PeerCapabilities {
    pub receive_push: bool,
    pub serve_pull: bool,
    pub resume_pull: bool,
    pub encrypted_frames: bool,
    pub blake3: bool,
    pub protocol_version: u16,
}
```

Wire capabilities 继续用字符串，方便兼容：

```text
receive
push
pull
resume-pull
encrypted
blake3
```

含义：

- `receive`：可以接收入站 push。
- `pull`：有共享 manifest，可以被别人 pull。
- `push`：可以主动 push。通常所有 CLI 都有。
- `resume-pull`：支持请求 offset 续传。
- `encrypted`：控制帧和数据帧加密。
- `blake3`：完成后校验 BLAKE3。

### 6.4 `PeerNode`

```rust
pub struct PeerNode {
    addr: SocketAddr,
    config: Arc<PeerConfig>,
    peer_table: Arc<PeerTable>,
    listener_task: JoinHandle<()>,
    discovery_task: Option<JoinHandle<()>>,
    shutdown: ShutdownHandle,
}
```

公开 API：

```rust
impl PeerNode {
    pub async fn bind(config: PeerConfig) -> anyhow::Result<Self>;
    pub fn addr(&self) -> SocketAddr;
    pub fn config(&self) -> &PeerConfig;
    pub fn peer_table(&self) -> Arc<PeerTable>;
    pub async fn shutdown(self) -> anyhow::Result<()>;
}
```

设计原则：

- `bind()` 完成：
  1. 规范化配置。
  2. 创建 inbox。
  3. 构建当前 share manifest。
  4. 绑定 TCP listener。
  5. 启动 inbound responder loop。
  6. 启动 discovery service。
- `Drop` 保留 abort 兜底。
- 正常停止用 `shutdown()`，方便测试清理。

### 6.5 `PeerTable`

职责：维护 discovery 看到的对端。

```rust
pub struct PeerTable {
    peers: RwLock<HashMap<PeerKey, PeerRecord>>,
}

pub struct PeerRecord {
    pub device_name: String,
    pub session_id: Uuid,
    pub addr: SocketAddr,
    pub capabilities: Vec<String>,
    pub version: u16,
    pub first_seen: Instant,
    pub last_seen: Instant,
}
```

`PeerKey`：

```rust
pub enum PeerKey {
    Session(Uuid),
    Name(String),
    Addr(SocketAddr),
}
```

查找规则：

1. 如果 `--connect`，直接用地址。
2. 如果 `--to/--from` 是 UUID，按 session id 找。
3. 否则按 device name 找：
   - 精确匹配优先。
   - 多个同名 peer 时返回错误，要求用户用地址或 session id。
4. 过滤过期 peer：`now - last_seen > peer_ttl`。

### 6.6 `DiscoveryService`

拆分为两个循环：

```text
announce loop:
  every announce_interval:
    send beacon to 255.255.255.255:discovery_port

listen loop:
  bind 0.0.0.0:discovery_port
  recv beacon
  parse
  ignore own session id
  update PeerTable
```

关键改进：

- 当前 `discover_for()` 单独绑定 UDP 48150；如果 `peer` 常驻也绑定 48150，临时 discover 可能端口冲突。
- v0.2 需要支持：
  - 常驻 peer 内部有 discovery listener。
  - 临时 `discover` 命令也可运行。
- 方案：
  - discovery listener 绑定时设置 reuse addr/port 如果 tokio/std 支持有限，可通过 `socket2` 增加依赖。
  - 如果不引入 `socket2`，首版采用 fallback：绑定失败时只做主动 broadcast，然后提示端口被占用，并从已有 peer 进程获取不可行；此方案 UX 较差。
- 推荐：引入 `socket2`，构造 UDP socket：
  - `set_reuse_address(true)`。
  - Unix 尝试 `set_reuse_port(true)`。
  - Windows 使用 `SO_REUSEADDR`。

Beacon wire v1 兼容当前格式：

```json
{
  "magic": "MFT_DISCOVERY_V1",
  "version": 1,
  "device_name": "Lou-Win",
  "port": 48151,
  "session_id": "...",
  "capabilities": ["receive", "push", "pull", "resume-pull", "encrypted", "blake3"]
}
```

v0.2 可新增可选字段但保持 serde 兼容：

```rust
pub struct DiscoveryBeacon {
    pub magic: String,
    pub version: u16,
    pub device_name: String,
    pub port: u16,
    pub session_id: Uuid,
    pub capabilities: Vec<String>,
    pub node_kind: Option<String>,       // "peer"
    pub protocol_min: Option<u16>,
    pub protocol_max: Option<u16>,
    #[serde(skip)]
    pub observed_addr: Option<SocketAddr>,
}
```

---

## 7. Transfer Session 设计

### 7.1 命名模型

把 server/client 心智模型替换成：

```text
Dialer/Acceptor       网络连接方向
Initiator/Responder   本次传输请求方向
Pusher/Receiver       文件 push 方向
Puller/Provider       文件 pull 方向
```

一次会话例子：

- Mac 主动发文件给 Windows：
  - Mac = dialer + initiator + pusher
  - Windows = acceptor + responder + receiver
- Windows 从 Mac 拉文件：
  - Windows = dialer + initiator + puller
  - Mac = acceptor + responder + provider

### 7.2 Wire format v0.2 策略

为降低风险，首版 wire format 可以保留现有 `Frame`：

- `Hello`：initiator hello。
- `HelloAck`：responder hello ack。
- `Auth`：initiator auth。
- `AuthOk { manifest }`：responder 返回自己的共享 manifest。
- `GetFile`：pull 请求。
- `PutFileStart`：push 请求。
- `FileChunk/FileDone`：数据。
- `Ack/Done/Error`：控制。

但代码命名改成：

```rust
acceptor_handshake()
initiator_handshake()
handle_inbound_session()
```

文档中明确：`HelloAck.server_nonce` 在 wire v1 中保留字段名，但内部语义叫 responder nonce。

### 7.3 未来 wire v2 目标

未来可演进：

```rust
Frame::PeerHello { device_name, version_min, version_max, nonce, intent }
Frame::PeerHelloAck { device_name, selected_version, nonce, password_salt_hex, capabilities }
Frame::Auth { device_name }
Frame::AuthOk { capabilities, manifest_summary }
Frame::PushManifest { manifest }
Frame::PullManifestRequest
Frame::PullManifest { manifest }
Frame::RequestFile { path, offset }
Frame::DataChunk { transfer_id, path, offset, data, last }
Frame::FileDone { transfer_id, path, size, blake3_hex }
Frame::TransferAck { transfer_id, path }
Frame::TransferDone { transfer_id }
Frame::Error { code, message }
```

v0.2 不要求完成 wire v2。

---

## 8. Push 流程

### 8.1 Mac push 到 Windows

```text
Mac CLI
  │
  │ mft send --to Lou-Win ~/Desktop/a.zip
  ▼
resolve target
  ├─ if --connect: use address
  └─ else discover_for/PeerTable lookup Lou-Win
  ▼
TransferInitiator::push_paths(addr, password, paths)
  ▼
TCP connect Windows PeerNode listener
  ▼
initiator_handshake
  ├─ send Hello(client_nonce)
  ├─ recv HelloAck(responder_nonce, salt)
  ├─ derive key from password + salt + nonces
  ├─ send encrypted Auth
  └─ recv encrypted AuthOk(responder_manifest)
  ▼
build local manifest(paths)
  ▼
for each entry:
  ├─ send PutFileStart(entry)
  ├─ if directory: wait Ack
  ├─ if file:
  │   ├─ stream FileChunk offsets 0..N
  │   ├─ send FileDone(size, blake3)
  │   └─ wait Ack
  ▼
send Done
```

Windows responder：

```text
PeerNode listener accept
  ▼
acceptor_handshake
  ▼
handle_inbound_session
  ▼
Frame::PutFileStart(entry)
  ├─ clean path
  ├─ check accept policy
  ├─ check overwrite policy
  ├─ create parent dirs
  ├─ write to .part
  ├─ stream hash
  ├─ verify FileDone
  ├─ atomic rename
  └─ Ack(path)
```

### 8.2 Windows push 到 Mac

完全对称：

```powershell
mft-win-peer send --to Haven-Mac C:\Users\Lou\Desktop\a.zip
```

Windows 此时是 initiator/pusher，Mac 是 responder/receiver。

### 8.3 失败处理

常见失败和用户文案：

| 场景 | 错误码 | 文案 |
|---|---|---|
| 没发现 peer | `peer-not-found` | `no peer named Lou-Win discovered; use --connect ip:port` |
| 同名 peer 多个 | `ambiguous-peer` | `multiple peers named Lou-Win; use session id or --connect` |
| 密码错误 | `auth-failed` | `authentication failed; check password on both devices` |
| 对端拒绝接收 | `receive-rejected` | `peer rejected incoming transfer` |
| 目标已存在 | `already-exists` | `destination exists and overwrite policy is never` |
| 路径不安全 | `unsafe-path` | `unsafe relative path rejected` |
| 校验失败 | `integrity-failed` | `integrity check failed; partial file kept/removed according to policy` |
| 连接断开 | `connection-lost` | `connection lost; retry the transfer` |

---

## 9. Pull 流程

### 9.1 Pull manifest

当前 handshake 的 `AuthOk { manifest }` 已经返回 responder 共享 manifest，所以 pull 流程可以复用当前 `download_all`。

```text
Puller connects provider
  ▼
handshake receives provider manifest
  ▼
create dirs
  ▼
for each file:
  ├─ inspect .part size
  ├─ send GetFile(path, offset)
  ├─ append chunks
  ├─ verify FileDone + blake3
  └─ rename .part to final
  ▼
send Done
```

### 9.2 Empty share behavior

如果 provider 没有 `share_paths`：

- manifest 为空。
- `pull` 输出：

```text
peer has no shared files
```

- 不算错误。

---

## 10. 安全设计

### 10.1 密码与认证

沿用现有策略：

- `mft init` 设置固定密码。
- 明文密码不落盘。
- 配置只保存带盐密码哈希/record。
- 每次 TCP 会话用随机 nonce 派生独立 session key。

P2P 变化：

- 每个 PeerNode 使用自己的 `PasswordRecord`。
- 双方要使用相同密码才能通信。
- initiator 根据 responder salt + 双 nonce 派生 key。
- responder 根据本地 record + 双 nonce 派生 key。

### 10.2 Discovery 泄露边界

Discovery 允许暴露：

- device name。
- protocol version。
- TCP port。
- session id。
- capabilities。

Discovery 不允许暴露：

- 文件名。
- 用户目录。
- inbox 路径。
- 密码 salt。
- OS 用户名，除非用户显式把 device name 设成用户名。

### 10.3 路径安全

继续强制：

- manifest entry path 必须是相对路径。
- 拒绝 `..`。
- 拒绝绝对路径。
- 拒绝 Windows drive prefix 逃逸，例如 `C:\...` 进入 manifest。
- 拒绝 UNC 路径。
- 默认不跟随 symlink。
- 写入时必须 `safe_join(inbox, clean_relative_path(path))`。
- 文件完成前写 `.part`。
- 校验成功后原子 rename。

Windows 额外注意：

- 拒绝保留设备名：`CON`, `PRN`, `AUX`, `NUL`, `COM1`..`COM9`, `LPT1`..`LPT9`。
- 拒绝路径段末尾空格或点，避免 Windows 规范化歧义。
- 路径分隔符统一为 `/` 存入 manifest，写入本地时转换。

如果当前 `clean_relative_path` 未覆盖 Windows 特例，v0.2 应补测试并增强。

### 10.4 覆盖策略

默认：`OverwritePolicy::Never`。

接收文件时：

```text
if final exists:
  if overwrite never: Error already-exists
  if overwrite always: write .part then replace final
  if ask: future TUI prompt; CLI peer v0.2 不启用 ask
```

为了避免 `.part` 冲突：

```text
file.txt.part
file.txt.part.1
file.txt.part.<transfer_id>
```

推荐 v0.2：使用 transfer/session id：

```text
.<filename>.mft-<short-transfer-id>.part
```

这样多个并发接收不互相覆盖。

---

## 11. 进度事件设计

### 11.1 `ProgressEvent`

```rust
pub enum ProgressEvent {
    PeerOnline { name: String, addr: SocketAddr },
    PeerDiscovered { peer: PeerRecord },
    PeerExpired { session_id: Uuid },
    TransferStarted { id: Uuid, direction: Direction, peer: String, files: usize, bytes: u64 },
    FileStarted { id: Uuid, path: String, size: u64 },
    FileProgress { id: Uuid, path: String, written: u64, total: u64, bytes_per_sec: f64 },
    FileFinished { id: Uuid, path: String, blake3_hex: String },
    TransferFinished { id: Uuid, files: usize, bytes: u64, elapsed: Duration },
    TransferFailed { id: Option<Uuid>, code: String, message: String },
}
```

### 11.2 CLI 输出

普通文本：

```text
connecting to Lou-Win at 192.168.2.31:48151
sending 2 files, 1.4 GiB
[1/2] a.zip 35% 120 MiB/s ETA 8s
[2/2] demo/photo.jpg done
transfer complete: 2 files, 1.4 GiB in 12.1s
```

JSON 输出：

```json
{"event":"transfer_started","id":"...","direction":"push","peer":"Lou-Win","files":2,"bytes":1503238553}
{"event":"file_progress","path":"a.zip","written":123,"total":456,"bytes_per_sec":104857600.0}
```

---

## 12. 并发模型

### 12.1 任务结构

```text
PeerNode
  ├─ listener_task
  │   └─ per inbound connection task
  ├─ discovery_announce_task
  ├─ discovery_listen_task
  └─ optional cleanup_task for peer ttl
```

### 12.2 入站连接限制

`TransferLimits`：

```rust
pub struct TransferLimits {
    pub chunk_bytes: usize,
    pub max_frame_bytes: usize,
    pub max_concurrent_inbound: usize,
    pub max_manifest_entries: usize,
    pub max_path_bytes: usize,
}
```

默认：

```text
chunk_bytes = 1 MiB
max_frame_bytes = 8 MiB
max_concurrent_inbound = 4
max_manifest_entries = 200_000
max_path_bytes = 4096
```

使用 `Semaphore` 控制入站并发：

```rust
let permit = inbound_semaphore.acquire_owned().await?;
tokio::spawn(async move {
    let _permit = permit;
    handle_inbound_session(...).await
});
```

### 12.3 取消

- Ctrl+C 触发 shutdown token。
- listener 停止 accept。
- discovery 停止广播和接收。
- 正在进行的传输：
  - v0.2 可以直接断开。
  - 本地 `.part` 保留，pull 可续传。
  - push 接收端 `.part` 默认删除或保留？

推荐：

- pull 产生的 `.part` 保留，方便下次续传。
- push 接收端 `.part` 删除，因为首版不支持上传续传。

---

## 13. 配置设计

### 13.1 Mac config

路径：

```text
~/Library/Application Support/mft/config.json
```

字段：

```json
{
  "device_name": "Haven-Mac",
  "listen_addr": "0.0.0.0:48151",
  "discovery_port": 48150,
  "inbox_dir": "/Users/haven/Downloads/mft-inbox",
  "password_record": {
    "salt_hex": "...",
    "hash_hex": "..."
  }
}
```

### 13.2 Windows config

路径：

```text
%APPDATA%\mft\config.json
```

或 Rust `dirs::config_dir()`：

```text
C:\Users\Lou\AppData\Roaming\mft\config.json
```

### 13.3 权限

Mac/Linux：

- 配置目录 `0700`。
- 配置文件 `0600`。

Windows：

- v0.2 可以依赖用户 profile ACL。
- 后续可显式设置 ACL 仅当前用户可读。

---

## 14. 文件与模块接口详情

### 14.1 `peer/config.rs`

职责：

- 定义 `PeerConfig`。
- 定义 policies。
- 提供默认构造。
- 校验参数。

关键类型：

```rust
pub enum AcceptPolicy {
    Always,
    Reject,
    Ask,
}

pub enum OverwritePolicy {
    Never,
    Always,
    Ask,
}

pub enum Direction {
    Push,
    Pull,
}
```

校验：

- `inbox_dir` 不为空。
- `listen_addr.port() != 0` 允许测试，正式 CLI 默认 48151。
- `announce_interval >= 250ms`。
- `peer_ttl > announce_interval`。

### 14.2 `peer/node.rs`

职责：

- 生命周期管理。
- listener 绑定。
- discovery service 启停。
- 暴露 peer table。

不负责：

- 具体文件传输细节。
- CLI 输出。
- TUI 交互。

### 14.3 `peer/initiator.rs`

职责：

- 主动连接 peer。
- push/pull 操作。
- 解析 progress callback。

API：

```rust
pub async fn push_paths(
    addr: SocketAddr,
    password: &str,
    paths: &[PathBuf],
    options: PushOptions,
    progress: impl ProgressSink,
) -> anyhow::Result<TransferReport>;

pub async fn pull_all(
    addr: SocketAddr,
    password: &str,
    out_dir: &Path,
    options: PullOptions,
    progress: impl ProgressSink,
) -> anyhow::Result<TransferReport>;
```

### 14.4 `peer/responder.rs`

职责：

- 接收入站 TCP stream。
- 完成 acceptor handshake。
- 处理 `GetFile` 和 `PutFileStart`。
- 写 inbox / 从 share paths 读取。

API：

```rust
pub async fn handle_inbound_session(
    stream: TcpStream,
    state: Arc<PeerState>,
) -> anyhow::Result<()>;
```

### 14.5 `peer/session.rs`

职责：

- 长度前缀读写。
- 加密帧读写。
- handshake。
- send file content。
- receive file content。
- BLAKE3 helpers。

从当前 `transfer.rs` 迁移：

- `write_plain_frame`
- `read_plain_frame`
- `write_encrypted_frame`
- `read_encrypted_frame`
- `write_len_prefixed`
- `read_len_prefixed`
- `send_file_contents_from_offset`
- `blake3_file`

### 14.6 `peer/table.rs`

职责：

- `upsert(beacon, observed_addr)`。
- `list()`。
- `resolve(name/id)`。
- `prune_expired()`。

### 14.7 `transfer.rs` compatibility facade

保留 public API：

```rust
pub struct TransferServer { inner: PeerNode }

pub async fn download_all(...) -> anyhow::Result<TransferReport> {
    peer::initiator::pull_all(...).await
}

pub async fn upload_paths(...) -> anyhow::Result<TransferReport> {
    peer::initiator::push_paths(...).await
}
```

这样现有 tests 和旧 CLI 不会立即炸。

---

## 15. CLI 改造详情

### 15.1 Mac CLI

当前 `crates/mac-cli/src/main.rs` 中命令：

- `Init`
- `Tui`
- `Serve`
- `Send`
- `Receive`
- `Upload`
- `Discover`

目标：

```rust
enum Commands {
    Init { ... },
    Peer { ... },
    Tui { ... },
    Send { to, connect, paths, ... },
    Pull { from, connect, out, ... },
    Discover { seconds },
    Serve { ... },      // legacy
    Receive { ... },    // legacy
    Upload { ... },     // legacy alias to send --connect
}
```

注意：当前用户曾遇到 `mft: unrecognized option --bind`，说明本地 PATH 里可能是旧安装版本或 CLI 参数位置错误。新设计要求：

- `mft peer --listen` 是明确新命令。
- `mft serve --bind` 可以继续支持或迁移为 `--listen` alias。
- CLI help 必须展示完整参数。

### 15.2 Windows CLI

当前 `crates/win-peer/src/main.rs` 中命令：

- `Discover`
- `Download`
- `Upload`

目标：

```rust
enum Commands {
    Peer { name, listen, inbox, password, share_paths, ... },
    Discover { seconds },
    Send { to, connect, password, paths, discover_seconds },
    Pull { from, connect, password, out, discover_seconds },
    Download { ... }, // legacy alias to pull
    Upload { ... },   // legacy alias to send
}
```

Windows CLI 首版不做 TUI，但要完整支持：

- 常驻 peer 接收 Mac 主动发送。
- 主动 send 到 Mac。
- 主动 pull 从 Mac 下载。
- discover。

---

## 16. 测试设计

### 16.1 单元测试

新增/增强：

1. `PeerConfig` 默认值和校验。
2. `PeerCapabilities` 与 wire capability 字符串互转。
3. `PeerTable`：
   - upsert 新 peer。
   - same session 更新 last_seen。
   - 同名 peer resolve 报 ambiguous。
   - TTL prune。
4. DiscoveryBeacon：
   - v1 beacon 兼容解析。
   - 新可选字段缺失时兼容。
5. Windows 路径清理：
   - 拒绝 `C:\foo`。
   - 拒绝 `..\foo`。
   - 拒绝 `CON` 等保留名。
   - 接受中文和空格。
6. OverwritePolicy：
   - existing file + never => error。
   - existing file + always => replace。

### 16.2 集成测试

本机双 PeerNode：

1. peer A push 单文件到 peer B inbox。
2. peer B push 单文件到 peer A inbox。
3. A push 目录到 B。
4. B pull A share_paths。
5. pull 中断后 `.part` 续传。
6. wrong password 被拒绝。
7. 空目录 manifest。
8. 中文/空格路径。
9. 多个小文件不爆内存。
10. 对端没有 share 时 pull 返回空报告。

### 16.3 CLI smoke tests

可以用 `assert_cmd` 或 shell e2e：

```bash
cargo run -p mft -- peer --listen 127.0.0.1:0 --inbox ...
cargo run -p mft-win-peer -- send --connect 127.0.0.1:<port> --password pw file.txt
```

但端口 `0` 的动态地址不容易从 CLI 拿，建议 core 层 e2e 为主，CLI 只验证：

- help 输出。
- 参数解析。
- dry-run。

### 16.4 跨平台真实验收

Mac 端：

```bash
mft peer --name Haven-Mac --listen 0.0.0.0:48151 --inbox ~/Downloads/mft-inbox ~/mft-share
```

Windows 端：

```powershell
mft-win-peer peer --name Lou-Win --listen 0.0.0.0:48151 --inbox C:\Users\Lou\Downloads\mft-inbox
```

验收项：

1. Windows 能 discover Mac。
2. Mac 能 discover Windows。
3. Mac send 到 Windows inbox。
4. Windows send 到 Mac inbox。
5. Windows pull Mac share。
6. 密码错误可读错误。
7. 手动 `--connect` 可绕过 discovery。
8. 防火墙阻断时有可读错误。

---

## 17. 迁移计划

### Phase 0：文档与边界确认

输出：

- 本设计文档。
- Goal 模式 prompt。

### Phase 1：Peer core 骨架

完成：

- `peer/mod.rs`
- `PeerConfig`
- `PeerCapabilities`
- `PeerTable`
- `ProgressEvent`
- `TransferLimits`

验证：

```bash
cargo test -p mft-core peer
```

### Phase 2：Session 迁移

完成：

- 从 `transfer.rs` 迁移 handshake、frame IO、file streaming 到 `peer/session.rs`。
- 保持旧测试通过。

验证：

```bash
cargo test -p mft-core
cargo test -p mft-protocol
```

### Phase 3：PeerNode listener/responder

完成：

- `PeerNode::bind()`。
- inbound session handler。
- inbox receive。
- share provider。
- `TransferServer` 兼容包装。

验证：

```bash
cargo test -p mft-core
```

### Phase 4：Initiator push/pull

完成：

- `push_paths()`。
- `pull_all()`。
- `download_all/upload_paths` facade 调用新实现。

验证：

```bash
cargo test -p mft-core
```

### Phase 5：DiscoveryService

完成：

- announce loop。
- listen loop。
- peer table 更新。
- TTL prune。
- 临时 `discover_for()` 兼容。

验证：

```bash
cargo test -p mft-core discovery
```

### Phase 6：CLI P2P 命令

完成：

- Mac `mft peer/send/pull/discover`。
- Windows `mft-win-peer peer/send/pull/discover`。
- legacy aliases。

验证：

```bash
cargo test --workspace
cargo run -p mft -- --help
cargo run -p mft-win-peer -- --help
```

### Phase 7：真实部署验证

完成：

- 构建 Mac 二进制。
- 构建 Windows `.exe`。
- 上传到 `win_gw_lou`。
- Mac/Windows 双向传输验收。

---

## 18. 兼容性策略

### 18.1 保持 API 兼容

不能立刻删除：

- `TransferServer`
- `download_all`
- `upload_paths`
- `DiscoveryBeacon::new`
- `broadcast_once`
- `discover_for`

### 18.2 命令兼容

旧命令保留：

- `mft serve`
- `mft receive`
- `mft upload`
- `mft-win-peer download`
- `mft-win-peer upload`

但 help 中可以标注：

```text
legacy alias; prefer peer/send/pull
```

### 18.3 Wire 兼容

v0.2 不强制改 `Frame` enum 字段名，避免破坏现有测试。

内部代码用注释解释：

```rust
// Wire compatibility: HelloAck.server_nonce is responder_nonce in PeerNode terminology.
```

---

## 19. 风险与对策

| 风险 | 影响 | 对策 |
|---|---|---|
| UDP discovery 端口复用跨平台不一致 | peer/discover 冲突 | 使用 socket2；如果失败，给出明确错误和 --connect 兜底 |
| Windows 防火墙拦截入站 TCP | Mac 无法主动 send 到 Windows | 提供 --connect 测试命令和防火墙提示 |
| 同名设备 | send --to 选错 | PeerTable ambiguous error，要求 session id/address |
| 旧命令用户困惑 | 使用失败 | 保留 alias，并在 help 提示新命令 |
| 一次性重构太大 | 引入回归 | transfer.rs facade 分阶段迁移 |
| Push 不支持断点续传 | 大文件上传中断重来 | 文档明确；pull 先支持 resume |
| Windows 路径差异 | 路径逃逸/写入失败 | 增强 path tests |

---

## 20. 验收标准

### 20.1 代码级验收

必须通过：

```bash
cargo test --workspace
cargo build --release -p mft
cargo build --release -p mft-win-peer
```

### 20.2 CLI 级验收

必须看到：

```bash
mft --help
mft peer --help
mft send --help
mft pull --help
mft-win-peer --help
mft-win-peer peer --help
mft-win-peer send --help
mft-win-peer pull --help
```

### 20.3 本机 e2e 验收

必须覆盖：

- A push B。
- B push A。
- A pull B。
- B pull A。
- wrong password。
- resume pull。

### 20.4 跨机器验收

在 Mac 和 `win_gw_lou` 上必须完成：

1. Windows 原生 `.exe` 可运行，不是 WSL Linux 二进制。
2. 双方 `peer` 同时在线。
3. 双方 discover 互相可见。
4. Mac 主动 send 到 Windows 成功。
5. Windows 主动 send 到 Mac 成功。
6. Windows pull Mac share 成功。
7. 手动 `--connect` 成功。

---

## 21. 最小实施切片

如果实现时需要控制范围，最小可交付切片是：

1. `PeerNode::bind()` 等价包装现有 `TransferServer` listener。
2. Windows 新增 `peer` 命令，可以接收入站上传。
3. Mac/Windows 新增 `send --connect`，可主动 push 到对方。
4. Windows 主动 send 到 Mac 可用。
5. Mac 主动 send 到 Windows 可用。
6. 发现和 `--to` 可以作为下一小步，但最终 P2P 验收必须包含。

但本目标完整交付应包含 discovery peer table 和 `--to`。

---

## 22. 设计结论

MFT v0.2 应从“Mac server + Windows client”升级为“对等 PeerNode runtime”。实现上不需要立刻推翻现有协议，而是先把架构边界改正确：

- 程序身份：统一为 peer。
- 连接身份：临时 initiator/responder。
- 文件方向：push/pull。
- CLI：双方都有 `peer/send/pull/discover`。
- Core：`PeerNode` 管 listener、discovery、peer table；initiator/responder 管传输。
- 兼容：旧 API 和旧命令保留，内部逐步迁移。

这样能最快得到真正可用的局域网 P2P 文件传输，同时保留现有加密、manifest、BLAKE3、断点续传等已完成资产。
