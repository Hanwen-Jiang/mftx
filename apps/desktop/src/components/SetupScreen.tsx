import {Button, Form} from "@heroui/react";
import {FolderOpen, RadioTower, ShieldCheck, Wifi} from "lucide-react";
import {useEffect, useState} from "react";

import {chooseDirectory, completeSetup, getDefaultSetup} from "../lib/api";
import {listenAddrFromPort, listenPortFromAddr, validateListenPort} from "../lib/listen";
import type {SetupRequest} from "../lib/types";
import {isMac} from "../lib/platform";
import {useAppStore} from "../lib/use-app-store";
import {ErrorAlert, TextField} from "./ui";
import {WindowControls} from "./WindowControls";

export function SetupScreen() {
  const setState = useAppStore((store) => store.setState);
  const setBusy = useAppStore((store) => store.setBusy);
  const setMessage = useAppStore((store) => store.setMessage);
  const busy = useAppStore((store) => store.busy);
  const message = useAppStore((store) => store.message);
  const [form, setForm] = useState<SetupRequest>({
    deviceName: "",
    baseDir: "",
    listenAddr: "48151",
    password: "",
  });
  const portError = form.listenAddr?.trim() ? validateListenPort(form.listenAddr) : null;

  useEffect(() => {
    getDefaultSetup()
      .then((defaults) =>
        setForm((current) => ({
          ...current,
          ...defaults,
          listenAddr: listenPortFromAddr(defaults.listenAddr),
          password: "",
        })),
      )
      .catch((error) => setMessage(String(error)));
  }, [setMessage]);

  async function submit() {
    setBusy(true);
    setMessage(null);
    try {
      const portError = validateListenPort(form.listenAddr ?? "");
      if (portError) throw new Error(portError);
      const next = await completeSetup({
        ...form,
        listenAddr: listenAddrFromPort(form.listenAddr ?? ""),
      });
      setState(next);
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  }

  async function pickBaseDir() {
    const selected = await chooseDirectory("选择 MFTX 工作目录");
    if (selected) setForm((current) => ({...current, baseDir: selected}));
  }

  return (
    <div className="flex h-full flex-col bg-background">
      {!isMac && (
        <div className="drag-region flex h-9 shrink-0 items-center justify-end pr-1">
          <WindowControls />
        </div>
      )}
      <main className="flex min-h-0 flex-1 items-center justify-center overflow-y-auto px-6 pb-10">
      <div className="grid w-full max-w-5xl grid-cols-1 gap-6 lg:grid-cols-[360px_minmax(0,1fr)]">
        <section className="surface-card flex flex-col justify-between overflow-hidden p-6">
          <div>
            <div className="flex size-12 items-center justify-center rounded-2xl bg-accent text-accent-foreground shadow-surface">
              <RadioTower className="size-5" />
            </div>
            <h1 className="mt-5 text-2xl font-semibold leading-8 text-foreground">初始化 MFTX Desktop</h1>
            <p className="mt-2 text-sm leading-6 text-muted">
              设置本机名称、监听端口和工作目录后，就可以在局域网里发现设备并传文件。
            </p>
          </div>

          <div className="mt-8 grid gap-3">
            <InfoItem icon={<Wifi className="size-4" />} title="局域网发现" text="自动扫描同网段 peer" />
            <InfoItem icon={<ShieldCheck className="size-4" />} title="确认接收" text="陌生设备发送前先询问" />
            <InfoItem icon={<FolderOpen className="size-4" />} title="本机保存" text="配置和收件目录留在本机" />
          </div>
        </section>

        <section className="surface-card p-6">
          <Form className="grid gap-5" onSubmit={(event) => event.preventDefault()}>
            <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
              <TextField
                required
                label="设备名称"
                value={form.deviceName}
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
            <div className="grid grid-cols-[minmax(0,1fr)_auto] gap-2">
              <TextField
                className="min-w-0"
                label="工作目录"
                value={form.baseDir ?? ""}
                onValueChange={(baseDir) => setForm((current) => ({...current, baseDir}))}
              />
              <Button isIconOnly aria-label="选择工作目录" className="self-end" variant="outline" onPress={pickBaseDir}>
                <FolderOpen className="size-4" />
              </Button>
            </div>

            <ErrorAlert message={message} />

            <Button className="w-full" isDisabled={busy || Boolean(portError)} variant="primary" onPress={submit}>
              {busy ? "初始化中..." : "完成初始化"}
            </Button>
          </Form>
        </section>
      </div>
      </main>
    </div>
  );
}

type InfoItemProps = {
  icon: React.ReactNode;
  text: string;
  title: string;
};

function InfoItem({icon, text, title}: InfoItemProps) {
  return (
    <div className="subtle-card flex items-center gap-3 p-3">
      <div className="flex size-9 shrink-0 items-center justify-center rounded-2xl bg-surface text-muted">{icon}</div>
      <div className="min-w-0">
        <div className="text-sm font-medium text-foreground">{title}</div>
        <div className="truncate text-xs text-muted">{text}</div>
      </div>
    </div>
  );
}
