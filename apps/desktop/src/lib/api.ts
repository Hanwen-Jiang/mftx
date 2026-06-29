import {invoke} from "@tauri-apps/api/core";
import {listen} from "@tauri-apps/api/event";
import {open} from "@tauri-apps/plugin-dialog";

import type {
  AppState,
  InboxEntry,
  IncomingTransferRequest,
  Peer,
  SettingsRequest,
  SetupRequest,
  TransferEvent,
  TransferProgress,
  TransferReport,
  TrustedDevice,
} from "./types";

type PullRequest = {
  addr: string;
  password: string;
  outDir?: string | null;
};

type SendPathsRequest = {
  addr: string;
  paths: string[];
};

type IncomingTransferDecision = {
  id: string;
  accepted: boolean;
  trustDevice: boolean;
};

const isTauri = "__TAURI_INTERNALS__" in window;

const mockState: AppState = {
  setupComplete: false,
  peerRunning: false,
  localAddr: null,
  localDeviceId: null,
  localSessionId: null,
  trustedDevices: [],
  config: null,
};

export async function getAppState(): Promise<AppState> {
  if (!isTauri) return mockState;
  return invoke<AppState>("get_app_state");
}

export async function getDefaultSetup(): Promise<SetupRequest> {
  if (!isTauri) {
    return {
      deviceName: "Lou-Win",
      baseDir: "C:\\Users\\Lou\\mftx",
      listenAddr: "0.0.0.0:48151",
    };
  }
  return invoke<SetupRequest>("get_default_setup");
}

export async function completeSetup(request: SetupRequest): Promise<AppState> {
  if (!isTauri) {
    return {
      setupComplete: true,
      peerRunning: false,
      localAddr: null,
      localDeviceId: "local-demo-device",
      localSessionId: null,
      trustedDevices: [],
      config: {
        deviceId: "local-demo-device",
        deviceName: request.deviceName,
        listenAddr: request.listenAddr ?? "0.0.0.0:48151",
        discoveryTargets: [],
        dirs: {
          baseDir: request.baseDir ?? "C:\\Users\\Lou\\mftx",
          inboxDir: `${request.baseDir ?? "C:\\Users\\Lou\\mftx"}\\inbox`,
          shareDir: `${request.baseDir ?? "C:\\Users\\Lou\\mftx"}\\share`,
          receivedDir: `${request.baseDir ?? "C:\\Users\\Lou\\mftx"}\\received`,
          configPath: `${request.baseDir ?? "C:\\Users\\Lou\\mftx"}\\config.json`,
        },
      },
    };
  }
  return invoke<AppState>("complete_setup", {request});
}

export async function updateSettings(request: SettingsRequest): Promise<AppState> {
  if (!isTauri) return getAppState();
  return invoke<AppState>("update_settings", {request});
}

export async function startPeer(): Promise<AppState> {
  if (!isTauri) {
    return {
      ...mockState,
      setupComplete: true,
      peerRunning: true,
      localAddr: "127.0.0.1:48151",
      localDeviceId: "local-demo-device",
      localSessionId: "local-demo-peer",
      trustedDevices: [],
    };
  }
  return invoke<AppState>("start_peer");
}

export async function stopPeer(): Promise<AppState> {
  if (!isTauri) return {...mockState, setupComplete: true, peerRunning: false};
  return invoke<AppState>("stop_peer");
}

export async function discoverPeers(seconds = 3): Promise<Peer[]> {
  if (!isTauri) {
    return [
      {
        deviceId: "hanwen-demo-device",
        deviceName: "Hanwen-Mac",
        sessionId: "demo-peer",
        addr: "192.168.1.12:48151",
        port: 48151,
        capabilities: ["receive", "push", "pull", "encrypted", "blake3"],
        version: 1,
      },
    ];
  }
  return invoke<Peer[]>("discover_peers", {seconds});
}

export async function listTrustedDevices(): Promise<TrustedDevice[]> {
  if (!isTauri) return [];
  return invoke<TrustedDevice[]>("list_trusted_devices");
}

export async function untrustDevice(deviceId: string): Promise<AppState> {
  if (!isTauri) return getAppState();
  return invoke<AppState>("untrust_device", {request: {deviceId}});
}

export async function sendPaths(request: SendPathsRequest): Promise<TransferReport> {
  if (!isTauri) return {files: request.paths.length, bytes: 0};
  return invoke<TransferReport>("send_paths", {request});
}

export async function respondIncomingTransfer(decision: IncomingTransferDecision): Promise<void> {
  if (!isTauri) return;
  return invoke<void>("respond_incoming_transfer", {decision});
}

export async function pullFromPeer(request: PullRequest): Promise<TransferReport> {
  if (!isTauri) return {files: 3, bytes: 1024 * 1024};
  return invoke<TransferReport>("pull_from_peer", {request});
}

export async function openInbox(): Promise<void> {
  if (!isTauri) return;
  await invoke("open_inbox");
}

export async function connectPeer(addr: string): Promise<AppState> {
  if (!isTauri) return getAppState();
  return invoke<AppState>("connect_peer", {addr});
}

export async function listInbox(): Promise<InboxEntry[]> {
  if (!isTauri) return [];
  return invoke<InboxEntry[]>("list_inbox");
}

export async function revealPath(path: string): Promise<void> {
  if (!isTauri) return;
  await invoke("reveal_path", {path});
}

export async function chooseTransferPaths(): Promise<string[]> {
  if (!isTauri) return [];
  const selected = await open({
    multiple: true,
    directory: false,
    title: "选择要发送的文件",
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function chooseTransferDirectories(): Promise<string[]> {
  if (!isTauri) return [];
  const selected = await open({
    multiple: true,
    directory: true,
    title: "选择要发送的文件夹",
  });
  if (!selected) return [];
  return Array.isArray(selected) ? selected : [selected];
}

export async function chooseDirectory(title: string): Promise<string | null> {
  if (!isTauri) return null;
  const selected = await open({
    multiple: false,
    directory: true,
    title,
  });
  return typeof selected === "string" ? selected : null;
}

export function onTransferStarted(callback: (event: TransferEvent) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<TransferEvent>("mftx://transfer-started", (event) => callback(event.payload));
}

export function onTransferProgress(callback: (progress: TransferProgress) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<TransferProgress>("mftx://transfer-progress", (event) => callback(event.payload));
}

export function onTransferFinished(callback: (event: TransferEvent) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<TransferEvent>("mftx://transfer-finished", (event) => callback(event.payload));
}

export function onTransferFailed(callback: (event: TransferEvent) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<TransferEvent>("mftx://transfer-failed", (event) => callback(event.payload));
}

export function onIncomingTransferRequested(callback: (request: IncomingTransferRequest) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<IncomingTransferRequest>("mftx://transfer/incoming-requested", (event) =>
    callback(event.payload),
  );
}

export function onIncomingTransferExpired(callback: (request: IncomingTransferRequest) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<IncomingTransferRequest>("mftx://transfer/incoming-expired", (event) =>
    callback(event.payload),
  );
}

export function onTrustChanged(callback: () => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen("mftx://trust/changed", () => callback());
}

export function onPeerDiscovered(callback: (peer: Peer) => void) {
  if (!isTauri) return Promise.resolve(() => undefined);
  return listen<Peer>("mftx://peer-discovered", (event) => callback(event.payload));
}
