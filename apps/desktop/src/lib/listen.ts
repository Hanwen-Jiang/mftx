export const DEFAULT_LISTEN_PORT = "48151";

export function listenPortFromAddr(addr?: string | null): string {
  const value = addr?.trim();
  if (!value) return DEFAULT_LISTEN_PORT;
  const index = value.lastIndexOf(":");
  return index >= 0 ? value.slice(index + 1) : value;
}

export function listenAddrFromPort(port: string): string {
  return `0.0.0.0:${port.trim() || DEFAULT_LISTEN_PORT}`;
}

export function validateListenPort(port: string): string | null {
  const value = port.trim();
  if (!/^\d+$/.test(value)) return "监听端口必须是数字";
  const parsed = Number(value);
  if (!Number.isInteger(parsed) || parsed < 1 || parsed > 65535) {
    return "监听端口必须在 1 到 65535 之间";
  }
  return null;
}
