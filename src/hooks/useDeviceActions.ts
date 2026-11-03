import { useState } from "react";
import {
  Ban,
  ClipboardCopy,
  FileUp,
  Link2,
  Pencil,
  RefreshCw,
  ShieldCheck,
  ShieldOff,
  Trash2,
  Type,
} from "lucide-react";
import type { MenuItem } from "@/components/ui/Menu";
import { api } from "@/lib/api";
import type { Device } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { useApp } from "@/store/app";

/** Every action available on a device, shared by cards, rows and pages. */
export function useDeviceActions(device: Device) {
  const { navigate, startPairing, fail, toast } = useApp.getState();
  const [renaming, setRenaming] = useState(false);
  const [busy, setBusy] = useState(false);

  const run = async (fn: () => Promise<unknown>, success?: string) => {
    setBusy(true);
    try {
      await fn();
      if (success) toast({ tone: "success", title: success });
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const trusted = device.trust === "trusted";
  const blocked = device.trust === "blocked";

  const actions = {
    pair: () => startPairing(device),
    send: () => navigate("send", { recipients: [device.id] }),
    sendText: () => navigate("text", { recipients: [device.id] }),
    sendClipboard: () =>
      run(async () => {
        const [r] = await api.sendClipboard([device.id]);
        if (!r.ok) throw r.error ?? "Could not send clipboard";
      }, `Clipboard sent to ${device.name}`),
    rename: () => setRenaming(true),
    refresh: () =>
      run(async () => {
        const ok = await api.probeDevice(device.id);
        toast({ tone: ok ? "success" : "info", title: ok ? `${device.name} is reachable` : `${device.name} is not reachable` });
      }),
    revoke: async () => {
      const ok = await confirm({
        title: `Remove trust for ${device.name}?`,
        body: "It will no longer be able to send you files, text or clipboard content until you pair again.",
        confirmLabel: "Remove trust",
        tone: "danger",
      });
      if (ok) await run(() => api.revokeDevice(device.id), `${device.name} is no longer trusted`);
    },
    block: async () => {
      const ok = await confirm({
        title: `Block ${device.name}?`,
        body: "All connections from this device will be refused, including pairing requests.",
        confirmLabel: "Block device",
        tone: "danger",
      });
      if (ok) await run(() => api.blockDevice(device.id), `${device.name} blocked`);
    },
    unblock: () => run(() => api.unblockDevice(device.id), `${device.name} unblocked`),
    forget: async () => {
      const ok = await confirm({
        title: `Forget ${device.name}?`,
        body: "Its trust, permissions and group memberships are removed. Transfer history is kept.",
        confirmLabel: "Forget",
        tone: "danger",
      });
      if (ok) await run(() => api.forgetDevice(device.id));
    },
  };

  const menu: (MenuItem | "separator")[] = [
    { label: "Send files", icon: FileUp, onSelect: actions.send, hidden: !trusted },
    { label: "Send text", icon: Type, onSelect: actions.sendText, hidden: !trusted },
    { label: "Send clipboard", icon: ClipboardCopy, onSelect: actions.sendClipboard, hidden: !trusted || !device.online },
    { label: "Pair", icon: Link2, onSelect: actions.pair, hidden: trusted || blocked || !device.online },
    "separator",
    { label: "Rename", icon: Pencil, onSelect: actions.rename },
    { label: "Check connection", icon: RefreshCw, onSelect: actions.refresh },
    "separator",
    { label: "Remove trust", icon: ShieldOff, onSelect: actions.revoke, hidden: !trusted, tone: "danger" },
    { label: "Unblock", icon: ShieldCheck, onSelect: actions.unblock, hidden: !blocked },
    { label: "Block", icon: Ban, onSelect: actions.block, hidden: blocked, tone: "danger" },
    { label: "Forget device", icon: Trash2, onSelect: actions.forget, tone: "danger" },
  ];

  return { actions, menu, renaming, setRenaming, busy };
}
