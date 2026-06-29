import {Badge, Button, Tooltip} from "@heroui/react";
import {EmptyState, ListView} from "@heroui-pro/react";
import {CheckCircle2, CircleAlert, Clock3, Trash2} from "lucide-react";

import {basename, formatBytes} from "../../lib/format";
import {useAppStore} from "../../lib/use-app-store";

export function ActivityPanel() {
  const activities = useAppStore((store) => store.activities);
  const clearActivities = useAppStore((store) => store.clearActivities);

  return (
    <aside className="flex h-full w-full max-w-[320px] flex-col bg-background">
      <div className="flex items-center justify-between px-5 py-4">
        <div className="min-w-0">
          <h2 className="text-sm font-semibold leading-5">活动</h2>
          <p className="text-xs text-muted">历史已保存，刷新不丢失</p>
        </div>
        {activities.length > 0 && (
          <Tooltip>
            <Tooltip.Trigger>
              <Button isIconOnly aria-label="清空历史" size="sm" variant="ghost" onPress={clearActivities}>
                <Trash2 className="size-4" />
              </Button>
            </Tooltip.Trigger>
            <Tooltip.Content>清空历史</Tooltip.Content>
          </Tooltip>
        )}
      </div>

      <div className="min-h-0 flex-1 overflow-auto px-3 pb-4">
        {activities.length === 0 ? (
          <div className="surface-card mx-2 mt-2 flex min-h-[280px] items-center justify-center p-5">
            <EmptyState size="sm">
              <EmptyState.Header>
                <EmptyState.Media variant="icon">
                  <Clock3 className="size-5" />
                </EmptyState.Media>
                <EmptyState.Title>暂无传输</EmptyState.Title>
                <EmptyState.Description>任务开始后会出现在这里。</EmptyState.Description>
              </EmptyState.Header>
            </EmptyState>
          </div>
        ) : (
          <ListView aria-label="传输活动" items={activities} selectionMode="none" variant="primary">
            {(activity) => (
              <ListView.Item id={activity.id} textValue={activity.peer}>
                <ListView.ItemContent>
                  <StatusIcon status={activity.status} />
                  <div className="flex min-w-0 flex-1 flex-col">
                    <div className="flex min-w-0 items-center gap-2">
                      <span className="min-w-0 flex-1 truncate text-sm font-medium leading-5">
                        {directionLabel(activity.direction)} {activity.peer}
                      </span>
                      <StatusBadge status={activity.status} />
                    </div>
                    <ListView.Description>
                      {activity.report
                        ? `${activity.report.files} 个文件 · ${formatBytes(activity.report.bytes)}`
                        : activity.paths.map(basename).join(", ")}
                    </ListView.Description>
                    {activity.message && <span className="mt-1 truncate text-xs text-danger">{activity.message}</span>}
                  </div>
                </ListView.ItemContent>
              </ListView.Item>
            )}
          </ListView>
        )}
      </div>
    </aside>
  );
}

function directionLabel(direction: "push" | "pull" | "incoming") {
  if (direction === "push") return "发送";
  if (direction === "incoming") return "接收";
  return "兼容拉取";
}

function StatusIcon({status}: {status: "running" | "finished" | "failed" | "expired" | "rejected"}) {
  if (status === "failed" || status === "rejected" || status === "expired") {
    return <CircleAlert className="size-5 text-danger" />;
  }
  if (status === "finished") return <CheckCircle2 className="size-5 text-success" />;
  return <Clock3 className="size-5 text-warning" />;
}

function StatusBadge({status}: {status: "running" | "finished" | "failed" | "expired" | "rejected"}) {
  const failed = status === "failed" || status === "expired" || status === "rejected";
  return (
    <Badge
      className="shrink-0 self-center"
      color={failed ? "danger" : status === "finished" ? "success" : "warning"}
      size="sm"
      variant="soft"
    >
      {status === "failed"
        ? "失败"
        : status === "expired"
          ? "过期"
          : status === "rejected"
            ? "已拒绝"
            : status === "finished"
              ? "完成"
              : "进行中"}
    </Badge>
  );
}
