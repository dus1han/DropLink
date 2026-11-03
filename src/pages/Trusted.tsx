import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  Ban,
  BadgeCheck,
  Fingerprint,
  KeyRound,
  Link2,
  Lock,
  MoreHorizontal,
  ScrollText,
  ShieldAlert,
  ShieldCheck,
  ShieldOff,
  type LucideIcon,
} from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { RenameDialog } from "@/components/dialogs/RenameDialog";
import { Button, IconButton } from "@/components/ui/Button";
import { Menu } from "@/components/ui/Menu";
import { EmptyState, SectionTitle } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { useDeviceActions } from "@/hooks/useDeviceActions";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatDateTime, formatRelative, osLabel } from "@/lib/format";
import type { Device, SecurityEvent } from "@/lib/types";
import { sortDevices, useApp } from "@/store/app";

export function TrustedPage() {
  const devicesMap = useApp((s) => s.devices);
  const local = useApp((s) => s.local);
  const navigate = useApp((s) => s.navigate);
  const trusted = useMemo(() => sortDevices(Object.values(devicesMap).filter((d) => d.trust === "trusted")), [devicesMap]);
  const blocked = useMemo(() => Object.values(devicesMap).filter((d) => d.trust === "blocked"), [devicesMap]);

  return (
    <div className="space-y-7">
      <div className="relative overflow-hidden rounded-xl border border-success/20 bg-surface p-5 surface-highlight">
        <div className="pointer-events-none absolute -top-24 -right-10 h-64 w-64 rounded-full bg-success opacity-[0.06] blur-3xl" />
        <div className="flex items-start gap-4">
          <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-success-soft text-success">
            <ShieldCheck size={22} />
          </div>
          <div className="min-w-0 flex-1">
            <div className="text-[15px] font-semibold text-fg">Connections are end-to-end encrypted and authenticated</div>
            <p className="mt-1 text-[12.5px] text-fg-muted">
              Files never pass through a cloud service. Only devices you have paired can send you anything.
            </p>
          </div>
        </div>
        <div className="mt-5 grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-4">
          <Fact icon={Lock} title="TLS 1.3" body="Mutual authentication on every connection" />
          <Fact icon={KeyRound} title="ECDSA P-256" body="Per-device key pair, generated locally" />
          <Fact icon={Link2} title="Verified pairing" body="6-digit code confirmed on both screens" />
          <Fact icon={BadgeCheck} title="SHA-256" body="Every received file checked end to end" />
        </div>
        <div className="mt-4 flex items-center gap-2 rounded-lg bg-surface-2 px-3 py-2">
          <Fingerprint size={14} className="shrink-0 text-fg-subtle" />
          <span className="text-2xs text-fg-subtle">This device's fingerprint</span>
          <span className="selectable ml-auto truncate font-mono text-[11.5px] text-fg-muted">{local?.fingerprint}</span>
        </div>
      </div>

      <section>
        <SectionTitle count={trusted.length}>Trusted devices</SectionTitle>
        <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
          {trusted.length === 0 ? (
            <EmptyState
              icon={ShieldCheck}
              title="No trusted devices"
              description="Pair with a device from Nearby Devices. Both screens show the same code — confirm it on each."
              action={
                <Button size="sm" variant="subtle" onClick={() => navigate("devices")}>
                  Find devices
                </Button>
              }
            />
          ) : (
            <div className="divide-y divide-line">
              <AnimatePresence initial={false}>
                {trusted.map((d) => (
                  <TrustedRow key={d.id} device={d} />
                ))}
              </AnimatePresence>
            </div>
          )}
        </div>
      </section>

      {blocked.length > 0 && (
        <section>
          <SectionTitle count={blocked.length}>Blocked</SectionTitle>
          <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            {blocked.map((d) => (
              <BlockedRow key={d.id} device={d} />
            ))}
          </div>
        </section>
      )}

      <SecurityLog />
    </div>
  );
}

function Fact({ icon: Icon, title, body }: { icon: LucideIcon; title: string; body: string }) {
  return (
    <div className="rounded-lg border border-line bg-surface-2/60 p-3">
      <Icon size={15} className="text-success" />
      <div className="mt-2 text-[12.5px] font-semibold text-fg">{title}</div>
      <div className="mt-0.5 text-2xs leading-relaxed text-fg-muted">{body}</div>
    </div>
  );
}

function TrustedRow({ device }: { device: Device }) {
  const { menu, renaming, setRenaming } = useDeviceActions(device);
  const fail = useApp((s) => s.fail);
  const groups = useApp((s) => s.groups);
  const viaGroup = groups.filter((g) => g.memberIds.includes(device.id));
  const set = (p: { autoAccept?: boolean; clipboardSync?: boolean }) => api.setDevicePermissions(device.id, p).catch(fail);

  return (
    <motion.div layout="position" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="flex items-center gap-4 px-4 py-3.5">
      <DeviceAvatar os={device.os} online={device.online} size={38} />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-semibold text-fg">{device.name}</div>
        <div className="mt-0.5 truncate text-xs text-fg-muted">
          {osLabel(device.os)} · paired {device.pairedAt ? formatRelative(device.pairedAt) : "—"}
          {viaGroup.length > 0 && <span className="text-fg-subtle"> · in {viaGroup.map((g) => g.name).join(", ")}</span>}
        </div>
        <div className="selectable mt-1 truncate font-mono text-[10.5px] text-fg-subtle">{device.fingerprint}</div>
      </div>
      <div className="flex shrink-0 items-center gap-5">
        <label className="flex flex-col items-center gap-1 text-2xs text-fg-subtle">
          <Switch size="sm" checked={device.autoAccept} onChange={(v) => set({ autoAccept: v })} label="Auto-accept files" />
          Auto-accept
        </label>
        <label className="flex flex-col items-center gap-1 text-2xs text-fg-subtle">
          <Switch size="sm" checked={device.clipboardSync} onChange={(v) => set({ clipboardSync: v })} label="Clipboard sync" />
          Clipboard
        </label>
        <Menu items={menu} trigger={(p) => <IconButton icon={MoreHorizontal} label="More actions" size="sm" {...p} />} />
      </div>
      <RenameDialog device={device} open={renaming} onClose={() => setRenaming(false)} />
    </motion.div>
  );
}

function BlockedRow({ device }: { device: Device }) {
  const { actions } = useDeviceActions(device);
  return (
    <div className="flex items-center gap-4 px-4 py-3">
      <DeviceAvatar os={device.os} size={34} />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium text-fg">{device.name}</div>
        <div className="selectable truncate font-mono text-[10.5px] text-fg-subtle">{device.fingerprint}</div>
      </div>
      <Button size="sm" onClick={actions.unblock}>
        Unblock
      </Button>
      <Button size="sm" variant="ghost" onClick={actions.forget}>
        Forget
      </Button>
    </div>
  );
}

const EVENT_STYLE: Record<string, { icon: LucideIcon; tone: string; label: string }> = {
  paired: { icon: Link2, tone: "text-success bg-success-soft", label: "Paired" },
  revoked: { icon: ShieldOff, tone: "text-warning bg-warning-soft", label: "Trust removed" },
  blocked: { icon: Ban, tone: "text-danger bg-danger-soft", label: "Blocked" },
  unblocked: { icon: ShieldCheck, tone: "text-fg-muted bg-surface-3", label: "Unblocked" },
  forgotten: { icon: ShieldOff, tone: "text-fg-muted bg-surface-3", label: "Forgotten" },
  blocked_connection: { icon: Ban, tone: "text-danger bg-danger-soft", label: "Connection refused" },
  rejected_untrusted: { icon: ShieldAlert, tone: "text-warning bg-warning-soft", label: "Untrusted request" },
  identity_mismatch: { icon: ShieldAlert, tone: "text-danger bg-danger-soft", label: "Identity mismatch" },
  integrity_failure: { icon: ShieldAlert, tone: "text-danger bg-danger-soft", label: "Checksum failed" },
  pairing_tampered: { icon: ShieldAlert, tone: "text-danger bg-danger-soft", label: "Pairing tampered" },
  invalid_offer: { icon: ShieldAlert, tone: "text-warning bg-warning-soft", label: "Invalid transfer" },
  session_hijack: { icon: ShieldAlert, tone: "text-danger bg-danger-soft", label: "Session reuse" },
  permission_changed: { icon: KeyRound, tone: "text-fg-muted bg-surface-3", label: "Permission changed" },
  group_permissions: { icon: KeyRound, tone: "text-fg-muted bg-surface-3", label: "Group permissions" },
};

function SecurityLog() {
  const [events, setEvents] = useState<SecurityEvent[] | null>(null);
  const devices = useApp((s) => s.devices);
  useEffect(() => {
    api.securityEvents(60).then(setEvents).catch(() => setEvents([]));
  }, [devices]);

  return (
    <section>
      <SectionTitle>Security log</SectionTitle>
      <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
        {!events?.length ? (
          <EmptyState compact icon={ScrollText} title="No security events" description="Pairings, blocks and refused connections are recorded here." />
        ) : (
          <div className="max-h-[420px] divide-y divide-line overflow-y-auto">
            {events.map((e) => {
              const s = EVENT_STYLE[e.kind] ?? { icon: ScrollText, tone: "text-fg-muted bg-surface-3", label: e.kind };
              return (
                <div key={e.id} className="flex items-center gap-3 px-4 py-2.5">
                  <div className={cn("flex h-7 w-7 shrink-0 items-center justify-center rounded-lg", s.tone)}>
                    <s.icon size={13} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-[12.5px] text-fg">
                      <span className="font-medium">{s.label}</span>
                      {e.deviceName && <span className="text-fg-muted"> · {e.deviceName}</span>}
                    </div>
                    <div className="truncate text-2xs text-fg-subtle">{e.detail}</div>
                  </div>
                  <span className="tabular shrink-0 text-2xs text-fg-subtle">{formatDateTime(e.createdAt)}</span>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}
