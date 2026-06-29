import {Badge, Button, Tooltip} from "@heroui/react";
import {EmptyState, ListView} from "@heroui-pro/react";
import {Copy, RefreshCcw, RadioTower, Send, ShieldCheck, Wifi} from "lucide-react";

import {discoverPeers} from "../../lib/api";
import {shortId} from "../../lib/format";
import {listenPortFromAddr} from "../../lib/listen";
import {useAppStore} from "../../lib/use-app-store";

export function PeersPanel() {
  const state = useAppStore((store) => store.state);
  const peers = useAppStore((store) => store.peers);
  const setPeers = useAppStore((store) => store.setPeers);
  const setBusy = useAppStore((store) => store.setBusy);
  const busy = useAppStore((store) => store.busy);
  const setMessage = useAppStore((store) => store.setMessage);
  const setSelectedPeerAddr = useAppStore((store) => store.setSelectedPeerAddr);
  const setSelectedView = useAppStore((store) => store.setSelectedView);
  const listenPort = listenPortFromAddr(state?.localAddr ?? state?.config?.listenAddr);

  async function refresh() {
    setBusy(true);
    setMessage(null);
    try {
      setPeers(await discoverPeers(4));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function copyAddr(addr: string) {
    await navigator.clipboard.writeText(addr);
  }

  return (
    <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-6 py-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <h1 className="text-xl font-semibold leading-7">局域网设备</h1>
          <p className="text-sm text-muted">查看本机监听状态和局域网内发现到的 peer。</p>
        </div>
        <Button isDisabled={busy} variant="outline" onPress={refresh}>
          <RefreshCcw className="size-4" />
          发现设备
        </Button>
      </div>

      <section className="grid grid-cols-1 gap-4 lg:grid-cols-3">
        <div className="surface-card p-5">
          <div className="mb-3 flex items-center justify-between gap-3">
            <h2 className="text-sm font-semibold">本机</h2>
            <Badge color={state?.peerRunning ? "success" : "default"} variant="soft">
              {state?.peerRunning ? "在线" : "离线"}
            </Badge>
          </div>
          <dl className="grid gap-2 text-sm">
            <div className="flex justify-between gap-3">
              <dt className="text-muted">名称</dt>
              <dd className="truncate">{state?.config?.deviceName ?? "-"}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted">监听端口</dt>
              <dd className="truncate">{listenPort}</dd>
            </div>
            <div className="flex justify-between gap-3">
              <dt className="text-muted">共享</dt>
              <dd className="truncate">{state?.config?.dirs.shareDir ?? "-"}</dd>
            </div>
          </dl>
        </div>
        <div className="surface-card p-5">
          <div className="mb-3 flex items-center gap-2">
            <ShieldCheck className="size-4 text-success" />
            <h2 className="text-sm font-semibold">协议能力</h2>
          </div>
          <div className="flex flex-wrap gap-2">
            {["receive", "push", "pull", "encrypted", "blake3"].map((cap) => (
              <Badge key={cap} variant="soft">
                {cap}
              </Badge>
            ))}
          </div>
        </div>
        <div className="surface-card p-5">
          <h2 className="mb-3 text-sm font-semibold">发现结果</h2>
          <div className="text-2xl font-semibold">{peers.length}</div>
          <p className="text-sm text-muted">同网段响应 beacon/probe 的设备数量。</p>
        </div>
      </section>

      <section className="surface-card min-h-0 flex-1 p-5">
        {peers.length === 0 ? (
          <EmptyState>
            <EmptyState.Header>
              <EmptyState.Media variant="icon">
                <RadioTower className="size-5" />
              </EmptyState.Media>
              <EmptyState.Title>还没有发现设备</EmptyState.Title>
              <EmptyState.Description>
                确认两台设备在同一局域网、peer 已启动，并且防火墙允许 UDP 48150 和 TCP 48151。
              </EmptyState.Description>
            </EmptyState.Header>
            <EmptyState.Content>
              <Button variant="outline" onPress={refresh}>
                重新发现
              </Button>
            </EmptyState.Content>
          </EmptyState>
        ) : (
          <ListView aria-label="发现设备" items={peers} selectionMode="none" variant="primary">
            {(peer) => (
              <ListView.Item id={peer.addr ?? peer.sessionId} textValue={peer.deviceName}>
                <ListView.ItemContent>
                  <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl bg-accent-soft text-accent">
                    <Wifi className="size-4" />
                  </div>
                  <div className="flex min-w-0 flex-1 flex-col">
                    <ListView.Title>{peer.deviceName}</ListView.Title>
                    <ListView.Description>
                      {peer.addr ?? `unknown:${peer.port}`} · {shortId(peer.sessionId)}
                    </ListView.Description>
                  </div>
                  <div className="flex items-center gap-2">
                    {peer.addr && (
                      <>
                        <Tooltip>
                          <Tooltip.Trigger>
                            <Button
                              isIconOnly
                              aria-label="复制地址"
                              size="sm"
                              variant="outline"
                              onPress={() => copyAddr(peer.addr!)}
                            >
                              <Copy className="size-4" />
                            </Button>
                          </Tooltip.Trigger>
                          <Tooltip.Content>复制地址</Tooltip.Content>
                        </Tooltip>
                        <Button
                          size="sm"
                          onPress={() => {
                            setSelectedPeerAddr(peer.addr);
                            setSelectedView("transfer");
                          }}
                        >
                          <Send className="size-4" />
                          传输
                        </Button>
                      </>
                    )}
                  </div>
                </ListView.ItemContent>
              </ListView.Item>
            )}
          </ListView>
        )}
      </section>
    </div>
  );
}

