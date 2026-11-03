import { useEffect, useRef, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Bell,
  CheckCheck,
  CircleAlert,
  CircleCheck,
  Download,
  Link2,
  Monitor,
  Moon,
  Radar,
  ShieldAlert,
  Sun,
  Trash2,
  Type,
  Wifi,
  WifiOff,
} from "lucide-react";
import { IconButton } from "@/components/ui/Button";
import { EmptyState } from "@/components/ui/misc";
import { Tooltip } from "@/components/ui/Tooltip";
import { cn } from "@/lib/cn";
import { formatRelative } from "@/lib/format";
import type { Theme } from "@/lib/types";
import { useApp, type AppNotification } from "@/store/app";

export const PAGE_TITLES: Record<string, { title: string; subtitle: string }> = {
  overview: { title: "Overview", subtitle: "Your devices and transfers at a glance" },
  devices: { title: "Nearby Devices", subtitle: "Devices running DropLink on your networks" },
  send: { title: "Send Files", subtitle: "Files and folders, streamed directly and verified" },
  text: { title: "Send Text", subtitle: "Notes, links and snippets, delivered instantly" },
  receive: { title: "Receive", subtitle: "How this computer accepts incoming transfers" },
  clipboard: { title: "Universal Clipboard", subtitle: "Copy on one computer, paste on another" },
  activity: { title: "Transfer Activity", subtitle: "Live transfers and searchable history" },
  groups: { title: "Device Groups", subtitle: "Send to sets of devices and manage their permissions" },
  trusted: { title: "Trusted Devices", subtitle: "Paired devices, permissions and security events" },
  settings: { title: "Settings", subtitle: "Preferences for this computer" },
};

const THEME_NEXT: Record<Theme, Theme> = { dark: "light", light: "system", system: "dark" };
const THEME_ICON = { dark: Moon, light: Sun, system: Monitor };

export function TopBar() {
  const page = useApp((s) => s.page);
  const local = useApp((s) => s.local);
  const settings = useApp((s) => s.settings);
  const saveSettings = useApp((s) => s.saveSettings);
  const platform = useApp((s) => s.platform);
  const navigate = useApp((s) => s.navigate);
  const t = PAGE_TITLES[page];
  const online = (local?.addresses.length ?? 0) > 0;
  const theme = settings?.theme ?? "system";
  const ThemeIcon = THEME_ICON[theme];

  return (
    <header
      data-tauri-drag-region
      className={cn("flex shrink-0 items-center gap-4 border-b border-line px-6", platform === "macos" ? "h-[60px] pt-2" : "h-14")}
    >
      <div data-tauri-drag-region className="min-w-0 flex-1">
        <motion.h1
          key={page}
          initial={{ opacity: 0, y: 3 }}
          animate={{ opacity: 1, y: 0 }}
          className="truncate text-[15px] font-semibold tracking-tight text-fg"
        >
          {t.title}
        </motion.h1>
        <p className="truncate text-xs text-fg-subtle">{t.subtitle}</p>
      </div>

      <div className="flex items-center gap-1.5">
        <Tooltip label={online ? `Listening on ${local?.addresses.join(", ")} · port ${local?.port}` : "No network connection"}>
          <div
            className={cn(
              "hidden h-7 items-center gap-1.5 rounded-full border border-line px-2.5 text-2xs font-medium md:flex",
              online ? "text-fg-muted" : "text-danger",
            )}
          >
            {online ? <Wifi size={12} className="text-success" /> : <WifiOff size={12} />}
            <span className="tabular font-mono">{online ? local?.addresses[0] : "Offline"}</span>
          </div>
        </Tooltip>
        <Tooltip label={local?.discoveryActive ? "Advertising and discovering on the local network" : "Automatic discovery is off"}>
          <button
            onClick={() => navigate(local?.discoveryActive ? "devices" : "settings")}
            className="hidden h-7 items-center gap-1.5 rounded-full border border-line px-2.5 text-2xs font-medium text-fg-muted hover:bg-surface-2 lg:flex"
          >
            <span className="relative flex">
              <Radar size={12} className={local?.discoveryActive ? "text-accent" : "text-fg-subtle"} />
              {local?.discoveryActive && <span className="absolute inset-0 animate-ping rounded-full bg-accent/30" />}
            </span>
            {local?.discoveryActive ? "Discovering" : "Discovery off"}
          </button>
        </Tooltip>
        <div className="mx-1 h-5 w-px bg-line" />
        <NotificationCenter />
        <IconButton
          icon={ThemeIcon}
          label={`Theme: ${theme} (click for ${THEME_NEXT[theme]})`}
          onClick={() => saveSettings({ theme: THEME_NEXT[theme] })}
        />
      </div>
    </header>
  );
}

const KIND_ICON: Record<AppNotification["kind"], { icon: typeof Bell; cls: string }> = {
  incoming: { icon: Download, cls: "text-info bg-info-soft" },
  completed: { icon: CircleCheck, cls: "text-success bg-success-soft" },
  failed: { icon: CircleAlert, cls: "text-danger bg-danger-soft" },
  device: { icon: Radar, cls: "text-accent bg-accent-soft" },
  pairing: { icon: Link2, cls: "text-accent bg-accent-soft" },
  text: { icon: Type, cls: "text-info bg-info-soft" },
  security: { icon: ShieldAlert, cls: "text-warning bg-warning-soft" },
};

function NotificationCenter() {
  const notifications = useApp((s) => s.notifications);
  const markAllRead = useApp((s) => s.markAllRead);
  const clear = useApp((s) => s.clearNotifications);
  const navigate = useApp((s) => s.navigate);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const unread = notifications.filter((n) => !n.read).length;

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !ref.current?.contains(e.target as Node) && setOpen(false);
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <div ref={ref} className="relative">
      <IconButton
        icon={Bell}
        label="Notifications"
        active={open}
        onClick={() => {
          setOpen(!open);
          if (!open) setTimeout(markAllRead, 1200);
        }}
        badge={
          unread > 0 && (
            <span className="tabular absolute top-1 right-1 flex h-3.5 min-w-3.5 items-center justify-center rounded-full bg-accent px-1 text-[9px] font-bold text-white">
              {unread > 9 ? "9+" : unread}
            </span>
          )
        }
      />
      <AnimatePresence>
        {open && (
          <motion.div
            initial={{ opacity: 0, y: -6, scale: 0.98 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: -4 }}
            transition={{ duration: 0.16, ease: [0.22, 1, 0.36, 1] }}
            className="absolute top-10 right-0 z-40 w-[360px] overflow-hidden rounded-xl bg-elevated shadow-pop"
          >
            <div className="flex items-center gap-2 border-b border-line px-4 py-3">
              <div className="flex-1 text-[13px] font-semibold">Notifications</div>
              <IconButton icon={CheckCheck} label="Mark all as read" size="sm" onClick={markAllRead} disabled={unread === 0} />
              <IconButton icon={Trash2} label="Clear all" size="sm" onClick={clear} disabled={!notifications.length} />
            </div>
            <div className="max-h-[420px] overflow-y-auto">
              {notifications.length === 0 ? (
                <EmptyState compact icon={Bell} title="You're all caught up" description="Transfers, requests and new devices show up here." />
              ) : (
                notifications.map((n) => {
                  const k = KIND_ICON[n.kind];
                  return (
                    <button
                      key={n.id}
                      onClick={() => {
                        if (n.page) navigate(n.page);
                        setOpen(false);
                      }}
                      className="flex w-full gap-3 px-4 py-3 text-left transition-colors hover:bg-surface-2 [&+&]:border-t [&+&]:border-line"
                    >
                      <div className={cn("flex h-7 w-7 shrink-0 items-center justify-center rounded-lg", k.cls)}>
                        <k.icon size={14} />
                      </div>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <span className="truncate text-[12.5px] font-medium text-fg">{n.title}</span>
                          {!n.read && <span className="h-1.5 w-1.5 shrink-0 rounded-full bg-accent" />}
                          <span className="ml-auto shrink-0 text-2xs text-fg-subtle">{formatRelative(n.at)}</span>
                        </div>
                        <div className="mt-0.5 line-clamp-2 text-xs text-fg-muted">{n.body}</div>
                      </div>
                    </button>
                  );
                })
              )}
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
