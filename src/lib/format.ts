import type { DeliveryStatus, Os } from "./types";

const UNITS = ["B", "KB", "MB", "GB", "TB"];

/** Decimal units, matching what Finder and Explorer's "size" column show. */
export function formatBytes(n: number, digits = 1): string {
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1000) return `${Math.round(n)} B`;
  let v = n;
  let i = 0;
  while (v >= 1000 && i < UNITS.length - 1) {
    v /= 1000;
    i++;
  }
  const d = v >= 100 ? 0 : digits;
  return `${v.toFixed(d)} ${UNITS[i]}`;
}

export function formatSpeed(bps: number): string {
  if (!bps || bps < 1) return "—";
  return `${formatBytes(bps)}/s`;
}

export function formatDuration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds)) return "—";
  const s = Math.max(0, Math.round(seconds));
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m}m ${String(s % 60).padStart(2, "0")}s`;
  const h = Math.floor(m / 60);
  return `${h}h ${String(m % 60).padStart(2, "0")}m`;
}

export function formatRelative(ms: number, now = Date.now()): string {
  const diff = Math.round((now - ms) / 1000);
  if (diff < 10) return "just now";
  if (diff < 60) return `${diff}s ago`;
  const m = Math.floor(diff / 60);
  if (m < 60) return `${m}m ago`;
  const h = Math.floor(m / 60);
  if (h < 24) return `${h}h ago`;
  const d = Math.floor(h / 24);
  if (d < 7) return `${d}d ago`;
  return new Date(ms).toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

export function formatDateTime(ms: number): string {
  return new Date(ms).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

export function startOfToday(): number {
  const d = new Date();
  d.setHours(0, 0, 0, 0);
  return d.getTime();
}

export function osLabel(os: Os): string {
  switch (os) {
    case "macos":
      return "macOS";
    case "windows":
      return "Windows";
    case "linux":
      return "Linux";
    default:
      return "Unknown OS";
  }
}

export const STATUS_LABEL: Record<DeliveryStatus, string> = {
  queued: "Queued",
  connecting: "Connecting",
  waiting: "Waiting for approval",
  transferring: "Transferring",
  paused: "Paused",
  completed: "Completed",
  failed: "Failed",
  cancelled: "Cancelled",
  rejected: "Declined",
};

export function isActive(s: DeliveryStatus): boolean {
  return s === "queued" || s === "connecting" || s === "waiting" || s === "transferring";
}

export function isTerminal(s: DeliveryStatus): boolean {
  return s === "completed" || s === "failed" || s === "cancelled" || s === "rejected";
}

export function fileExtension(name: string | null | undefined): string {
  if (!name) return "";
  const base = name.split("/").pop() ?? name;
  const i = base.lastIndexOf(".");
  return i > 0 ? base.slice(i + 1).toLowerCase() : "";
}

export function pluralize(n: number, one: string, many = `${one}s`): string {
  return `${n.toLocaleString()} ${n === 1 ? one : many}`;
}

/** Last path component for either separator. */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? path;
}

export function joinPath(dir: string, ...rest: string[]): string {
  const sep = dir.includes("\\") ? "\\" : "/";
  return [dir.replace(/[\\/]+$/, ""), ...rest.map((r) => r.replace(/\//g, sep))].join(sep);
}
