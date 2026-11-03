import { useMemo } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Activity,
  ArrowRight,
  Database,
  FileCheck2,
  FileUp,
  MonitorSmartphone,
  Radar,
  Send,
  ShieldCheck,
  Type,
  Users,
  type LucideIcon,
} from "lucide-react";
import { DeviceCard } from "@/components/DeviceCard";
import { TransferItem } from "@/components/TransferItem";
import { Button } from "@/components/ui/Button";
import { EmptyState, SectionTitle } from "@/components/ui/misc";
import { useStats } from "@/hooks/useStats";
import { cn } from "@/lib/cn";
import { formatBytes, isActive } from "@/lib/format";
import { sortDevices, useApp, type PageId } from "@/store/app";

export function OverviewPage() {
  const devicesMap = useApp((s) => s.devices);
  const deliveriesMap = useApp((s) => s.deliveries);
  const navigate = useApp((s) => s.navigate);
  const local = useApp((s) => s.local);
  const stats = useStats();

  const devices = useMemo(
    () => sortDevices(Object.values(devicesMap).filter((d) => d.trust !== "blocked" && (d.online || d.trust === "trusted"))),
    [devicesMap],
  );
  const deliveries = useMemo(
    () =>
      Object.values(deliveriesMap)
        .sort((a, b) => Number(isActive(b.status)) - Number(isActive(a.status)) || (b.finishedAt ?? b.createdAt) - (a.finishedAt ?? a.createdAt))
        .slice(0, 6),
    [deliveriesMap],
  );
  const onlineTrusted = devices.filter((d) => d.online && d.trust === "trusted").map((d) => d.id);
  const active = Object.values(deliveriesMap).filter((d) => isActive(d.status)).length;

  return (
    <div className="space-y-7">
      <div className="grid grid-cols-2 gap-3 lg:grid-cols-5">
        <Stat icon={MonitorSmartphone} label="Connected devices" value={stats?.knownDevices ?? "—"} hint={`${stats?.trustedDevices ?? 0} trusted`} />
        <Stat icon={Radar} label="Online now" value={stats?.onlineDevices ?? "—"} hint={local?.discoveryActive ? "Discovery on" : "Discovery off"} accent="success" />
        <Stat icon={Activity} label="Active transfers" value={active} hint={active ? "In progress" : "Idle"} accent={active ? "accent" : undefined} />
        <Stat icon={FileCheck2} label="Files today" value={stats?.filesToday ?? "—"} hint={stats ? formatBytes(stats.bytesToday) : ""} />
        <Stat icon={Database} label="Total transferred" value={stats ? formatBytes(stats.bytesTotal) : "—"} hint={`${stats?.transfersTotal ?? 0} transfers`} />
      </div>

      <div className="grid grid-cols-2 gap-3 lg:grid-cols-4">
        <QuickAction icon={FileUp} title="Send files" body="Files or whole folders" onClick={() => navigate("send")} primary />
        <QuickAction icon={Type} title="Send text" body="Notes, links, snippets" onClick={() => navigate("text")} />
        <QuickAction
          icon={Users}
          title="Send to multiple"
          body={onlineTrusted.length ? `${onlineTrusted.length} trusted device${onlineTrusted.length === 1 ? "" : "s"} online` : "Pick several recipients"}
          onClick={() => navigate("send", { recipients: onlineTrusted })}
        />
        <QuickAction icon={ShieldCheck} title="Manage devices" body="Pairing and permissions" onClick={() => navigate("trusted")} />
      </div>

      <div className="grid gap-7 xl:grid-cols-[minmax(0,1fr)_400px]">
        <section>
          <SectionTitle
            count={devices.length}
            actions={
              <Button size="sm" variant="ghost" iconRight={ArrowRight} onClick={() => navigate("devices")}>
                All devices
              </Button>
            }
          >
            Devices
          </SectionTitle>
          {devices.length === 0 ? (
            <div className="rounded-xl border border-dashed border-line-strong">
              <EmptyState
                icon={Radar}
                title="Looking for devices…"
                description="Open DropLink on another computer on this network and it will appear here. You can also connect by address."
                action={
                  <Button size="sm" variant="subtle" onClick={() => navigate("devices")}>
                    Nearby devices
                  </Button>
                }
              />
            </div>
          ) : (
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <AnimatePresence>
                {devices.slice(0, 6).map((d) => (
                  <DeviceCard key={d.id} device={d} />
                ))}
              </AnimatePresence>
            </div>
          )}
        </section>

        <section>
          <SectionTitle
            actions={
              <Button size="sm" variant="ghost" iconRight={ArrowRight} onClick={() => navigate("activity")}>
                History
              </Button>
            }
          >
            Recent activity
          </SectionTitle>
          <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            {deliveries.length === 0 ? (
              <EmptyState compact icon={Send} title="No transfers yet" description="Files you send and receive will appear here." />
            ) : (
              <div className="divide-y divide-line">
                <AnimatePresence initial={false}>
                  {deliveries.map((d) => (
                    <TransferItem key={d.id} delivery={d} compact />
                  ))}
                </AnimatePresence>
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function Stat({
  icon: Icon,
  label,
  value,
  hint,
  accent,
}: {
  icon: LucideIcon;
  label: string;
  value: number | string;
  hint?: string;
  accent?: "success" | "accent";
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 4 }}
      animate={{ opacity: 1, y: 0 }}
      className="rounded-xl border border-line bg-surface px-4 py-3.5 surface-highlight"
    >
      <div className="flex items-center gap-1.5 text-xs text-fg-muted">
        <Icon size={13} className={cn(accent === "success" ? "text-success" : accent === "accent" ? "text-accent" : "text-fg-subtle")} />
        {label}
      </div>
      <div className="tabular mt-2 text-[22px] leading-none font-semibold tracking-tight text-fg">{value}</div>
      {hint && <div className="mt-1.5 truncate text-2xs text-fg-subtle">{hint}</div>}
    </motion.div>
  );
}

function QuickAction({
  icon: Icon,
  title,
  body,
  onClick,
  primary,
}: {
  icon: LucideIcon;
  title: string;
  body: string;
  onClick: () => void;
  primary?: boolean;
  page?: PageId;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "group relative flex items-center gap-3 overflow-hidden rounded-xl border p-3.5 text-left transition-all duration-200",
        primary
          ? "border-transparent text-white brand-gradient shadow-[0_8px_24px_-10px_rgba(110,107,255,0.6)] hover:brightness-110"
          : "border-line bg-surface hover:border-line-strong hover:bg-surface-2 surface-highlight",
      )}
    >
      <div
        className={cn(
          "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
          primary ? "bg-white/18" : "bg-surface-2 text-fg-muted group-hover:text-accent",
        )}
      >
        <Icon size={17} />
      </div>
      <div className="min-w-0">
        <div className={cn("text-[13px] font-semibold", primary ? "text-white" : "text-fg")}>{title}</div>
        <div className={cn("truncate text-xs", primary ? "text-white/75" : "text-fg-muted")}>{body}</div>
      </div>
      <ArrowRight
        size={14}
        className={cn("ml-auto shrink-0 transition-transform duration-200 group-hover:translate-x-0.5", primary ? "text-white/80" : "text-fg-subtle")}
      />
    </button>
  );
}
