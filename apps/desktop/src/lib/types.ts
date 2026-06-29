export type AppDirs = {
  baseDir: string;
  inboxDir: string;
  shareDir: string;
  receivedDir: string;
  configPath: string;
};

export type AppConfig = {
  deviceId: string;
  deviceName: string;
  listenAddr: string;
  discoveryTargets: string[];
  dirs: AppDirs;
};

export type AppState = {
  setupComplete: boolean;
  peerRunning: boolean;
  localAddr: string | null;
  localDeviceId: string | null;
  localSessionId: string | null;
  trustedDevices: TrustedDevice[];
  config: AppConfig | null;
};

export type SetupRequest = {
  deviceName: string;
  password?: string;
  baseDir?: string | null;
  listenAddr?: string | null;
};

export type SettingsRequest = {
  deviceName?: string | null;
  password?: string | null;
  listenAddr?: string | null;
  inboxDir?: string | null;
  shareDir?: string | null;
  receivedDir?: string | null;
  discoveryTargets?: string[] | null;
};

export type Peer = {
  deviceId: string;
  deviceName: string;
  sessionId: string;
  addr: string | null;
  port: number;
  capabilities: string[];
  version: number;
};

export type TrustedDevice = {
  deviceId: string;
  displayName: string;
  firstTrustedAtMs: number;
  lastSeenAtMs: number | null;
};

export type TransferReport = {
  files: number;
  bytes: number;
};

export type TransferEvent = {
  id: string;
  direction: "push" | "pull" | "incoming";
  peer: string;
  paths: string[];
  report: TransferReport | null;
  message: string | null;
};

export type Activity = TransferEvent & {
  status: "running" | "finished" | "failed" | "expired" | "rejected";
  createdAt: number;
};

/** Live state for the foreground transfer dialog (progress + speed). */
export type ActiveTransfer = {
  id: string;
  direction: "push" | "pull" | "incoming";
  peer: string;
  status: "running" | "finished" | "failed";
  files: number | null;
  /** Total bytes to move, when known (from the offer/report). */
  totalBytes: number | null;
  /** Bytes moved so far. Stays 0 until the backend emits progress events. */
  transferredBytes: number;
  startedAt: number;
  finishedAt: number | null;
  message: string | null;
};

export type IncomingTransferRequest = {
  id: string;
  deviceId: string;
  deviceName: string;
  files: number;
  bytes: number;
  pathsPreview: string[];
  createdAtMs: number;
};

export type TransferProgress = {
  id: string;
  direction: "push" | "pull" | "incoming";
  peer: string;
  transferred: number;
  total: number;
};

export type InboxEntry = {
  name: string;
  path: string;
  size: number;
  isDir: boolean;
  modifiedMs: number;
};
