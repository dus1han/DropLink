import { motion } from "framer-motion";
import { ArrowDownLeft, ArrowUpRight, FolderOpen, Pause, Play, RotateCw, ShieldCheck, X } from "lucide-react";
import { FileIcon } from "./FileIcon";
import { Badge, type Tone } from "./ui/Badge";
import { IconButton } from "./ui/Button";
import { ProgressBar } from "./ui/misc";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { STATUS_LABEL, formatBytes, formatDuration, formatRelative, formatSpeed, isActive, isTerminal, pluralize } from "@/lib/format";
import type { Delivery, DeliveryStatus } from "@/lib/types";
import { useApp } from "@/store/app";

const TONE: Record<DeliveryStatus, Tone> = {
  queued: "neutral",
  connecting: "info",
  waiting: "warning",
  transferring: "accent",
  paused: "warning",
  completed: "success",
  failed: "danger",
  cancelled: "neutral",
  rejected: "danger",
};

export function TransferItem({ delivery, compact }: { delivery: Delivery; compact?: boolean }) {
  const progress = useApp((s) => s.progress[delivery.id]);
  const fail = useApp((s) => s.fail);
  const d = delivery;
  const outgoing = d.direction === "outgoing";
  const bytesDone = progress?.bytesDone ?? d.bytesDone;
  const total = progress?.totalBytes ?? d.totalBytes;
  const frac = d.status === "completed" ? 1 : total > 0 ? bytesDone / total : 0;
  const live = d.status === "transferring";
  const speed = live ? progress?.speedBps ?? 0 : 0;

  const act = (fn: () => Promise<unknown>) => () => fn().catch((e) => fail(e));

  const isFolder = !!d.firstFile && d.firstFile.includes("/") && d.firstFile.split("/")[0] === d.label;
  const tone: "accent" | "success" | "warning" | "danger" | "neutral" =
    d.status === "completed" ? "success" : d.status === "failed" || d.status === "rejected" ? "danger" : d.status === "paused" ? "warning" : d.status === "cancelled" ? "neutral" : "accent";

  return (
    <motion.div
      layout="position"
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0 }}
      className={cn("group flex gap-3 px-4", compact ? "py-3" : "py-3.5")}
    >
      <div className="relative">
        <FileIcon name={d.firstFile ?? d.label} isDir={isFolder} size={compact ? 32 : 36} />
        <span
          className={cn(
            "absolute -right-1 -bottom-1 flex h-4 w-4 items-center justify-center rounded-full border-2 border-surface",
            outgoing ? "bg-accent text-white" : "bg-info text-white",
          )}
        >
          {outgoing ? <ArrowUpRight size={9} strokeWidth={3} /> : <ArrowDownLeft size={9} strokeWidth={3} />}
        </span>
      </div>

      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1 truncate text-[13px] font-medium text-fg">{d.label}</div>
          {d.verified && (
            <span title="Every file matched its SHA-256 checksum" className="text-success">
              <ShieldCheck size={13} />
            </span>
          )}
          <Badge tone={TONE[d.status]}>{STATUS_LABEL[d.status]}</Badge>
        </div>
        <div className="mt-0.5 truncate text-xs text-fg-muted">
          {outgoing ? "To" : "From"} <span className="text-fg">{d.deviceName}</span>
          <span className="text-fg-subtle"> · {pluralize(d.totalFiles, "file")} · {formatBytes(d.totalBytes)}</span>
          {isTerminal(d.status) && d.finishedAt && <span className="text-fg-subtle"> · {formatRelative(d.finishedAt)}</span>}
        </div>

        {!isTerminal(d.status) && (
          <div className="mt-2.5">
            <ProgressBar value={frac} tone={tone} indeterminate={d.status === "connecting" || d.status === "queued"} />
            <div className="tabular mt-1.5 flex items-center gap-3 text-2xs text-fg-subtle">
              <span>
                {formatBytes(bytesDone)} of {formatBytes(total)}
              </span>
              <span>{Math.floor(frac * 100)}%</span>
              {live && <span>{formatSpeed(speed)}</span>}
              {live && progress?.etaSeconds != null && <span>{formatDuration(progress.etaSeconds)} left</span>}
              {!live && d.status !== "paused" && <span className="text-fg-muted">{statusHint(d)}</span>}
              {live && progress?.currentFile && d.totalFiles > 1 && (
                <span className="min-w-0 flex-1 truncate text-right">{progress.currentFile}</span>
              )}
            </div>
          </div>
        )}
        {d.error && (d.status === "failed" || d.status === "paused" || d.status === "rejected" || d.status === "connecting" || d.status === "cancelled") && (
          <div className={cn("mt-1.5 text-2xs", d.status === "failed" ? "text-danger" : "text-fg-muted")}>{d.error}</div>
        )}
      </div>

      <div className="flex shrink-0 items-start gap-0.5 pt-0.5">
        {isActive(d.status) && (outgoing || d.status === "transferring") && d.status !== "queued" && (
          <IconButton icon={Pause} label="Pause" size="sm" onClick={act(() => api.pauseDelivery(d.id))} />
        )}
        {d.status === "paused" && <IconButton icon={Play} label="Resume" size="sm" tone="accent" onClick={act(() => api.resumeDelivery(d.id))} />}
        {outgoing && (d.status === "failed" || d.status === "cancelled" || d.status === "rejected") && (
          <IconButton icon={RotateCw} label="Retry" size="sm" onClick={act(() => api.resumeDelivery(d.id))} />
        )}
        {!outgoing && d.status === "completed" && d.destDir && (
          <IconButton icon={FolderOpen} label="Show in folder" size="sm" onClick={act(() => api.openPath(d.destDir!))} />
        )}
        {!isTerminal(d.status) && <IconButton icon={X} label="Cancel" size="sm" tone="danger" onClick={act(() => api.cancelDelivery(d.id))} />}
      </div>
    </motion.div>
  );
}

function statusHint(d: Delivery): string {
  switch (d.status) {
    case "queued":
      return "Waiting for a free slot";
    case "connecting":
      return d.error ? "" : "Connecting securely…";
    case "waiting":
      return d.direction === "outgoing" ? `Waiting for ${d.deviceName} to accept` : "Waiting for your approval";
    default:
      return "";
  }
}
