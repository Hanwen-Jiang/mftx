import {Badge, Button, Checkbox, Tooltip} from "@heroui/react";
import {EmptyState} from "@heroui-pro/react";
import {
  Check,
  CheckCircle2,
  ExternalLink,
  File as FileIcon,
  Folder,
  FolderOpen,
  Inbox,
  RefreshCcw,
  ShieldCheck,
  X,
} from "lucide-react";
import {useCallback, useEffect, useMemo, useState} from "react";

import {listInbox, openInbox, respondIncomingTransfer, revealPath} from "../../lib/api";
import {formatBytes, shortId} from "../../lib/format";
import type {ActiveTransfer, InboxEntry, IncomingTransferRequest} from "../../lib/types";
import {useAppStore} from "../../lib/use-app-store";

export function InboxPanel() {
  const [trustByRequest, setTrustByRequest] = useState<Record<string, boolean>>({});
  const [entries, setEntries] = useState<InboxEntry[]>([]);
  const incomingRequests = useAppStore((store) => store.incomingRequests);
  const activities = useAppStore((store) => store.activities);
  const removeIncomingRequest = useAppStore((store) => store.removeIncomingRequest);
  const beginTransfer = useAppStore((store) => store.beginTransfer);
  const setMessage = useAppStore((store) => store.setMessage);

  // Re-list the inbox whenever an incoming transfer completes.
  const completedIncoming = useMemo(
    () =>
      activities.filter(
        (activity) => activity.direction === "incoming" && activity.status === "finished",
      ).length,
    [activities],
  );

  const refreshInbox = useCallback(() => {
    listInbox()
      .then(setEntries)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    refreshInbox();
  }, [refreshInbox, completedIncoming]);

  async function decideIncoming(request: IncomingTransferRequest, accepted: boolean) {
    const trustDevice = Boolean(trustByRequest[request.id]);
    removeIncomingRequest(request.id);
    setTrustByRequest((current) => {
      const {[request.id]: _removed, ...next} = current;
      return next;
    });
    if (accepted) {
      const transfer: ActiveTransfer = {
        id: request.id,
        direction: "incoming",
        peer: request.deviceName,
        status: "running",
        files: request.files,
        totalBytes: request.bytes,
        transferredBytes: 0,
        startedAt: Date.now(),
        finishedAt: null,
        message: null,
      };
      beginTransfer(transfer);
    }
    try {
      await respondIncomingTransfer({id: request.id, accepted, trustDevice});
    } catch (error) {
      setMessage(String(error));
    }
  }

  return (
    <div className="mx-auto flex w-full max-w-3xl flex-col gap-5 px-6 py-6">
      {/* Pending incoming requests */}
      {incomingRequests.length > 0 && (
        <section className="surface-card flex min-h-0 flex-col p-5">
          <div className="flex items-center gap-2">
            <Inbox className="size-4 text-muted" />
            <h2 className="text-sm font-semibold">待确认接收</h2>
            <Badge color="warning" size="sm" variant="soft">
              {incomingRequests.length}
            </Badge>
          </div>
          <div className="mt-4 flex max-h-[50vh] flex-col gap-3 overflow-y-auto pr-1">
            {incomingRequests.map((request) => (
              <RequestCard
                key={request.id}
                request={request}
                trusted={Boolean(trustByRequest[request.id])}
                onTrustChange={(value) =>
                  setTrustByRequest((current) => ({...current, [request.id]: value}))
                }
                onDecide={(accepted) => decideIncoming(request, accepted)}
              />
            ))}
          </div>
        </section>
      )}

      {/* Inbox file list */}
      <section className="surface-card flex min-h-0 flex-col p-5">
        <div className="flex items-center justify-between gap-3">
          <div className="flex items-center gap-2">
            <Inbox className="size-4 text-muted" />
            <h2 className="text-sm font-semibold">收件箱</h2>
            {entries.length > 0 && (
              <Badge size="sm" variant="soft">
                {entries.length}
              </Badge>
            )}
          </div>
          <div className="flex items-center gap-1">
            <Tooltip>
              <Tooltip.Trigger>
                <Button isIconOnly aria-label="刷新收件箱" size="sm" variant="ghost" onPress={refreshInbox}>
                  <RefreshCcw className="size-4" />
                </Button>
              </Tooltip.Trigger>
              <Tooltip.Content>刷新</Tooltip.Content>
            </Tooltip>
            <Button size="sm" variant="outline" onPress={openInbox}>
              <FolderOpen className="size-4" />
              打开文件夹
            </Button>
          </div>
        </div>

        {entries.length === 0 ? (
          <div className="mt-4 flex min-h-[200px] items-center justify-center">
            <EmptyState size="sm">
              <EmptyState.Header>
                <EmptyState.Media variant="icon">
                  <Inbox className="size-5" />
                </EmptyState.Media>
                <EmptyState.Title>收件箱是空的</EmptyState.Title>
                <EmptyState.Description>接收到的文件会出现在这里。</EmptyState.Description>
              </EmptyState.Header>
            </EmptyState>
          </div>
        ) : (
          <div className="mt-3 flex max-h-[60vh] flex-col gap-1 overflow-y-auto pr-1">
            {entries.map((entry) => (
              <InboxRow key={entry.path} entry={entry} onOpen={() => void revealPath(entry.path)} />
            ))}
          </div>
        )}
      </section>
    </div>
  );
}

function InboxRow({entry, onOpen}: {entry: InboxEntry; onOpen: () => void}) {
  return (
    <button
      type="button"
      aria-label={`打开 ${entry.name}`}
      className="mftx-peer-row group flex w-full items-center gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors"
      onClick={onOpen}
    >
      <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-surface-secondary text-muted">
        {entry.isDir ? <Folder className="size-4" /> : <FileIcon className="size-4" />}
      </div>
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-sm font-medium">{entry.name}</span>
        <span className="truncate text-xs text-muted">
          {entry.isDir ? "文件夹" : formatBytes(entry.size)} · {formatDate(entry.modifiedMs)}
        </span>
      </div>
      <ExternalLink className="size-4 shrink-0 text-muted opacity-0 transition-opacity group-hover:opacity-100" />
    </button>
  );
}

function formatDate(ms: number): string {
  if (!ms) return "";
  try {
    return new Date(ms).toLocaleString();
  } catch {
    return "";
  }
}

type RequestCardProps = {
  request: IncomingTransferRequest;
  trusted: boolean;
  onTrustChange: (value: boolean) => void;
  onDecide: (accepted: boolean) => void;
};

function RequestCard({request, trusted, onTrustChange, onDecide}: RequestCardProps) {
  return (
    <div className="subtle-card flex flex-col gap-3 p-4">
      <div className="flex items-start gap-3">
        <div className="flex size-10 shrink-0 items-center justify-center rounded-2xl bg-accent-soft text-accent">
          <Inbox className="size-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="truncate text-sm font-semibold text-foreground">{request.deviceName}</div>
          <div className="truncate text-xs text-muted">
            {request.files} 个文件 · {formatBytes(request.bytes)} · {shortId(request.deviceId)}
          </div>
        </div>
      </div>

      {request.pathsPreview.length > 0 && (
        <div className="grid gap-1 px-1">
          {request.pathsPreview.slice(0, 3).map((path) => (
            <div key={path} className="path-text whitespace-normal break-all">
              {path}
            </div>
          ))}
        </div>
      )}

      <Checkbox aria-label="信任此设备" isSelected={trusted} onChange={onTrustChange}>
        <Checkbox.Content className="items-center gap-2 px-1 text-sm">
          <Checkbox.Control>
            <Checkbox.Indicator>
              <Check className="size-3" />
            </Checkbox.Indicator>
          </Checkbox.Control>
          <span className="flex items-center gap-1.5">
            <ShieldCheck className="size-4 text-muted" />
            信任此设备，下次自动接收
          </span>
        </Checkbox.Content>
      </Checkbox>

      <div className="grid grid-cols-2 gap-2">
        <Button variant="outline" onPress={() => onDecide(false)}>
          <X className="size-4" />
          拒绝
        </Button>
        <Tooltip>
          <Tooltip.Trigger>
            <Button variant="primary" onPress={() => onDecide(true)}>
              <CheckCircle2 className="size-4" />
              接收
            </Button>
          </Tooltip.Trigger>
          <Tooltip.Content>接收后写入收件箱</Tooltip.Content>
        </Tooltip>
      </div>
    </div>
  );
}
