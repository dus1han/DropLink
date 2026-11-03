import { useEffect, useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { AlertTriangle, Download, FolderOpen } from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { FileIcon } from "@/components/FileIcon";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Segmented } from "@/components/ui/misc";
import { api } from "@/lib/api";
import { formatBytes, osLabel, pluralize } from "@/lib/format";
import type { IncomingRequest } from "@/lib/types";
import { useApp } from "@/store/app";

/** Approval prompt for the oldest pending incoming transfer. */
export function IncomingDialog() {
  const req = useApp((s) => s.incoming[0]);
  return <IncomingView key={req?.id ?? "none"} req={req} />;
}

function IncomingView({ req }: { req: IncomingRequest | undefined }) {
  const settings = useApp((s) => s.settings);
  const fail = useApp((s) => s.fail);
  const [dest, setDest] = useState<string | null>(null);
  const [policy, setPolicy] = useState<"rename" | "overwrite">(settings?.conflictPolicy ?? "rename");
  const [busy, setBusy] = useState<"accept" | "decline" | null>(null);
  const [remaining, setRemaining] = useState(300);

  useEffect(() => {
    if (!req) return;
    const t = window.setInterval(() => setRemaining(Math.max(0, 300 - Math.floor((Date.now() - req.createdAt) / 1000))), 1000);
    return () => window.clearInterval(t);
  }, [req]);

  if (!req) return <Dialog open={false}>{null}</Dialog>;

  const respond = async (accept: boolean) => {
    setBusy(accept ? "accept" : "decline");
    try {
      await api.respondIncoming(req.id, accept, dest, policy === "overwrite");
      useApp.setState((s) => ({ incoming: s.incoming.filter((r) => r.id !== req.id) }));
    } catch (e) {
      fail(e);
      useApp.setState((s) => ({ incoming: s.incoming.filter((r) => r.id !== req.id) }));
    } finally {
      setBusy(null);
    }
  };

  const choose = async () => {
    const picked = await openDialog({ directory: true, defaultPath: dest ?? req.destDir, title: "Save incoming files to" });
    if (typeof picked === "string") setDest(picked);
  };

  const top = topLevel(req.items);

  return (
    <Dialog open dismissable={false} width={480}>
      <div className="flex items-center gap-3">
        <DeviceAvatar os={req.deviceOs} size={40} />
        <div className="min-w-0 flex-1">
          <div className="text-[15px] font-semibold text-fg">{req.deviceName} wants to send you</div>
          <div className="text-xs text-fg-muted">
            {osLabel(req.deviceOs)} · trusted device · expires in {Math.floor(remaining / 60)}:{String(remaining % 60).padStart(2, "0")}
          </div>
        </div>
      </div>

      <div className="mt-4 overflow-hidden rounded-lg border border-line bg-surface">
        <div className="flex items-center gap-3 border-b border-line px-3 py-2.5">
          <FileIcon name={req.label} isDir={top.length === 1 && top[0].isDir} size={34} />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-medium text-fg">{req.label}</div>
            <div className="text-xs text-fg-muted">
              {pluralize(req.totalFiles, "file")} · {formatBytes(req.totalBytes)}
            </div>
          </div>
        </div>
        <div className="max-h-40 overflow-y-auto py-1">
          {top.slice(0, 40).map((it) => (
            <div key={it.path} className="flex items-center gap-2 px-3 py-1 text-xs">
              <FileIcon name={it.path} isDir={it.isDir} size={20} className="rounded-md" />
              <span className="min-w-0 flex-1 truncate text-fg-muted">{it.path}</span>
              {!it.isDir && <span className="tabular text-fg-subtle">{formatBytes(it.size)}</span>}
            </div>
          ))}
          {top.length > 40 && <div className="px-3 py-1 text-2xs text-fg-subtle">and {top.length - 40} more…</div>}
        </div>
      </div>

      <div className="mt-4 space-y-3">
        <div className="flex items-center gap-2">
          <div className="min-w-0 flex-1">
            <div className="text-2xs text-fg-subtle">Save to</div>
            <div className="selectable truncate text-xs text-fg">{dest ?? req.destDir}</div>
          </div>
          <Button size="sm" variant="secondary" icon={FolderOpen} onClick={choose}>
            Change
          </Button>
        </div>
        {req.conflicts > 0 && !dest && (
          <div className="rounded-lg border border-warning/30 bg-warning-soft px-3 py-2.5">
            <div className="flex items-center gap-2 text-xs font-medium text-warning">
              <AlertTriangle size={13} />
              {pluralize(req.conflicts, "item")} with the same name already {req.conflicts === 1 ? "exists" : "exist"}
            </div>
            <div className="mt-2">
              <Segmented
                size="sm"
                value={policy}
                onChange={setPolicy}
                options={[
                  { value: "rename", label: "Keep both" },
                  { value: "overwrite", label: "Replace existing" },
                ]}
              />
            </div>
          </div>
        )}
      </div>

      <div className="mt-5 flex gap-2">
        <Button variant="secondary" className="flex-1" onClick={() => respond(false)} loading={busy === "decline"} disabled={!!busy}>
          Decline
        </Button>
        <Button variant="primary" className="flex-1" icon={Download} onClick={() => respond(true)} loading={busy === "accept"} disabled={!!busy}>
          Accept
        </Button>
      </div>
    </Dialog>
  );
}

function topLevel(items: IncomingRequest["items"]) {
  // Show folders as single entries rather than every nested file.
  const seen = new Map<string, { path: string; size: number; isDir: boolean }>();
  for (const it of items) {
    const [head, ...rest] = it.path.split("/");
    const isDir = it.isDir || rest.length > 0;
    const prev = seen.get(head);
    if (prev) prev.size += it.size;
    else seen.set(head, { path: head, size: it.size, isDir });
  }
  return [...seen.values()];
}
