import {create} from "zustand";

import {applyTheme, getInitialTheme, type Theme} from "./theme";
import type {
  ActiveTransfer,
  Activity,
  AppState,
  IncomingTransferRequest,
  Peer,
  TransferEvent,
} from "./types";

type View = "transfer" | "inbox" | "peers" | "settings";

const AUTO_START_KEY = "mftx-auto-start-peer";
const ACTIVITIES_KEY = "mftx-activities";
const MAX_ACTIVITIES = 100;

/** Read the "auto-start receiver on launch" preference (defaults to on). */
function getInitialAutoStart(): boolean {
  if (typeof window === "undefined") return true;
  return window.localStorage.getItem(AUTO_START_KEY) !== "false";
}

function persistAutoStart(value: boolean) {
  try {
    window.localStorage.setItem(AUTO_START_KEY, value ? "true" : "false");
  } catch {
    /* localStorage unavailable — ignore */
  }
}

/** Activity history is persisted so it survives a refresh / app restart. */
function getInitialActivities(): Activity[] {
  if (typeof window === "undefined") return [];
  try {
    const raw = window.localStorage.getItem(ACTIVITIES_KEY);
    const parsed = raw ? JSON.parse(raw) : [];
    return Array.isArray(parsed) ? (parsed as Activity[]) : [];
  } catch {
    return [];
  }
}

function persistActivities(activities: Activity[]) {
  try {
    window.localStorage.setItem(ACTIVITIES_KEY, JSON.stringify(activities));
  } catch {
    /* localStorage unavailable / quota — ignore */
  }
}

type AppStore = {
  state: AppState | null;
  peers: Peer[];
  incomingRequests: IncomingTransferRequest[];
  activities: Activity[];
  activeTransfer: ActiveTransfer | null;
  selectedPeerAddr: string | null;
  manualAddr: string;
  selectedView: View;
  theme: Theme;
  autoStartPeer: boolean;
  busy: boolean;
  discovering: boolean;
  message: string | null;
  toggleTheme: () => void;
  setAutoStartPeer: (value: boolean) => void;
  setState: (state: AppState) => void;
  setPeers: (peers: Peer[]) => void;
  addPeer: (peer: Peer) => void;
  addIncomingRequest: (request: IncomingTransferRequest) => void;
  removeIncomingRequest: (id: string) => void;
  setSelectedPeerAddr: (addr: string | null) => void;
  setManualAddr: (addr: string) => void;
  setSelectedView: (view: View) => void;
  setBusy: (busy: boolean) => void;
  setDiscovering: (discovering: boolean) => void;
  setMessage: (message: string | null) => void;
  addActivity: (event: TransferEvent, status: Activity["status"]) => void;
  clearActivities: () => void;
  beginTransfer: (transfer: ActiveTransfer) => void;
  updateTransfer: (id: string, patch: Partial<ActiveTransfer>) => void;
  dismissTransfer: () => void;
};

function cleanPeers(peers: Peer[], state: AppState | null) {
  const seenDeviceIds = new Set<string>();
  const seenSessions = new Set<string>();
  const seenAddrs = new Set<string>();
  const localDeviceId = state?.localDeviceId ?? null;
  const localSessionId = state?.localSessionId ?? null;
  const localAddr = state?.localAddr ?? null;

  return peers
    .filter((peer) => {
      if (localDeviceId && peer.deviceId === localDeviceId) return false;
      if (localSessionId && peer.sessionId === localSessionId) return false;
      if (localAddr && peer.addr === localAddr) return false;
      if (seenDeviceIds.has(peer.deviceId)) return false;
      seenDeviceIds.add(peer.deviceId);
      if (seenSessions.has(peer.sessionId)) return false;
      seenSessions.add(peer.sessionId);

      if (peer.addr) {
        if (seenAddrs.has(peer.addr)) return false;
        seenAddrs.add(peer.addr);
      }

      return true;
    })
    .sort((a, b) => {
      const byName = a.deviceName.localeCompare(b.deviceName);
      if (byName !== 0) return byName;
      const byAddr = (a.addr ?? "").localeCompare(b.addr ?? "");
      if (byAddr !== 0) return byAddr;
      return a.sessionId.localeCompare(b.sessionId);
    });
}

function nextSelectedPeerAddr(
  peers: Peer[],
  selectedPeerAddr: string | null,
  manualAddr: string,
) {
  if (selectedPeerAddr && peers.some((peer) => peer.addr === selectedPeerAddr)) {
    return selectedPeerAddr;
  }
  // The user is targeting a manually entered address; never auto-select a
  // discovered peer over it — that would silently change the send target.
  if (manualAddr.trim()) {
    return null;
  }
  return peers.find((peer) => peer.addr)?.addr ?? null;
}

export const useAppStore = create<AppStore>((set) => ({
  state: null,
  peers: [],
  incomingRequests: [],
  activities: getInitialActivities(),
  activeTransfer: null,
  selectedPeerAddr: null,
  manualAddr: "",
  selectedView: "transfer",
  theme: getInitialTheme(),
  autoStartPeer: getInitialAutoStart(),
  busy: false,
  discovering: false,
  message: null,
  toggleTheme: () =>
    set((current) => {
      const theme: Theme = current.theme === "dark" ? "light" : "dark";
      applyTheme(theme);
      return {theme};
    }),
  setAutoStartPeer: (autoStartPeer) => {
    persistAutoStart(autoStartPeer);
    set({autoStartPeer});
  },
  setState: (state) =>
    set((current) => {
      const peers = cleanPeers(current.peers, state);
      return {
        state,
        peers,
        selectedPeerAddr: nextSelectedPeerAddr(peers, current.selectedPeerAddr, current.manualAddr),
      };
    }),
  setPeers: (peers) =>
    set((current) => {
      const cleaned = cleanPeers(peers, current.state);
      return {
        peers: cleaned,
        selectedPeerAddr: nextSelectedPeerAddr(cleaned, current.selectedPeerAddr, current.manualAddr),
      };
    }),
  addPeer: (peer) =>
    set((current) => {
      // Merge a streamed peer in immediately; cleanPeers dedupes so re-emits of
      // an already-known device are idempotent.
      const cleaned = cleanPeers([peer, ...current.peers], current.state);
      return {
        peers: cleaned,
        selectedPeerAddr: nextSelectedPeerAddr(cleaned, current.selectedPeerAddr, current.manualAddr),
      };
    }),
  addIncomingRequest: (request) =>
    set((current) => ({
      incomingRequests: [
        request,
        ...current.incomingRequests.filter((item) => item.id !== request.id),
      ],
    })),
  removeIncomingRequest: (id) =>
    set((current) => ({
      incomingRequests: current.incomingRequests.filter((request) => request.id !== id),
    })),
  setSelectedPeerAddr: (selectedPeerAddr) =>
    set((current) => ({
      selectedPeerAddr,
      // Picking a discovered peer supersedes any half-typed manual address.
      manualAddr: selectedPeerAddr ? "" : current.manualAddr,
    })),
  setManualAddr: (manualAddr) =>
    set((current) => ({
      manualAddr,
      // Typing a manual address takes over from any auto/explicit peer selection.
      selectedPeerAddr: manualAddr.trim() ? null : current.selectedPeerAddr,
    })),
  setSelectedView: (selectedView) => set({selectedView}),
  setBusy: (busy) => set({busy}),
  setDiscovering: (discovering) => set({discovering}),
  setMessage: (message) => set({message}),
  addActivity: (event, status) =>
    set((current) => {
      const existing = current.activities.find((activity) => activity.id === event.id);
      const next: Activity = {
        ...event,
        status,
        createdAt: existing?.createdAt ?? Date.now(),
      };
      const activities = [
        next,
        ...current.activities.filter((activity) => activity.id !== event.id),
      ].slice(0, MAX_ACTIVITIES);
      persistActivities(activities);
      return {activities};
    }),
  clearActivities: () => {
    persistActivities([]);
    set({activities: []});
  },
  beginTransfer: (transfer) => set({activeTransfer: transfer}),
  updateTransfer: (id, patch) =>
    set((current) =>
      current.activeTransfer && current.activeTransfer.id === id
        ? {activeTransfer: {...current.activeTransfer, ...patch}}
        : {},
    ),
  dismissTransfer: () => set({activeTransfer: null}),
}));
