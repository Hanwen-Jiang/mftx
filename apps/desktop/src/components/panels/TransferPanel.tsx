import {Badge, Button, Tooltip} from "@heroui/react";
import {DropZone} from "@heroui-pro/react";
import {CheckCircle2, Link2, Plug, Radar, RefreshCcw, Send, Trash2, UploadCloud, Wifi} from "lucide-react";
import {useState} from "react";

import {
  chooseTransferDirectories,
  chooseTransferPaths,
  connectPeer,
  discoverPeers,
  sendPaths,
} from "../../lib/api";
import {basename, shortId} from "../../lib/format";
import type {Peer} from "../../lib/types";
import {useAppStore} from "../../lib/use-app-store";
import {ErrorAlert} from "../ui";

export function TransferPanel() {
  const [paths, setPaths] = useState<string[]>([]);
  const [connectAddr, setConnectAddr] = useState("");
  const [connecting, setConnecting] = useState(false);
  const peers = useAppStore((store) => store.peers);
  const selectedPeerAddr = useAppStore((store) => store.selectedPeerAddr);
  const setSelectedPeerAddr = useAppStore((store) => store.setSelectedPeerAddr);
  const setPeers = useAppStore((store) => store.setPeers);
  const setState = useAppStore((store) => store.setState);
  const busy = useAppStore((store) => store.busy);
  const setBusy = useAppStore((store) => store.setBusy);
  const discovering = useAppStore((store) => store.discovering);
  const setDiscovering = useAppStore((store) => store.setDiscovering);
  const message = useAppStore((store) => store.message);
  const setMessage = useAppStore((store) => store.setMessage);

  // Only ever send to a verified, discovered peer — never to a raw typed address.
  const selectedPeer = peers.find((peer) => peer.addr === selectedPeerAddr) ?? null;
  const canSend = Boolean(selectedPeerAddr) && paths.length > 0 && !busy;

  async function pickPaths() {
    const selected = await chooseTransferPaths();
    if (selected.length > 0) {
      setPaths((current) => [...selected, ...current.filter((path) => !selected.includes(path))]);
    }
  }

  async function pickDirectories() {
    const selected = await chooseTransferDirectories();
    if (selected.length > 0) {
      setPaths((current) => [...selected, ...current.filter((path) => !selected.includes(path))]);
    }
  }

  async function refreshPeers() {
    setDiscovering(true);
    setMessage(null);
    try {
      setPeers(await discoverPeers(3));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setDiscovering(false);
    }
  }

  // "Connect" = establish/verify a peer by address, NOT a blind send. We persist
  // the address as a discovery target; the running peer probes it and, once it
  // answers, it appears in the list above as a reachable device to send to.
  async function runConnect() {
    const addr = connectAddr.trim();
    if (!addr) return;
    setConnecting(true);
    setMessage(null);
    try {
      // Discovery probes the well-known port; the host is what matters. Strip any
      // typed port so the verified peer comes back with its real transfer address.
      const host = addr.replace(/:\d+$/, "");
      setState(await connectPeer(host));
      setConnectAddr("");
      setDiscovering(true);
      try {
        setPeers(await discoverPeers(2));
      } finally {
        setDiscovering(false);
      }
      setMessage(`已向 ${host} 发起连接；对方在线时会出现在上面的设备列表里。`);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setConnecting(false);
    }
  }

  async function runSend() {
    if (!selectedPeerAddr) return;
    setBusy(true);
    setMessage(null);
    try {
      await sendPaths({addr: selectedPeerAddr, paths});
      setPaths([]);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-5 px-6 py-6">
        {/* Choose / connect a verified device */}
        <section className="surface-card flex flex-col p-5">
          <div className="flex items-center justify-between gap-3">
            <div className="flex items-center gap-2">
              <Wifi className="size-4 text-muted" />
              <h2 className="text-sm font-semibold">设备</h2>
              {discovering && (
                <span className="mftx-scan flex items-center gap-1.5 text-xs text-accent">
                  <Radar className="size-3.5 mftx-scan-icon" />
                  探测中
                </span>
              )}
            </div>
            <Tooltip>
              <Tooltip.Trigger>
                <Button
                  isIconOnly
                  aria-label="重新探测设备"
                  isDisabled={discovering}
                  size="sm"
                  variant="ghost"
                  onPress={refreshPeers}
                >
                  <RefreshCcw className={`size-4 ${discovering ? "mftx-spin" : ""}`} />
                </Button>
              </Tooltip.Trigger>
              <Tooltip.Content>重新探测设备</Tooltip.Content>
            </Tooltip>
          </div>

          {peers.length === 0 ? (
            <div className="mt-3 flex min-h-[96px] flex-col items-center justify-center gap-1 text-center">
              <p className="text-sm text-muted">
                {discovering ? "正在探测设备…" : "暂无设备，可在下方输入对方地址连接。"}
              </p>
            </div>
          ) : (
            <div className="mt-3 flex max-h-[40vh] flex-col gap-1.5 overflow-y-auto pr-1">
              {peers.map((peer) => (
                <PeerRow
                  key={peer.addr ?? peer.sessionId}
                  peer={peer}
                  selected={peer.addr != null && peer.addr === selectedPeerAddr}
                  onSelect={() => peer.addr && setSelectedPeerAddr(peer.addr)}
                />
              ))}
            </div>
          )}

          {/* Establish a connection by address (verified, not blind send). */}
          <div className="mt-4 flex flex-col gap-2 border-t border-separator pt-4">
            <div className="flex items-center gap-2 text-xs font-medium text-muted">
              <Plug className="size-3.5" />
              连接设备(输入对方地址)
            </div>
            <div className="flex gap-2">
              <input
                aria-label="对方地址"
                className="mftx-connect-input min-w-0 flex-1 rounded-xl border border-border bg-surface px-3 py-2 text-sm outline-none transition-colors focus:border-accent"
                placeholder="192.168.1.12 或 100.x.x.x:48151"
                value={connectAddr}
                onChange={(event) => setConnectAddr(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === "Enter") void runConnect();
                }}
              />
              <Button
                isDisabled={connecting || !connectAddr.trim()}
                variant="outline"
                onPress={runConnect}
              >
                {connecting ? "连接中…" : "连接"}
              </Button>
            </div>
            <p className="text-xs leading-5 text-muted">
              先建立连接验证对方可达,成功后它会出现在上面的列表,再选中发送 —— 不会盲发到一个写错的地址。
            </p>
          </div>
        </section>

        {/* RIGHT: content + send */}
        <section className="surface-card flex flex-col p-5">
          <div className="flex items-center justify-between gap-2">
            <div className="flex items-center gap-2">
              <UploadCloud className="size-4 text-muted" />
              <h2 className="text-sm font-semibold">待发送内容</h2>
              {paths.length > 0 && (
                <Badge size="sm" variant="soft">
                  {paths.length}
                </Badge>
              )}
            </div>
            {paths.length > 0 && (
              <Tooltip>
                <Tooltip.Trigger>
                  <Button isIconOnly aria-label="清空待发送列表" size="sm" variant="ghost" onPress={() => setPaths([])}>
                    <Trash2 className="size-4" />
                  </Button>
                </Tooltip.Trigger>
                <Tooltip.Content>清空待发送列表</Tooltip.Content>
              </Tooltip>
            )}
          </div>

          <DropZone className="mt-3">
            <DropZone.Area className="min-h-0 px-3 py-4" onDrop={() => void pickPaths()}>
              <DropZone.Icon />
              <DropZone.Label>添加文件或文件夹</DropZone.Label>
              <div className="flex flex-wrap justify-center gap-2">
                <DropZone.Trigger onPress={pickPaths}>选择文件</DropZone.Trigger>
                <DropZone.Trigger onPress={pickDirectories}>选择文件夹</DropZone.Trigger>
              </div>
            </DropZone.Area>
            {paths.length > 0 && (
              <DropZone.FileList className="max-h-56 overflow-y-auto">
                {paths.map((path) => (
                  <DropZone.FileItem key={path} status="complete">
                    <DropZone.FileFormatIcon color="blue" format="FILE" />
                    <DropZone.FileInfo>
                      <DropZone.FileName>{basename(path)}</DropZone.FileName>
                      <DropZone.FileMeta>{path}</DropZone.FileMeta>
                    </DropZone.FileInfo>
                    <DropZone.FileRemoveTrigger
                      aria-label={`移除 ${basename(path)}`}
                      onPress={() => setPaths((current) => current.filter((item) => item !== path))}
                    />
                  </DropZone.FileItem>
                ))}
              </DropZone.FileList>
            )}
          </DropZone>

          <div className="mt-3 flex items-start gap-2 text-xs leading-5 text-muted">
            <Link2 className="mt-0.5 size-3.5 shrink-0" />
            <span>
              {selectedPeer
                ? `将发送给 ${selectedPeer.deviceName}（${selectedPeer.addr}）；对方确认后才会写入收件箱。`
                : "请先在左侧选择/连接一个设备。"}
            </span>
          </div>

          <ErrorAlert className="mt-4" message={message} />

          <Button className="mt-4 w-full justify-center" isDisabled={!canSend} variant="primary" onPress={runSend}>
            <Send className="size-4" />
            {busy
              ? "处理中…"
              : !selectedPeerAddr
                ? "请选择设备"
                : paths.length === 0
                  ? "请添加文件"
                  : "开始发送"}
          </Button>
        </section>
    </div>
  );
}

type PeerRowProps = {
  peer: Peer;
  selected: boolean;
  onSelect: () => void;
};

function PeerRow({peer, selected, onSelect}: PeerRowProps) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      className="mftx-peer-row flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors"
      data-selected={selected ? "true" : "false"}
      onClick={onSelect}
    >
      <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-accent-soft text-accent">
        <Wifi className="size-4" />
      </div>
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{peer.deviceName}</span>
        <span className="truncate text-xs text-muted">
          {peer.addr ?? `unknown:${peer.port}`} · {shortId(peer.sessionId)}
        </span>
      </div>
      {selected ? (
        <CheckCircle2 className="size-4 shrink-0 text-accent" />
      ) : (
        <Badge size="sm" variant="soft">
          v{peer.version}
        </Badge>
      )}
    </button>
  );
}
