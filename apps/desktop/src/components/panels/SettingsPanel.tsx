import {Button, Disclosure, Switch, Tooltip} from "@heroui/react";
import {ListView} from "@heroui-pro/react";
import {
  ChevronDown,
  FolderOpen,
  KeyRound,
  Monitor,
  Moon,
  Plus,
  Power,
  Radar,
  Save,
  ShieldCheck,
  Sun,
  SunMoon,
  Trash2,
  X,
} from "lucide-react";
import {useEffect, useState} from "react";

import {chooseDirectory, untrustDevice, updateSettings} from "../../lib/api";
import {listenAddrFromPort, listenPortFromAddr, validateListenPort} from "../../lib/listen";
import type {SettingsRequest, TrustedDevice} from "../../lib/types";
import {shortId} from "../../lib/format";
import {useAppStore} from "../../lib/use-app-store";
import {ErrorAlert, TextField} from "../ui";

export function SettingsPanel() {
  const state = useAppStore((store) => store.state);
  const setState = useAppStore((store) => store.setState);
  const autoStartPeer = useAppStore((store) => store.autoStartPeer);
  const setAutoStartPeer = useAppStore((store) => store.setAutoStartPeer);
  const themeMode = useAppStore((store) => store.themeMode);
  const setThemeMode = useAppStore((store) => store.setThemeMode);
  const busy = useAppStore((store) => store.busy);
  const setBusy = useAppStore((store) => store.setBusy);
  const message = useAppStore((store) => store.message);
  const setMessage = useAppStore((store) => store.setMessage);
  const [form, setForm] = useState<SettingsRequest>({});
  const [newTarget, setNewTarget] = useState("");
  const portError = form.listenAddr?.trim() ? validateListenPort(form.listenAddr) : null;
  const discoveryTargets = form.discoveryTargets ?? [];

  useEffect(() => {
    if (!state?.config) return;
    setForm({
      deviceName: state.config.deviceName,
      listenAddr: listenPortFromAddr(state.config.listenAddr),
      inboxDir: state.config.dirs.inboxDir,
      shareDir: state.config.dirs.shareDir,
      receivedDir: state.config.dirs.receivedDir,
      discoveryTargets: state.config.discoveryTargets ?? [],
      password: "",
    });
  }, [state?.config]);

  function addDiscoveryTarget() {
    const value = newTarget.trim();
    if (!value) return;
    setForm((current) => {
      const existing = current.discoveryTargets ?? [];
      if (existing.includes(value)) return current;
      return {...current, discoveryTargets: [...existing, value]};
    });
    setNewTarget("");
  }

  function removeDiscoveryTarget(target: string) {
    setForm((current) => ({
      ...current,
      discoveryTargets: (current.discoveryTargets ?? []).filter((item) => item !== target),
    }));
  }

  async function save() {
    setBusy(true);
    setMessage(null);
    try {
      const portError = validateListenPort(form.listenAddr ?? "");
      if (portError) throw new Error(portError);
      setState(
        await updateSettings({
          ...form,
          password: form.password?.trim() ? form.password : null,
          listenAddr: listenAddrFromPort(form.listenAddr ?? ""),
        }),
      );
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function removeTrusted(device: TrustedDevice) {
    setBusy(true);
    setMessage(null);
    try {
      setState(await untrustDevice(device.deviceId));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function pickDir(key: "inboxDir" | "shareDir" | "receivedDir", title: string) {
    const selected = await chooseDirectory(title);
    if (selected) setForm((current) => ({...current, [key]: selected}));
  }

  return (
    <div className="mx-auto flex w-full max-w-7xl flex-col gap-5 px-6 py-6">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div className="flex min-w-0 flex-col">
          <h1 className="text-xl font-semibold leading-7">配置</h1>
          <p className="text-sm text-muted">管理本机端口、目录和可信设备；端口变更后建议重启 peer。</p>
        </div>
        <Button isDisabled={busy || Boolean(portError)} variant="primary" onPress={save}>
          <Save className="size-4" />
          {busy ? "保存中..." : "保存"}
        </Button>
      </div>

      <section className="surface-card flex items-center justify-between gap-4 p-5">
        <div className="flex min-w-0 items-start gap-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-surface-secondary text-muted">
            <Power className="size-4" />
          </div>
          <div className="min-w-0">
            <h2 className="text-sm font-semibold">打开应用时自动启动接收</h2>
            <p className="text-xs text-muted">开启后无需手动点击“启动”，应用会在后台监听并可被局域网设备发现。</p>
          </div>
        </div>
        <Switch isSelected={autoStartPeer} onChange={setAutoStartPeer} aria-label="打开应用时自动启动接收" />
      </section>

      <section className="surface-card flex flex-wrap items-center justify-between gap-4 p-5">
        <div className="flex min-w-0 items-start gap-3">
          <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-surface-secondary text-muted">
            <SunMoon className="size-4" />
          </div>
          <div className="min-w-0">
            <h2 className="text-sm font-semibold">外观</h2>
            <p className="text-xs text-muted">跟随系统、浅色或深色。</p>
          </div>
        </div>
        <div className="mftx-segmented inline-flex shrink-0 rounded-xl bg-surface-secondary p-0.5">
          {(
            [
              ["auto", "自动", Monitor],
              ["light", "浅色", Sun],
              ["dark", "深色", Moon],
            ] as const
          ).map(([mode, label, Icon]) => (
            <button
              key={mode}
              type="button"
              aria-pressed={themeMode === mode}
              className="mftx-segment flex items-center gap-1.5 rounded-lg px-3 py-1.5 text-xs font-medium transition-colors"
              data-active={themeMode === mode ? "true" : "false"}
              onClick={() => setThemeMode(mode)}
            >
              <Icon className="size-3.5" />
              {label}
            </button>
          ))}
        </div>
      </section>

      <section className="grid grid-cols-1 gap-4 xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="surface-card p-5">
          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <TextField
              label="设备名称"
              value={form.deviceName ?? ""}
              onValueChange={(deviceName) => setForm((current) => ({...current, deviceName}))}
            />
            <TextField
              label="监听端口"
              inputMode="numeric"
              placeholder="48151"
              error={portError}
              value={form.listenAddr ?? ""}
              onValueChange={(listenAddr) => setForm((current) => ({...current, listenAddr}))}
            />
          </div>

          <Disclosure className="mt-5 rounded-2xl bg-surface-secondary">
            <Disclosure.Heading>
              <Disclosure.Trigger className="flex w-full items-center justify-between gap-3 px-4 py-3 text-left text-sm font-semibold">
                <span className="flex items-center gap-2">
                  <KeyRound className="size-4 text-muted" />
                  高级兼容
                </span>
                <Disclosure.Indicator>
                  <ChevronDown className="size-4" />
                </Disclosure.Indicator>
              </Disclosure.Trigger>
            </Disclosure.Heading>
            <Disclosure.Content>
              <Disclosure.Body className="grid gap-3 px-4 pb-4">
                <TextField
                  label="兼容传输密码"
                  placeholder="留空则不修改"
                  type="password"
                  value={form.password ?? ""}
                  onValueChange={(password) => setForm((current) => ({...current, password}))}
                />
                <p className="text-xs leading-5 text-muted">
                  仅用于旧 CLI 或远程中继兼容；桌面端局域网主流程不需要输入密码。
                </p>
              </Disclosure.Body>
            </Disclosure.Content>
          </Disclosure>
        </div>

        <div className="surface-card p-5">
          <h2 className="mb-3 text-sm font-semibold">配置位置</h2>
          <p className="path-text whitespace-normal break-all">{state?.config?.dirs.configPath ?? "-"}</p>
        </div>
      </section>

      <section className="surface-card p-5">
        <h2 className="mb-4 text-sm font-semibold">目录</h2>
        <div className="grid grid-cols-1 gap-4">
          <PathInput
            label="收件箱"
            value={form.inboxDir ?? ""}
            onPick={() => pickDir("inboxDir", "选择收件箱目录")}
            onValueChange={(inboxDir) => setForm((current) => ({...current, inboxDir}))}
          />
          <PathInput
            label="共享目录"
            value={form.shareDir ?? ""}
            onPick={() => pickDir("shareDir", "选择共享目录")}
            onValueChange={(shareDir) => setForm((current) => ({...current, shareDir}))}
          />
          <PathInput
            label="兼容拉取保存目录"
            value={form.receivedDir ?? ""}
            onPick={() => pickDir("receivedDir", "选择兼容拉取保存目录")}
            onValueChange={(receivedDir) => setForm((current) => ({...current, receivedDir}))}
          />
        </div>
      </section>

      <section className="surface-card p-5">
        <div className="mb-1 flex items-center gap-2">
          <Radar className="size-4 text-accent" />
          <h2 className="text-sm font-semibold">发现目标</h2>
        </div>
        <p className="mb-4 text-xs leading-5 text-muted">
          局域网广播无法穿透 Tailscale 等点对点叠加网络。添加对端的固定地址（如 Tailscale 的 100.x
          地址，可选带端口），即可在广播之外定向探测并发现该设备。留空则保持原有广播发现行为。
        </p>
        <div className="flex flex-col gap-2 sm:flex-row">
          <TextField
            className="min-w-0 flex-1"
            ariaLabel="发现目标地址"
            placeholder="100.64.0.2 或 100.64.0.2:48150"
            value={newTarget}
            onValueChange={setNewTarget}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                addDiscoveryTarget();
              }
            }}
          />
          <Button isDisabled={!newTarget.trim()} variant="outline" onPress={addDiscoveryTarget}>
            <Plus className="size-4" />
            添加
          </Button>
        </div>
        {discoveryTargets.length === 0 ? (
          <p className="mt-3 text-sm text-muted">还没有发现目标。</p>
        ) : (
          <ul className="mt-3 flex flex-col gap-2">
            {discoveryTargets.map((target) => (
              <li
                key={target}
                className="flex items-center justify-between gap-3 rounded-2xl bg-surface-secondary px-4 py-2"
              >
                <span className="path-text min-w-0 break-all">{target}</span>
                <Button
                  isIconOnly
                  aria-label={`移除 ${target}`}
                  size="sm"
                  variant="danger-soft"
                  onPress={() => removeDiscoveryTarget(target)}
                >
                  <X className="size-4" />
                </Button>
              </li>
            ))}
          </ul>
        )}
      </section>

      <section className="surface-card p-5">
        <div className="mb-4 flex items-center gap-2">
          <ShieldCheck className="size-4 text-success" />
          <h2 className="text-sm font-semibold">信任设备</h2>
        </div>
        {(state?.trustedDevices.length ?? 0) === 0 ? (
          <p className="text-sm text-muted">还没有信任设备。接收文件时勾选“信任此设备”后会出现在这里。</p>
        ) : (
          <ListView aria-label="信任设备" items={state?.trustedDevices ?? []} selectionMode="none" variant="primary">
            {(device) => (
              <ListView.Item id={device.deviceId} textValue={device.displayName}>
                <ListView.ItemContent>
                  <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-accent-soft text-accent">
                    <ShieldCheck className="size-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <ListView.Title>{device.displayName}</ListView.Title>
                    <ListView.Description>设备 {shortId(device.deviceId)}</ListView.Description>
                  </div>
                  <Tooltip>
                    <Tooltip.Trigger>
                      <Button
                        isIconOnly
                        aria-label={`移除 ${device.displayName}`}
                        size="sm"
                        variant="danger-soft"
                        onPress={() => removeTrusted(device)}
                      >
                        <Trash2 className="size-4" />
                      </Button>
                    </Tooltip.Trigger>
                    <Tooltip.Content>移除信任</Tooltip.Content>
                  </Tooltip>
                </ListView.ItemContent>
              </ListView.Item>
            )}
          </ListView>
        )}
        <ErrorAlert className="mt-4" message={message} />
      </section>
    </div>
  );
}

type PathInputProps = {
  label: string;
  value: string;
  onPick: () => void;
  onValueChange: (value: string) => void;
};

function PathInput({label, value, onPick, onValueChange}: PathInputProps) {
  return (
    <div className="flex gap-2">
      <TextField className="min-w-0 flex-1" label={label} value={value} onValueChange={onValueChange} />
      <Tooltip>
        <Tooltip.Trigger>
          <Button isIconOnly aria-label={`选择${label}`} className="mt-6" variant="outline" onPress={onPick}>
            <FolderOpen className="size-4" />
          </Button>
        </Tooltip.Trigger>
        <Tooltip.Content>{`选择${label}`}</Tooltip.Content>
      </Tooltip>
    </div>
  );
}
