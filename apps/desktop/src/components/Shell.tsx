import {Button, Tooltip} from "@heroui/react";
import {AppLayout, Navbar, Sidebar} from "@heroui-pro/react";
import {Inbox, PanelRight, Power, RadioTower, Send, Settings, UsersRound} from "lucide-react";
import type {ReactNode} from "react";

import {startPeer, stopPeer} from "../lib/api";
import {isMac} from "../lib/platform";
import {useAppStore} from "../lib/use-app-store";
import {ActivityPanel} from "./panels/ActivityPanel";
import {WindowControls, toggleMaximizeWindow} from "./WindowControls";

type ShellProps = {
  children: ReactNode;
};

const navItems = [
  {id: "transfer", label: "发送", icon: Send},
  {id: "inbox", label: "收件箱", icon: Inbox},
  {id: "peers", label: "设备", icon: UsersRound},
  {id: "settings", label: "设置", icon: Settings},
] as const;

const viewTitles = {
  transfer: "发送",
  inbox: "收件箱",
  peers: "设备",
  settings: "设置",
} as const;

export function Shell({children}: ShellProps) {
  const state = useAppStore((store) => store.state);
  const selectedView = useAppStore((store) => store.selectedView);
  const setSelectedView = useAppStore((store) => store.setSelectedView);
  const setState = useAppStore((store) => store.setState);
  const setMessage = useAppStore((store) => store.setMessage);
  const busy = useAppStore((store) => store.busy);
  const setBusy = useAppStore((store) => store.setBusy);
  const incomingCount = useAppStore((store) => store.incomingRequests.length);
  const running = Boolean(state?.peerRunning);

  async function togglePeer() {
    setBusy(true);
    setMessage(null);
    try {
      setState(running ? await stopPeer() : await startPeer());
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  const menu = (idPrefix: string) => (
    <Sidebar.Menu>
      {navItems.map((item) => {
        const Icon = item.icon;
        const showBadge = item.id === "inbox" && incomingCount > 0;
        return (
          <Sidebar.MenuItem
            key={item.id}
            id={`${idPrefix}${item.id}`}
            isCurrent={selectedView === item.id}
            textValue={item.label}
            onAction={() => setSelectedView(item.id)}
          >
            <Sidebar.MenuIcon>
              <Icon className="size-4" />
            </Sidebar.MenuIcon>
            <Sidebar.MenuLabel>{item.label}</Sidebar.MenuLabel>
            {showBadge && <span className="mftx-nav-badge" data-sidebar="label">{incomingCount}</span>}
          </Sidebar.MenuItem>
        );
      })}
    </Sidebar.Menu>
  );

  const sidebar = (
    <>
      <Sidebar>
        <Sidebar.Header>
          <div
            className={`mftx-sidebar-brand flex items-center gap-3 px-2 pb-2 ${isMac ? "pt-9" : "pt-2"}`}
            data-tauri-drag-region={isMac ? "" : undefined}
          >
            <div className="mftx-sidebar-brand-icon flex size-9 shrink-0 items-center justify-center rounded-2xl bg-accent text-accent-foreground shadow-surface">
              <RadioTower className="size-4" />
            </div>
            <div className="mftx-sidebar-brand-label flex min-w-0 flex-col" data-sidebar="label">
              <span className="truncate text-sm font-semibold leading-5">MFTX</span>
              <span className="truncate text-xs text-muted">{state?.config?.deviceName ?? "Desktop"}</span>
            </div>
          </div>
        </Sidebar.Header>
        <Sidebar.Content>
          <Sidebar.Group>
            <Sidebar.GroupLabel>工作区</Sidebar.GroupLabel>
            {menu("")}
          </Sidebar.Group>
        </Sidebar.Content>
        <Sidebar.Footer>
          <div className="mftx-sidebar-footer flex flex-col gap-2 px-2 pb-3">
            <div className="mftx-sidebar-status flex items-center gap-2 rounded-xl px-2.5 py-2">
              <span className="mftx-sidebar-status-dot" data-running={running ? "true" : "false"} />
              <span className="min-w-0 flex-1 truncate text-xs text-muted" data-sidebar="label">
                {running ? "接收已开启" : "接收未开启"}
              </span>
            </div>
          </div>
        </Sidebar.Footer>
        <Sidebar.Rail />
      </Sidebar>
      <Sidebar.Mobile>
        <Sidebar>
          <Sidebar.Header>
            <div className="flex items-center gap-3 px-2 py-2">
              <div className="flex size-9 items-center justify-center rounded-2xl bg-accent text-accent-foreground">
                <RadioTower className="size-4" />
              </div>
              <span className="text-sm font-semibold">MFTX</span>
            </div>
          </Sidebar.Header>
          <Sidebar.Content>{menu("mobile-")}</Sidebar.Content>
        </Sidebar>
      </Sidebar.Mobile>
    </>
  );

  const navbar = (
    <Navbar maxWidth="full" position="sticky">
      <Navbar.Header
        className="mftx-titlebar drag-region border-b border-separator bg-surface pl-3 pr-1"
        data-tauri-drag-region={isMac ? "" : undefined}
        onDoubleClick={(event) => {
          if (!(event.target as HTMLElement).closest("button, a, input")) toggleMaximizeWindow();
        }}
      >
        <AppLayout.MenuToggle className="no-drag" />
        <Sidebar.Trigger className="no-drag" />
        <span className="truncate text-sm font-semibold">{viewTitles[selectedView]}</span>
        <Navbar.Spacer />
        <Navbar.Content className="no-drag gap-1">
          <Tooltip>
            <Tooltip.Trigger>
              <Button
                aria-label={running ? "停止接收" : "启动接收"}
                className="mftx-peer-toggle"
                isDisabled={busy}
                size="sm"
                variant={running ? "danger-soft" : "primary"}
                onPress={togglePeer}
              >
                <Power className="size-3.5" />
                {running ? "停止" : "启动"}
              </Button>
            </Tooltip.Trigger>
            <Tooltip.Content>{running ? "停止接收 peer" : "启动接收 peer"}</Tooltip.Content>
          </Tooltip>
          <Tooltip>
            <Tooltip.Trigger>
              <AppLayout.AsideTrigger aria-label="显示活动栏" className="mftx-aside-trigger">
                <PanelRight className="size-4" />
              </AppLayout.AsideTrigger>
            </Tooltip.Trigger>
            <Tooltip.Content>显示活动栏</Tooltip.Content>
          </Tooltip>
        </Navbar.Content>
        {!isMac && (
          <div className="no-drag ml-1 flex items-center self-center">
            <WindowControls />
          </div>
        )}
      </Navbar.Header>
    </Navbar>
  );

  return (
    <AppLayout
      aside={<ActivityPanel />}
      defaultAsideOpen={false}
      defaultSidebarOpen
      navbar={navbar}
      sidebar={sidebar}
      sidebarCollapsible="icon"
    >
      <div className="mftx-content-scroll h-full overflow-y-auto">{children}</div>
    </AppLayout>
  );
}
