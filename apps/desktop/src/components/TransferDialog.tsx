import {Button} from "@heroui/react";
import {ArrowDownToLine, ArrowUpFromLine, CheckCircle2, CircleAlert, X} from "lucide-react";
import {useEffect, useState} from "react";

import {formatBytes} from "../lib/format";
import type {ActiveTransfer} from "../lib/types";
import {useAppStore} from "../lib/use-app-store";

/** Re-render once a second so the elapsed time / speed readout stays live. */
function useTick(active: boolean) {
  const [, setTick] = useState(0);
  useEffect(() => {
    if (!active) return;
    const id = window.setInterval(() => setTick((value) => value + 1), 1000);
    return () => window.clearInterval(id);
  }, [active]);
}

export function TransferDialog() {
  const transfer = useAppStore((store) => store.activeTransfer);
  const dismissTransfer = useAppStore((store) => store.dismissTransfer);
  useTick(transfer?.status === "running");

  // Auto-dismiss a finished/failed card after a short grace period.
  useEffect(() => {
    if (!transfer || transfer.status === "running") return;
    const id = window.setTimeout(() => dismissTransfer(), 4000);
    return () => window.clearTimeout(id);
  }, [transfer, dismissTransfer]);

  if (!transfer) return null;

  const elapsedMs = (transfer.finishedAt ?? Date.now()) - transfer.startedAt;
  const elapsedSec = Math.max(elapsedMs / 1000, 0.001);
  const speed = transfer.transferredBytes > 0 ? transfer.transferredBytes / elapsedSec : 0;
  const ratio =
    transfer.totalBytes && transfer.totalBytes > 0
      ? Math.min(transfer.transferredBytes / transfer.totalBytes, 1)
      : null;
  const determinate = transfer.status === "running" && ratio !== null && transfer.transferredBytes > 0;

  return (
    <div className="mftx-transfer-dialog surface-card pointer-events-auto w-[320px] p-4">
      <div className="flex items-start gap-3">
        <StatusGlyph transfer={transfer} />
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <span className="truncate text-sm font-semibold">{transfer.peer}</span>
            <Button
              isIconOnly
              aria-label="关闭"
              className="-mr-1 -mt-1"
              size="sm"
              variant="ghost"
              onPress={dismissTransfer}
            >
              <X className="size-4" />
            </Button>
          </div>
          <p className="truncate text-xs text-muted">{statusLine(transfer)}</p>
        </div>
      </div>

      <div
        className="mftx-progress-track mt-3"
        data-state={transfer.status}
        data-determinate={determinate ? "true" : "false"}
      >
        <div
          className="mftx-progress-fill"
          style={determinate ? {width: `${Math.round((ratio ?? 0) * 100)}%`} : undefined}
        />
      </div>

      <div className="mt-2 flex items-center justify-between text-xs text-muted">
        <span>
          {transfer.files != null ? `${transfer.files} 个文件` : ""}
          {transfer.totalBytes ? ` · ${formatBytes(transfer.totalBytes)}` : ""}
        </span>
        <span>
          {transfer.status === "running" && speed > 0
            ? `${formatBytes(speed)}/s`
            : transfer.status === "finished"
              ? "已完成"
              : transfer.status === "failed"
                ? "失败"
                : "准备中…"}
        </span>
      </div>
    </div>
  );
}

function StatusGlyph({transfer}: {transfer: ActiveTransfer}) {
  if (transfer.status === "failed") {
    return (
      <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-danger-soft text-danger">
        <CircleAlert className="size-4" />
      </div>
    );
  }
  if (transfer.status === "finished") {
    return (
      <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-success-soft text-success">
        <CheckCircle2 className="size-4" />
      </div>
    );
  }
  return (
    <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-accent-soft text-accent">
      {transfer.direction === "incoming" ? (
        <ArrowDownToLine className="size-4" />
      ) : (
        <ArrowUpFromLine className="size-4" />
      )}
    </div>
  );
}

function statusLine(transfer: ActiveTransfer): string {
  const verb = transfer.direction === "incoming" ? "接收" : "发送";
  if (transfer.status === "running") return `正在${verb}…`;
  if (transfer.status === "finished") return `${verb}完成`;
  return transfer.message ?? `${verb}失败`;
}
