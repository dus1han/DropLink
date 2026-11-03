import { motion } from "framer-motion";
import { Clock, Globe, Link2, MoreHorizontal, Send, ShieldCheck, Wifi } from "lucide-react";
import { DeviceAvatar } from "./DeviceAvatar";
import { RenameDialog } from "./dialogs/RenameDialog";
import { Badge } from "./ui/Badge";
import { Button, IconButton } from "./ui/Button";
import { Menu } from "./ui/Menu";
import { useDeviceActions } from "@/hooks/useDeviceActions";
import { cn } from "@/lib/cn";
import { formatRelative, osLabel } from "@/lib/format";
import type { Device } from "@/lib/types";

export function TrustBadge({ device }: { device: Device }) {
  if (device.trust === "trusted") return <Badge tone="success" icon={ShieldCheck}>Trusted</Badge>;
  if (device.trust === "blocked") return <Badge tone="danger">Blocked</Badge>;
  return <Badge tone="neutral">Not paired</Badge>;
}

export function ConnectionLabel({ device }: { device: Device }) {
  const Icon = device.connectionType === "manual" ? Globe : Wifi;
  return (
    <span className="inline-flex items-center gap-1">
      <Icon size={11} />
      {device.connectionType === "manual" ? "Direct address" : "Local network"}
    </span>
  );
}

export function DeviceCard({ device }: { device: Device }) {
  const { actions, menu, renaming, setRenaming } = useDeviceActions(device);
  const trusted = device.trust === "trusted";

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.98 }}
      transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
      className={cn(
        "group relative flex flex-col rounded-xl border border-line bg-surface p-4 surface-highlight",
        "transition-[border-color,background-color] duration-200 hover:border-line-strong",
        !device.online && "opacity-75",
      )}
    >
      <div className="flex items-start gap-3">
        <DeviceAvatar os={device.os} online={device.online} size={40} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[13.5px] font-semibold text-fg">{device.name}</div>
          <div className="mt-0.5 flex items-center gap-1.5 text-xs text-fg-muted">
            <span>{osLabel(device.os)}</span>
            <span className="text-fg-subtle">·</span>
            <span className={device.online ? "text-success" : undefined}>{device.online ? "Online" : "Offline"}</span>
          </div>
        </div>
        <Menu
          items={menu}
          trigger={(p) => (
            <IconButton icon={MoreHorizontal} label="More" size="sm" className="-mt-1 -mr-1 opacity-60 group-hover:opacity-100" {...p} />
          )}
        />
      </div>

      <dl className="mt-4 grid grid-cols-2 gap-x-3 gap-y-2 text-xs">
        <div className="min-w-0">
          <dt className="text-2xs text-fg-subtle">Address</dt>
          <dd className="selectable mt-0.5 truncate font-mono text-[11.5px] text-fg-muted">
            {device.address ? `${device.address}${device.port ? `:${device.port}` : ""}` : "—"}
          </dd>
        </div>
        <div className="min-w-0">
          <dt className="text-2xs text-fg-subtle">Last seen</dt>
          <dd className="mt-0.5 flex items-center gap-1 text-fg-muted">
            <Clock size={11} />
            {device.online ? "Now" : formatRelative(device.lastSeen)}
          </dd>
        </div>
        <div className="min-w-0">
          <dt className="text-2xs text-fg-subtle">Connection</dt>
          <dd className="mt-0.5 text-fg-muted">
            <ConnectionLabel device={device} />
          </dd>
        </div>
        <div className="min-w-0">
          <dt className="text-2xs text-fg-subtle">Security</dt>
          <dd className="mt-0.5">
            <TrustBadge device={device} />
          </dd>
        </div>
      </dl>

      <div className="mt-4 flex gap-2">
        {trusted ? (
          <Button variant="secondary" size="sm" icon={Send} className="flex-1" onClick={actions.send}>
            Send
          </Button>
        ) : device.trust === "unknown" ? (
          <Button variant="subtle" size="sm" icon={Link2} className="flex-1" onClick={actions.pair} disabled={!device.online}>
            Pair device
          </Button>
        ) : (
          <Button variant="secondary" size="sm" className="flex-1" onClick={actions.unblock}>
            Unblock
          </Button>
        )}
      </div>
      <RenameDialog device={device} open={renaming} onClose={() => setRenaming(false)} />
    </motion.div>
  );
}
