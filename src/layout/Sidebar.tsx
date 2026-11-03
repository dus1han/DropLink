import { motion } from "framer-motion";
import {
  Activity,
  Boxes,
  Clipboard,
  Download,
  LayoutDashboard,
  PanelLeftClose,
  PanelLeftOpen,
  Radar,
  Send,
  Settings,
  ShieldCheck,
  Type,
  type LucideIcon,
} from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { LogoMark } from "@/components/Logo";
import { IconButton } from "@/components/ui/Button";
import { Tooltip } from "@/components/ui/Tooltip";
import { cn } from "@/lib/cn";
import { isActive } from "@/lib/format";
import { useApp, type PageId } from "@/store/app";

interface NavItem {
  id: PageId;
  label: string;
  icon: LucideIcon;
}

const NAV: NavItem[][] = [
  [
    { id: "overview", label: "Overview", icon: LayoutDashboard },
    { id: "devices", label: "Nearby Devices", icon: Radar },
  ],
  [
    { id: "send", label: "Send Files", icon: Send },
    { id: "text", label: "Send Text", icon: Type },
    { id: "receive", label: "Receive", icon: Download },
    { id: "clipboard", label: "Clipboard", icon: Clipboard },
    { id: "activity", label: "Transfer Activity", icon: Activity },
  ],
  [
    { id: "groups", label: "Device Groups", icon: Boxes },
    { id: "trusted", label: "Trusted Devices", icon: ShieldCheck },
    { id: "settings", label: "Settings", icon: Settings },
  ],
];

export function Sidebar() {
  const page = useApp((s) => s.page);
  const navigate = useApp((s) => s.navigate);
  const collapsed = useApp((s) => s.sidebarCollapsed);
  const setCollapsed = useApp((s) => s.setSidebarCollapsed);
  const local = useApp((s) => s.local);
  const platform = useApp((s) => s.platform);
  const onlineCount = useApp((s) => Object.values(s.devices).filter((d) => d.online && d.trust !== "blocked").length);
  const activeCount = useApp((s) => Object.values(s.deliveries).filter((d) => isActive(d.status)).length);
  const pending = useApp((s) => s.incoming.length);

  const badges: Partial<Record<PageId, { n: number; tone: "accent" | "warning" | "muted" }>> = {
    devices: { n: onlineCount, tone: "muted" },
    activity: { n: activeCount, tone: "accent" },
    receive: { n: pending, tone: "warning" },
  };

  return (
    <motion.aside
      initial={false}
      animate={{ width: collapsed ? 60 : 232 }}
      transition={{ type: "spring", stiffness: 380, damping: 38 }}
      className="relative z-10 flex h-full shrink-0 flex-col border-r border-line bg-sidebar"
    >
      {platform === "macos" && <div data-tauri-drag-region className="h-7 shrink-0" />}
      <div data-tauri-drag-region className={cn("flex h-12 shrink-0 items-center gap-2.5", collapsed ? "justify-center px-0" : "px-4")}>
        <LogoMark size={22} />
        {!collapsed && (
          <motion.span initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="text-[14px] font-semibold tracking-tight text-fg">
            DropLink
          </motion.span>
        )}
      </div>

      <nav className="flex-1 overflow-x-hidden overflow-y-auto px-2 py-2">
        {NAV.map((group, gi) => (
          <div key={gi} className={cn(gi > 0 && "mt-4")}>
            {group.map((item) => {
              const active = page === item.id;
              const badge = badges[item.id];
              const btn = (
                <button
                  key={item.id}
                  onClick={() => navigate(item.id)}
                  className={cn(
                    "relative mb-0.5 flex h-8 w-full items-center gap-2.5 rounded-md text-[13px] transition-colors duration-150",
                    collapsed ? "justify-center" : "px-2.5",
                    active ? "text-fg" : "text-fg-muted hover:bg-surface-2/70 hover:text-fg",
                  )}
                >
                  {active && (
                    <motion.span
                      layoutId="nav-active"
                      className="absolute inset-0 rounded-md bg-surface-3 shadow-soft"
                      transition={{ type: "spring", stiffness: 500, damping: 40 }}
                    />
                  )}
                  <item.icon size={16} strokeWidth={active ? 2.1 : 1.8} className={cn("relative shrink-0", active && "text-accent")} />
                  {!collapsed && <span className="relative truncate font-medium">{item.label}</span>}
                  {badge && badge.n > 0 && (
                    <span
                      className={cn(
                        "tabular relative ml-auto flex h-[18px] min-w-[18px] items-center justify-center rounded-full px-1 text-[10.5px] font-semibold",
                        collapsed && "absolute top-0.5 right-1 ml-0 h-3.5 min-w-3.5 text-[9px]",
                        badge.tone === "warning"
                          ? "bg-warning text-black"
                          : badge.tone === "accent"
                            ? "bg-accent text-white"
                            : "bg-surface-3 text-fg-muted",
                      )}
                    >
                      {badge.n}
                    </span>
                  )}
                </button>
              );
              return collapsed ? (
                <Tooltip key={item.id} label={item.label} side="right">
                  {btn}
                </Tooltip>
              ) : (
                btn
              );
            })}
          </div>
        ))}
      </nav>

      <div className="border-t border-line p-2">
        {local && (
          <button
            onClick={() => navigate("settings")}
            className={cn(
              "flex w-full items-center gap-2.5 rounded-lg p-2 text-left transition-colors hover:bg-surface-2",
              collapsed && "justify-center p-1.5",
            )}
          >
            <DeviceAvatar os={local.os} size={30} online={local.discoveryActive || local.addresses.length > 0} />
            {!collapsed && (
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12.5px] font-semibold text-fg">{local.name}</div>
                <div className="truncate text-2xs text-fg-subtle">
                  {local.addresses.length === 0
                    ? "No network"
                    : local.discoveryActive
                      ? "Visible to nearby devices"
                      : "Hidden from discovery"}
                </div>
              </div>
            )}
          </button>
        )}
        <div className={cn("mt-1 flex items-center", collapsed ? "justify-center" : "justify-between px-2")}>
          {!collapsed && <span className="text-2xs text-fg-subtle tabular">v{local?.appVersion}</span>}
          <IconButton
            icon={collapsed ? PanelLeftOpen : PanelLeftClose}
            label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
            size="sm"
            onClick={() => setCollapsed(!collapsed)}
          />
        </div>
      </div>
    </motion.aside>
  );
}
