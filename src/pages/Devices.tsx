import { useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Globe, History, Radar, RadioTower } from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { DeviceRow } from "@/components/DeviceRow";
import { ConnectDialog } from "@/components/dialogs/ConnectDialog";
import { Button } from "@/components/ui/Button";
import { EmptyState, SectionTitle } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { sortDevices, useApp } from "@/store/app";

export function DevicesPage() {
  const devicesMap = useApp((s) => s.devices);
  const local = useApp((s) => s.local);
  const settings = useApp((s) => s.settings);
  const saveSettings = useApp((s) => s.saveSettings);
  const [connectOpen, setConnectOpen] = useState(false);

  const all = useMemo(() => sortDevices(Object.values(devicesMap).filter((d) => d.trust !== "blocked")), [devicesMap]);
  const online = all.filter((d) => d.online);
  const offline = all.filter((d) => !d.online);
  const discovering = !!local?.discoveryActive;

  return (
    <div className="space-y-7">
      <div className="relative overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
        <div className="flex items-center gap-6 p-5">
          <RadarVisual active={discovering} count={online.length} />
          <div className="min-w-0 flex-1">
            <div className="text-[15px] font-semibold text-fg">
              {discovering ? "Looking for DropLink devices on your network" : "Automatic discovery is off"}
            </div>
            <p className="mt-1 max-w-xl text-[12.5px] leading-relaxed text-fg-muted">
              {discovering ? (
                <>
                  This computer is visible as <span className="font-medium text-fg">{local?.name}</span>. Devices appear
                  automatically when they join and disappear when they leave. New devices are never trusted until you pair
                  them.
                </>
              ) : (
                "Other devices cannot find this computer and it does not look for them. You can still connect by address."
              )}
            </p>
            <div className="mt-3.5 flex flex-wrap items-center gap-2">
              <Button size="sm" icon={Globe} onClick={() => setConnectOpen(true)}>
                Connect by address
              </Button>
              <label className="ml-1 flex items-center gap-2 text-xs text-fg-muted">
                <Switch size="sm" checked={!!settings?.discoveryEnabled} onChange={(v) => saveSettings({ discoveryEnabled: v })} />
                Automatic discovery
              </label>
            </div>
          </div>
          <div className="hidden shrink-0 text-right lg:block">
            <div className="text-2xs text-fg-subtle">This device</div>
            <div className="selectable mt-1 font-mono text-xs text-fg-muted">
              {local?.addresses.map((a) => (
                <div key={a}>
                  {a}:{local.port}
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      <section>
        <SectionTitle count={online.length}>Available now</SectionTitle>
        <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
          {online.length === 0 ? (
            <EmptyState
              icon={RadioTower}
              title={discovering ? "No other devices yet" : "Discovery is off"}
              description={
                discovering
                  ? "Install DropLink on your other computers and open it — they'll show up here within a few seconds. Make sure they're on the same network and the firewall allows DropLink."
                  : "Turn on automatic discovery or connect to a device by its address."
              }
            />
          ) : (
            <div className="divide-y divide-line">
              <AnimatePresence initial={false}>
                {online.map((d) => (
                  <DeviceRow key={d.id} device={d} />
                ))}
              </AnimatePresence>
            </div>
          )}
        </div>
      </section>

      {offline.length > 0 && (
        <section>
          <SectionTitle count={offline.length}>
            <span className="inline-flex items-center gap-1.5">
              <History size={12} /> Previously seen
            </span>
          </SectionTitle>
          <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            {offline.map((d) => (
              <DeviceRow key={d.id} device={d} />
            ))}
          </div>
        </section>
      )}
      <ConnectDialog open={connectOpen} onClose={() => setConnectOpen(false)} />
    </div>
  );
}

function RadarVisual({ active, count }: { active: boolean; count: number }) {
  const os = useApp((s) => s.local?.os ?? "unknown");
  return (
    <div className="relative flex h-[104px] w-[104px] shrink-0 items-center justify-center">
      {active &&
        [0, 1, 2].map((i) => (
          <motion.span
            key={i}
            className="absolute inset-0 rounded-full border border-accent/40"
            initial={{ scale: 0.35, opacity: 0.8 }}
            animate={{ scale: 1, opacity: 0 }}
            transition={{ duration: 3, repeat: Infinity, delay: i, ease: "easeOut" }}
          />
        ))}
      <span className="absolute inset-3 rounded-full border border-line" />
      <span className="absolute inset-7 rounded-full border border-line" />
      <div className="relative">
        <DeviceAvatar os={os} size={38} />
        {active ? null : <Radar size={14} className="absolute -right-2 -bottom-2 text-fg-subtle" />}
      </div>
      {count > 0 && (
        <span className="tabular absolute top-1 right-1 flex h-5 min-w-5 items-center justify-center rounded-full bg-success px-1 text-[10px] font-bold text-black">
          {count}
        </span>
      )}
    </div>
  );
}
