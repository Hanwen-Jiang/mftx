# MFTX 项目接手文档

## 项目概览

项目：MFTX / MFT 文件传输工具

本地源码路径：`/Users/haven/Documents/code/file-transfer`

Rust workspace 组成：

- `crates/protocol`：协议帧、加密、manifest、路径清理。
- `crates/core`：发现、PeerNode、配置、传输核心。
- `crates/mac-cli`：Mac 端 CLI，当前本机命令为 `mftx`。
- `crates/win-peer`：Windows 端 CLI，当前远端命令为 `mft-win-peer.exe`。

## 当前工作进展

已完成：

1. 在原 UDP 广播发现基础上，新增 ARP/邻居表单播探测。
2. 新增 discovery probe 消息类型，常驻 peer 收到 probe 后会立即返回 beacon。
3. Windows 端 release 已部署到：`C:\Users\Lou\mft\mft-win-peer.exe`。
4. Windows 用户 PATH 已把 `C:\Users\Lou\mft` 放到前面，可以直接运行 `mft-win-peer.exe`。
5. Mac 端已创建 `mftx` 命令：`/Users/haven/.local/bin/mftx`，并加入 zsh PATH。

## 关键改动文件

- `crates/core/src/discovery.rs`
- `crates/core/src/peer/discovery_service.rs`
- `crates/core/tests/core_tests.rs`

## 验证情况

已验证过：

- Mac 本地：`cargo test --workspace` 通过。
- Windows 端：`cargo test --workspace` 通过。
- Windows release：`cargo build --release -p mft-win-peer` 通过。
- Windows 部署产物 SHA256：`26EB9CCB5559A64A04C0C9483B9F256121860B16F7818CB0A1860594F5B7EB21`。

## 两机测试方式

### Windows 端启动 peer

如果希望 Mac 传来的文件直接落到 `E:\jhw`：

```powershell
New-Item -ItemType Directory -Force E:\jhw
mft-win-peer.exe peer --name Lou-Win --listen 0.0.0.0:48151 --password test123 --inbox E:\jhw
```

该窗口保持打开。

### Mac 端发现和发送

```bash
mftx discover --seconds 5
MFTX_PASSWORD=test123 mftx send --to Lou-Win /tmp/example.txt
```

发现不稳定时可直连：

```bash
MFTX_PASSWORD=test123 mftx send --connect 100.122.46.119:48151 /tmp/example.txt
```

## 源码包说明

本次交接上传到远端 `E:\jhw` 的源码包已排除：

- `target/`
- `.git/`
- `.DS_Store`
- IDE 临时目录

解压方式：

```powershell
tar -xzf E:\jhw\mftx-project-*.tar.gz -C E:\jhw
```

解压后目录：

```text
E:\jhw\mftx-project
```

进入后可以验证：

```powershell
cd E:\jhw\mftx-project
cargo test --workspace
cargo build --release -p mft-win-peer
```

## 注意点

- 当前发送端不能指定远端任意落盘目录；接收目录由 peer 启动时的 `--inbox` 决定。
- 因此要让文件落到 `E:\jhw`，Windows peer 需要以 `--inbox E:\jhw` 启动。
- 如果 SSH 不稳定，可用 `mftx send` 传包；如果 MFTX 发现不到，可使用 `--connect 100.122.46.119:48151`。
