# Goal 模式 Prompt：实现 MFT PeerNode P2P 架构

请在 `/Users/haven/Documents/code/file-transfer` 仓库中继续工作，目标是把当前 server-client 形态的 MFT 改造成 `PeerNode + peer 命令 + Windows 主动/被动对等传输` 的局域网 P2P 首版。

## 背景

当前仓库是 Rust workspace：

- `crates/protocol`：帧格式、manifest、crypto、path。
- `crates/core`：discovery、fs_manifest、transfer。
- `crates/mac-cli`：Mac CLI/TUI，命令名 `mft`。
- `crates/win-peer`：Windows 最小 CLI，命令名 `mft-win-peer`。

当前问题：

- `crates/core/src/transfer.rs` 以 `TransferServer` 为中心。
- Mac 端负责 serve，Windows 端更多是 client。
- Discovery 看起来像 peer，但 TCP 传输语义仍是 server-client。
- 需要让 Mac 和 Windows 都能作为 peer：既能主动发送，也能被动接收。

详细设计文档已经写好，请先阅读并以它为准：

```text
/Users/haven/Documents/code/file-transfer/docs/architecture/peernode-p2p-architecture.md
```

## 总目标

实现局域网 P2P 首版：

1. 新增 core 层 `PeerNode` 架构。
2. 新增/完善 `peer` 命令。
3. Mac 和 Windows 双方都能启动常驻 peer。
4. Mac 可以主动 send 到 Windows。
5. Windows 可以主动 send 到 Mac。
6. Windows 可以作为被动接收端。
7. 保留手动 `--connect ip:port` 兜底。
8. 保留 UDP discovery，并支持 `--to <peer-name>` 查找对端。
9. 保留现有加密、manifest、BLAKE3、`.part`、pull resume 逻辑。
10. 旧命令尽量兼容，不要无故删除现有 API。

## 实施原则

- 不要一次性推翻所有代码。
- 优先复用当前 `transfer.rs` 中已经可用的握手、加密帧、文件流式读写、BLAKE3 校验逻辑。
- 用 `PeerNode` 正确重塑架构边界。
- wire format 首版可以保持 `Frame` enum 不大改，内部命名改成 initiator/responder。
- 保留：
  - `TransferServer`
  - `download_all`
  - `upload_paths`
  - `DiscoveryBeacon`
  - `broadcast_once`
  - `discover_for`
- 让旧 API 调用新 peer 内核，作为 compatibility facade。
- 每完成一个阶段都运行相关测试。

## 推荐实施阶段

### Phase 1：Peer core 骨架

新增目录：

```text
crates/core/src/peer/
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

至少实现：

- `PeerConfig`
- `PeerCapabilities`
- `PeerNode`
- `PeerTable`
- `ProgressEvent`
- `TransferLimits`
- `AcceptPolicy`
- `OverwritePolicy`

测试：

```bash
cargo test -p mft-core
```

### Phase 2：Session 迁移

从 `crates/core/src/transfer.rs` 迁移或复用这些逻辑到 `peer/session.rs`：

- plain/encrypted frame read/write。
- length-prefixed frame IO。
- initiator/responder handshake。
- send file contents from offset。
- receive upload。
- BLAKE3 file hash。
- safe join / part path。

注意：wire 字段 `HelloAck.server_nonce` 可以保留，但注释说明它在 PeerNode 语义中是 responder nonce。

测试：

```bash
cargo test -p mft-core
cargo test -p mft-protocol
```

### Phase 3：PeerNode listener/responder

实现：

- `PeerNode::bind(config)`。
- TCP listener。
- inbound connection handler。
- responder 能处理：
  - `GetFile`：对端 pull 本机 share。
  - `PutFileStart`：对端 push 到本机 inbox。
- inbox 自动创建。
- share manifest 构建。
- 入站并发限制。

把旧 `TransferServer` 改成包装 `PeerNode` 或调用同一 responder 内核。

测试：

```bash
cargo test -p mft-core
```

### Phase 4：Initiator push/pull

实现：

- `peer::initiator::push_paths(...)`
- `peer::initiator::pull_all(...)`

然后把旧函数改成 facade：

- `upload_paths()` -> `push_paths()`。
- `download_all()` -> `pull_all()`。

测试必须覆盖：

- A push B。
- B push A。
- A pull B。
- wrong password。
- pull resume。

运行：

```bash
cargo test -p mft-core
```

### Phase 5：DiscoveryService + PeerTable

实现：

- 常驻 peer 周期性 announce。
- 常驻 peer 监听 discovery。
- 更新 `PeerTable`。
- TTL 过期清理。
- `--to` / `--from` 名称解析。
- 同名 peer 报 ambiguous，要求 session id 或 `--connect`。

如遇 UDP 48150 多进程绑定冲突，优先用 `socket2` 支持 reuse address/port；如果短期无法跨平台稳定实现，至少保证常驻 peer 内部 discovery 可用，临时 discover 命令有可读错误。

测试：

```bash
cargo test -p mft-core discovery
cargo test -p mft-core peer
```

### Phase 6：CLI 命令

Mac `mft` 新增/调整：

```text
mft peer
mft send --to <PEER> <PATHS>...
mft send --connect <IP:PORT> <PATHS>...
mft pull --from <PEER> --out <DIR>
mft pull --connect <IP:PORT> --out <DIR>
mft discover
```

Windows `mft-win-peer` 新增/调整：

```text
mft-win-peer peer
mft-win-peer send --to <PEER> <PATHS>...
mft-win-peer send --connect <IP:PORT> <PATHS>...
mft-win-peer pull --from <PEER> --out <DIR>
mft-win-peer pull --connect <IP:PORT> --out <DIR>
mft-win-peer discover
```

旧命令兼容：

```text
mft serve
mft receive
mft upload
mft-win-peer download
mft-win-peer upload
```

测试：

```bash
cargo test --workspace
cargo run -p mft -- --help
cargo run -p mft -- peer --help
cargo run -p mft -- send --help
cargo run -p mft -- pull --help
cargo run -p mft-win-peer -- --help
cargo run -p mft-win-peer -- peer --help
cargo run -p mft-win-peer -- send --help
cargo run -p mft-win-peer -- pull --help
```

### Phase 7：真实 Windows 宿主部署验证

注意：Windows 宿主是 `win_gw_lou`，不是 `gw_wsl`。`gw_wsl` 是 WSL Linux。

需要构建 Windows 原生 `.exe` 并部署到：

```text
C:\Users\Lou\.cargo\bin\mft-win-peer.exe
```

验证：

```bash
ssh win_gw_lou 'mft-win-peer --help'
ssh win_gw_lou 'mft-win-peer peer --help'
ssh win_gw_lou 'mft-win-peer send --help'
```

跨机器验收：

1. Mac 启动：

```bash
mft peer --name Haven-Mac --listen 0.0.0.0:48151 --inbox "$HOME/Downloads/mft-inbox" "$HOME/mft-folder"
```

2. Windows 启动：

```powershell
mft-win-peer peer --name Lou-Win --listen 0.0.0.0:48151 --inbox C:\Users\Lou\Downloads\mft-inbox
```

3. Windows discover Mac。
4. Mac discover Windows。
5. Mac send 到 Windows。
6. Windows send 到 Mac。
7. Windows pull Mac share。
8. 手动 `--connect` 成功。
9. 密码错误给出可读错误。

## 验收标准

最终必须满足：

- `cargo test --workspace` 通过。
- `cargo build --release -p mft` 通过。
- `cargo build --release -p mft-win-peer` 通过。
- Mac CLI help 展示 `peer/send/pull/discover`。
- Windows CLI help 展示 `peer/send/pull/discover`。
- 本机 core e2e 覆盖双向 push/pull。
- Windows 原生 `mft-win-peer.exe` 已部署到 `win_gw_lou`。
- 真实 Mac/Windows 双向传输完成。

## 输出要求

完成后请用简体中文汇报：

1. 改了哪些核心文件。
2. 新的命令怎么使用。
3. 测试/构建结果。
4. Windows 宿主部署路径。
5. 已验证的 Mac/Windows 双向传输结果。
6. 仍未完成或后续建议。

不要把 WSL 当成 Windows 宿主；目标 Windows 宿主是 `win_gw_lou`。
