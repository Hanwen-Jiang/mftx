import {useEffect, useRef} from "react";

import {
  discoverPeers,
  getAppState,
  onIncomingTransferExpired,
  onIncomingTransferRequested,
  onPeerDiscovered,
  onTrustChanged,
  onTransferFailed,
  onTransferFinished,
  onTransferProgress,
  onTransferStarted,
  startPeer,
} from "./lib/api";
import {isMac} from "./lib/platform";
import type {ActiveTransfer, TransferEvent} from "./lib/types";
import {useAppStore} from "./lib/use-app-store";
import {SetupScreen} from "./components/SetupScreen";
import {Shell} from "./components/Shell";
import {TransferDialog} from "./components/TransferDialog";
import {WindowControls} from "./components/WindowControls";
import {InboxPanel} from "./components/panels/InboxPanel";
import {PeersPanel} from "./components/panels/PeersPanel";
import {SettingsPanel} from "./components/panels/SettingsPanel";
import {TransferPanel} from "./components/panels/TransferPanel";

// Background discovery cadence. The core now caches the ARP table (~15s) so a
// sweep no longer re-spawns arp.exe, but a longer interval still trims overall
// process churn; fast peers surface immediately via the streamed
// peer-discovered events regardless of this cadence.
const DISCOVERY_INTERVAL_MS = 10000;

export default function App() {
  const state = useAppStore((store) => store.state);
  const selectedView = useAppStore((store) => store.selectedView);
  const autoStartPeer = useAppStore((store) => store.autoStartPeer);
  const setState = useAppStore((store) => store.setState);
  const setPeers = useAppStore((store) => store.setPeers);
  const addPeer = useAppStore((store) => store.addPeer);
  const setDiscovering = useAppStore((store) => store.setDiscovering);
  const setMessage = useAppStore((store) => store.setMessage);
  const addActivity = useAppStore((store) => store.addActivity);
  const addIncomingRequest = useAppStore((store) => store.addIncomingRequest);
  const removeIncomingRequest = useAppStore((store) => store.removeIncomingRequest);
  const beginTransfer = useAppStore((store) => store.beginTransfer);
  const updateTransfer = useAppStore((store) => store.updateTransfer);

  const setupComplete = Boolean(state?.setupComplete);
  const peerRunning = Boolean(state?.peerRunning);
  const autoStartedRef = useRef(false);

  useEffect(() => {
    getAppState()
      .then(setState)
      .catch((error) => setMessage(String(error)));
  }, [setMessage, setState]);

  // Auto-start the receiver once on launch (configurable in 设置) so the app is
  // discoverable and ready to receive without the user pressing 启动.
  useEffect(() => {
    if (!setupComplete || peerRunning || !autoStartPeer || autoStartedRef.current) return;
    autoStartedRef.current = true;
    startPeer()
      .then(setState)
      .catch(() => undefined);
  }, [setupComplete, peerRunning, autoStartPeer, setState]);

  // Discover on mount and then keep the peer list fresh in the background.
  useEffect(() => {
    if (!setupComplete) return;
    let cancelled = false;

    const sweep = async () => {
      setDiscovering(true);
      try {
        const peers = await discoverPeers(2);
        if (!cancelled) setPeers(peers);
      } catch {
        /* discovery best-effort */
      } finally {
        if (!cancelled) setDiscovering(false);
      }
    };

    void sweep();
    const id = window.setInterval(() => void sweep(), DISCOVERY_INTERVAL_MS);
    return () => {
      cancelled = true;
      window.clearInterval(id);
    };
  }, [setupComplete, setPeers, setDiscovering]);

  useEffect(() => {
    const toActiveTransfer = (event: TransferEvent): ActiveTransfer => ({
      id: event.id,
      direction: event.direction,
      peer: event.peer,
      status: "running",
      files: event.report?.files ?? null,
      totalBytes: event.report?.bytes ?? null,
      transferredBytes: 0,
      startedAt: Date.now(),
      finishedAt: null,
      message: null,
    });

    const unsubscribers = [
      onTransferStarted((event) => {
        addActivity(event, "running");
        beginTransfer(toActiveTransfer(event));
      }),
      onTransferProgress((progress) => {
        updateTransfer(progress.id, {
          transferredBytes: progress.transferred,
          totalBytes: progress.total > 0 ? progress.total : null,
        });
      }),
      onTransferFinished((event) => {
        addActivity(event, "finished");
        updateTransfer(event.id, {
          status: "finished",
          finishedAt: Date.now(),
          files: event.report?.files ?? null,
          totalBytes: event.report?.bytes ?? null,
          transferredBytes: event.report?.bytes ?? 0,
        });
      }),
      onTransferFailed((event) => {
        addActivity(event, "failed");
        updateTransfer(event.id, {
          status: "failed",
          finishedAt: Date.now(),
          message: event.message,
        });
      }),
      onIncomingTransferRequested((request) => {
        addIncomingRequest(request);
        addActivity(
          {
            id: request.id,
            direction: "incoming",
            peer: request.deviceName,
            paths: request.pathsPreview,
            report: null,
            message: "等待确认",
          },
          "running",
        );
      }),
      onIncomingTransferExpired((request) => {
        removeIncomingRequest(request.id);
        addActivity(
          {
            id: request.id,
            direction: "incoming",
            peer: request.deviceName,
            paths: request.pathsPreview,
            report: null,
            message: "请求已过期",
          },
          "expired",
        );
      }),
      onTrustChanged(() => {
        getAppState().then(setState).catch(() => undefined);
      }),
      onPeerDiscovered((peer) => addPeer(peer)),
    ];

    return () => {
      for (const unsubscribe of unsubscribers) {
        unsubscribe.then((cleanup) => cleanup());
      }
    };
  }, [
    addActivity,
    addIncomingRequest,
    removeIncomingRequest,
    beginTransfer,
    updateTransfer,
    addPeer,
    setState,
  ]);

  if (!state) {
    return (
      <div className="flex h-full flex-col bg-background">
        {!isMac && (
          <div className="drag-region flex h-9 shrink-0 items-center justify-end pr-1">
            <WindowControls />
          </div>
        )}
        <main className="flex min-h-0 flex-1 items-center justify-center">
          <div className="text-muted text-sm">正在加载 MFTX...</div>
        </main>
      </div>
    );
  }

  if (!state.setupComplete) {
    return <SetupScreen />;
  }

  return (
    <Shell>
      {selectedView === "transfer" && <TransferPanel />}
      {selectedView === "inbox" && <InboxPanel />}
      {selectedView === "peers" && <PeersPanel />}
      {selectedView === "settings" && <SettingsPanel />}
      <div className="mftx-transfer-dialog-anchor pointer-events-none">
        <TransferDialog />
      </div>
    </Shell>
  );
}
