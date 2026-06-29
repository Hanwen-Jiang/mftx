# MFT

MFT is a lightweight Rust file transfer tool for macOS and Windows peers on the
same LAN. The macOS side is the primary target and exposes a CLI/TUI binary named
`mft`; the repository also includes `mft-win-peer`, a minimal Windows-side CLI
peer for protocol validation and practical transfers.

## What Works

- Encrypted custom TCP protocol with password-derived session keys.
- UDP LAN discovery with manual `ip:port` fallback.
- One-shot file and directory transfer.
- Mac-to-peer download resume via `.part` files.
- Peer-to-Mac upload into an inbox directory.
- BLAKE3 integrity checks and atomic rename after successful receive.

## Quick Start

Initialize a fixed password on macOS:

```bash
cargo run -p mft -- init
```

Start an upload/download session from macOS:

```bash
cargo run -p mft -- send ~/Downloads/example.zip
```

Start the TUI:

```bash
cargo run -p mft -- tui ~/Downloads/example.zip
```

Download from the peer side:

```bash
cargo run -p mft-win-peer -- download --connect 192.168.1.10:48151 --password '<password>' --out ./received
```

Upload from the peer side:

```bash
cargo run -p mft-win-peer -- upload --connect 192.168.1.10:48151 --password '<password>' ./folder
```

If `--password` is omitted, `mft-win-peer` prompts without echoing the password.
If `--connect` is omitted, it listens for discovery packets for three seconds:

```bash
cargo run -p mft-win-peer -- discover
cargo run -p mft-win-peer -- download --out ./received
cargo run -p mft-win-peer -- upload ./folder
```

## Desktop GUI

The desktop app lives in `apps/desktop` and uses Tauri v2, React, Vite,
TypeScript, and HeroUI Pro.

Install and refresh the Pro artifacts:

```bash
cd apps/desktop
npm install
npx -y hpsetup@latest <HP_KEY> react
```

Run the desktop app in development:

```bash
cd apps/desktop
npm run tauri:dev
```

Build Windows installers:

```bash
cd apps/desktop
npm run tauri:build
```

On the current GNU Rust toolchain, the built `mft-desktop.exe` needs
`WebView2Loader.dll` beside it. Use the NSIS/MSI installer from
`target/release/bundle`, or run the executable from `target/release` where
Cargo places both files together. Do not copy only `mft-desktop.exe` to another
folder.

Useful verification commands:

```bash
cd apps/desktop
npm run typecheck
npm run build

cd ../..
cargo check -p mft-desktop
cargo test --workspace
```

The first GUI version can initialize MFTX config, start/stop the local peer,
discover LAN peers, send files or folders, pull from a peer, show transfer
results, and open inbox/received directories.

## Current Boundaries

- LAN only; no relay, accounts, or NAT traversal.
- GUI transfer progress is currently task-level start/finish/fail; per-file
  progress events need the core transfer facade to emit `ProgressEvent`.
- Upload resume is not implemented yet.
- Discovery broadcasts do not include file names or shared path names.
