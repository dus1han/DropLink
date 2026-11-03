import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import {
  ArrowDownLeft,
  ClipboardCheck,
  ClipboardList,
  Copy,
  Image as ImageIcon,
  Laptop,
  MoreHorizontal,
  Send,
  Trash2,
  TriangleAlert,
} from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { Badge } from "@/components/ui/Badge";
import { Button, IconButton } from "@/components/ui/Button";
import { Menu, type MenuItem } from "@/components/ui/Menu";
import { EmptyState, Panel, Row, SectionTitle } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatBytes, formatRelative, osLabel } from "@/lib/format";
import type { ClipboardEntry, SendResult } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { sortDevices, useApp } from "@/store/app";

export function ClipboardPage() {
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const available = useApp((s) => s.clipboardAvailable);
  const devicesMap = useApp((s) => s.devices);
  const clips = useApp((s) => s.clips);
  const loadClips = useApp((s) => s.loadClips);
  const { fail, toast } = useApp.getState();
  const [sendingAll, setSendingAll] = useState(false);

  useEffect(() => {
    void loadClips();
  }, [loadClips]);

  const trusted = useMemo(() => sortDevices(Object.values(devicesMap).filter((d) => d.trust === "trusted")), [devicesMap]);
  const syncTargets = trusted.filter((d) => d.online && d.clipboardSync);

  const report = (results: SendResult[], what: string) => {
    const bad = results.filter((r) => !r.ok);
    if (!bad.length) toast({ tone: "success", title: `${what} sent to ${results.map((r) => r.deviceName).join(", ")}` });
    else toast({ tone: "error", title: `Could not send to ${bad.map((r) => r.deviceName).join(", ")}`, body: bad[0].error ?? undefined });
  };

  const sendNow = async () => {
    setSendingAll(true);
    try {
      report(await api.sendClipboard(syncTargets.map((d) => d.id)), "Clipboard");
    } catch (e) {
      fail(e, "Could not send clipboard");
    } finally {
      setSendingAll(false);
    }
  };

  if (!settings) return null;

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="space-y-6">
        {!available && (
          <div className="flex items-start gap-3 rounded-xl border border-warning/30 bg-warning-soft px-4 py-3 text-[12.5px] text-fg">
            <TriangleAlert size={16} className="mt-px shrink-0 text-warning" />
            The system clipboard could not be accessed on this computer, so clipboard sync is unavailable.
          </div>
        )}
        <div className="relative overflow-hidden rounded-xl border border-line bg-surface p-5 surface-highlight">
          <div className="pointer-events-none absolute -top-16 -left-16 h-52 w-52 rounded-full brand-gradient opacity-[0.07] blur-3xl" />
          <div className="flex items-start gap-4">
            <div
              className={cn(
                "flex h-11 w-11 shrink-0 items-center justify-center rounded-xl transition-colors",
                settings.clipboardAutoSync ? "brand-gradient text-white" : "bg-surface-2 text-fg-muted",
              )}
            >
              <ClipboardCheck size={20} />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <div className="text-[15px] font-semibold text-fg">Automatic sync</div>
                <Badge tone={settings.clipboardAutoSync ? "success" : "neutral"}>{settings.clipboardAutoSync ? "On" : "Off"}</Badge>
              </div>
              <p className="mt-1 max-w-lg text-[12.5px] leading-relaxed text-fg-muted">
                When on, text you copy here is available to paste on your trusted devices, and theirs here. Content that
                password managers mark as confidential is never synced.
              </p>
              {settings.clipboardAutoSync && (
                <p className="mt-2 text-xs text-fg-subtle">
                  {syncTargets.length
                    ? `Syncing with ${syncTargets.map((d) => d.name).join(", ")}`
                    : "No trusted device with clipboard sync is online right now."}
                </p>
              )}
            </div>
            <Switch checked={settings.clipboardAutoSync} disabled={!available} onChange={(v) => save({ clipboardAutoSync: v })} />
          </div>
          <div className="mt-4 flex flex-wrap gap-2 border-t border-line pt-4">
            <Button size="sm" icon={Send} disabled={!syncTargets.length || !available} loading={sendingAll} onClick={sendNow}>
              Send current clipboard now
            </Button>
            <span className="self-center text-2xs text-fg-subtle">Works even when automatic sync is off</span>
          </div>
        </div>

        <section>
          <SectionTitle
            count={clips?.length}
            actions={
              !!clips?.length && (
                <Button
                  size="sm"
                  variant="ghost"
                  icon={Trash2}
                  onClick={async () => {
                    if (await confirm({ title: "Clear clipboard history?", confirmLabel: "Clear", tone: "danger" })) {
                      await api.clearClipboardHistory().catch(fail);
                      await loadClips();
                    }
                  }}
                >
                  Clear
                </Button>
              )
            }
          >
            History
          </SectionTitle>
          <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            {!clips?.length ? (
              <EmptyState
                icon={ClipboardList}
                title={settings.clipboardHistoryEnabled ? "No clipboard history yet" : "Clipboard history is off"}
                description={
                  settings.clipboardHistoryEnabled
                    ? "Synced clipboard items from this and other devices appear here while automatic sync is on."
                    : "Turn history on in the panel on the right to keep recent synced items."
                }
              />
            ) : (
              <div className="divide-y divide-line">
                <AnimatePresence initial={false}>
                  {clips.map((c) => (
                    <ClipRow key={c.id} clip={c} targets={trusted.filter((d) => d.online)} onChanged={loadClips} report={report} />
                  ))}
                </AnimatePresence>
              </div>
            )}
          </div>
        </section>
      </div>

      <div className="space-y-4 lg:sticky lg:top-0 lg:self-start">
        <Panel title="Options" padded={false}>
          <Row title="Sync images" description="Screenshots and copied images, up to 40 megapixels.">
            <Switch checked={settings.clipboardImages} onChange={(v) => save({ clipboardImages: v })} />
          </Row>
          <Row title="Keep history" description={`The last ${settings.clipboardHistoryLimit} items. Images keep only their size.`}>
            <Switch checked={settings.clipboardHistoryEnabled} onChange={(v) => save({ clipboardHistoryEnabled: v })} />
          </Row>
        </Panel>
        <Panel title="Devices" subtitle="Choose which trusted devices share a clipboard with this one" padded={false}>
          {trusted.length === 0 ? (
            <EmptyState compact icon={Laptop} title="No trusted devices" description="Pair a device to share your clipboard." />
          ) : (
            trusted.map((d) => (
              <div key={d.id} className="flex items-center gap-3 px-4 py-2.5 [&+&]:border-t [&+&]:border-line">
                <DeviceAvatar os={d.os} size={28} online={d.online} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] font-medium text-fg">{d.name}</div>
                  <div className="text-2xs text-fg-subtle">{osLabel(d.os)}</div>
                </div>
                <Switch
                  size="sm"
                  checked={d.clipboardSync}
                  onChange={(v) => api.setDevicePermissions(d.id, { clipboardSync: v }).catch(fail)}
                  label={`Clipboard sync with ${d.name}`}
                />
              </div>
            ))
          )}
        </Panel>
      </div>
    </div>
  );
}

function ClipRow({
  clip,
  targets,
  onChanged,
  report,
}: {
  clip: ClipboardEntry;
  targets: { id: string; name: string }[];
  onChanged: () => void;
  report: (r: SendResult[], what: string) => void;
}) {
  const { fail, toast } = useApp.getState();
  const remote = clip.origin === "remote";
  const isText = clip.kind === "text";
  const menu: (MenuItem | "separator")[] = targets.length
    ? targets.map((t) => ({
        label: `Send to ${t.name}`,
        icon: Send,
        onSelect: () => api.sendClipEntry(clip.id, [t.id]).then((r) => report(r, "Item")).catch(fail),
      }))
    : [{ label: "No trusted devices online", onSelect: () => undefined, disabled: true }];

  return (
    <motion.div layout="position" initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="group flex gap-3 px-4 py-3">
      <div
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg",
          remote ? "bg-info-soft text-info" : "bg-surface-2 text-fg-muted",
        )}
      >
        {isText ? remote ? <ArrowDownLeft size={15} /> : <Copy size={14} /> : <ImageIcon size={15} />}
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2 text-xs text-fg-muted">
          <span className="font-medium text-fg">{remote ? `From ${clip.deviceName ?? "device"}` : "Copied here"}</span>
          <span className="text-fg-subtle">· {formatRelative(clip.createdAt)}</span>
        </div>
        {isText ? (
          <div className="selectable mt-1 line-clamp-3 font-mono text-[12px] leading-relaxed break-all whitespace-pre-wrap text-fg-muted">{clip.text}</div>
        ) : (
          <div className="mt-1 text-xs text-fg-muted">
            Image · {clip.width}×{clip.height} · {formatBytes(clip.byteSize)}
          </div>
        )}
      </div>
      <div className="flex shrink-0 items-start gap-0.5 opacity-60 transition-opacity group-hover:opacity-100">
        {isText && (
          <IconButton
            icon={Copy}
            label="Copy"
            size="sm"
            onClick={() =>
              api
                .copyClip(clip.id)
                .then(() => toast({ tone: "success", title: "Copied to clipboard" }))
                .catch(fail)
            }
          />
        )}
        {isText && <Menu items={menu} width={220} trigger={(p) => <IconButton icon={MoreHorizontal} label="Send to…" size="sm" {...p} />} />}
        <IconButton icon={Trash2} label="Delete" size="sm" tone="danger" onClick={() => api.deleteClip(clip.id).then(onChanged).catch(fail)} />
      </div>
    </motion.div>
  );
}
