import { motion } from "framer-motion";
import { Link2, MoreHorizontal, Send } from "lucide-react";
import { ConnectionLabel, TrustBadge } from "./DeviceCard";
import { DeviceAvatar } from "./DeviceAvatar";
import { RenameDialog } from "./dialogs/RenameDialog";
import { Button, IconButton } from "./ui/Button";
import { Menu } from "./ui/Menu";
import { useDeviceActions } from "@/hooks/useDeviceActions";
import { formatRelative, osLabel } from "@/lib/format";
import type { Device } from "@/lib/types";

export function DeviceRow({ device, extra }: { device: Device; extra?: React.ReactNode }) {
  const { actions, menu, renaming, setRenaming } = useDeviceActions(device);
  return (
    <motion.div
      layout="position"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="group flex items-center gap-3.5 px-4 py-3"
    >
      <DeviceAvatar os={device.os} online={device.online} size={36} />
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="truncate text-[13px] font-semibold text-fg">{device.name}</span>
          <TrustBadge device={device} />
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 truncate text-xs text-fg-muted">
          <span>{osLabel(device.os)}</span>
          <span className="text-fg-subtle">·</span>
          <ConnectionLabel device={device} />
          {device.address && (
            <>
              <span className="text-fg-subtle">·</span>
              <span className="selectable font-mono text-[11.5px]">{device.address}</span>
            </>
          )}
          {!device.online && (
            <>
              <span className="text-fg-subtle">·</span>
              <span>Seen {formatRelative(device.lastSeen)}</span>
            </>
          )}
        </div>
      </div>
      {extra}
      {device.trust === "trusted" && (
        <Button size="sm" icon={Send} onClick={actions.send}>
          Send
        </Button>
      )}
      {device.trust === "unknown" && (
        <Button size="sm" variant="subtle" icon={Link2} onClick={actions.pair} disabled={!device.online}>
          Pair
        </Button>
      )}
      <Menu items={menu} trigger={(p) => <IconButton icon={MoreHorizontal} label="More actions" size="sm" {...p} />} />
      <RenameDialog device={device} open={renaming} onClose={() => setRenaming(false)} />
    </motion.div>
  );
}
