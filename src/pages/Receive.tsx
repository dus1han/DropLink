import { useMemo } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence } from "framer-motion";
import { Download, Eye, EyeOff, FolderOpen, Inbox, ShieldCheck } from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { TransferItem } from "@/components/TransferItem";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { EmptyState, Panel, Row, Segmented, SectionTitle } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { api } from "@/lib/api";
import { formatBytes, isActive, osLabel } from "@/lib/format";
import { useApp } from "@/store/app";

export function ReceivePage() {
  const local = useApp((s) => s.local);
  const settings = useApp((s) => s.settings);
  const save = useApp((s) => s.saveSettings);
  const deliveriesMap = useApp((s) => s.deliveries);
  const incoming = useApp((s) => s.incoming);
  const devices = useApp((s) => s.devices);
  const fail = useApp((s) => s.fail);

  const received = useMemo(
    () =>
      Object.values(deliveriesMap)
        .filter((d) => d.direction === "incoming")
        .sort((a, b) => Number(isActive(b.status)) - Number(isActive(a.status)) || (b.finishedAt ?? b.createdAt) - (a.finishedAt ?? a.createdAt))
        .slice(0, 30),
    [deliveriesMap],
  );
  const autoAcceptCount = Object.values(devices).filter((d) => d.trust === "trusted" && d.autoAccept).length;

  if (!local || !settings) return null;

  const chooseFolder = async () => {
    const r = await open({ directory: true, defaultPath: settings.downloadDir, title: "Save received files to" });
    if (typeof r === "string") await save({ downloadDir: r });
  };

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="space-y-6">
        <div className="relative overflow-hidden rounded-xl border border-line bg-surface p-5 surface-highlight">
          <div className="pointer-events-none absolute -top-20 -right-20 h-60 w-60 rounded-full brand-gradient opacity-[0.08] blur-3xl" />
          <div className="flex items-center gap-4">
            <DeviceAvatar os={local.os} size={52} online={local.addresses.length > 0} />
            <div className="min-w-0 flex-1">
              <div className="text-2xs font-medium tracking-wide text-fg-subtle uppercase">Ready to receive as</div>
              <div className="mt-0.5 truncate text-[18px] font-semibold tracking-tight text-fg">{local.name}</div>
              <div className="mt-1 flex flex-wrap items-center gap-1.5">
                <Badge tone={local.discoveryActive ? "success" : "neutral"} icon={local.discoveryActive ? Eye : EyeOff}>
                  {local.discoveryActive ? "Visible on the local network" : "Hidden from discovery"}
                </Badge>
                <Badge tone="accent" icon={ShieldCheck}>
                  Trusted devices only
                </Badge>
                <span className="text-2xs text-fg-subtle">{osLabel(local.os)}</span>
              </div>
            </div>
          </div>
          <div className="mt-4 grid grid-cols-2 gap-3 border-t border-line pt-4 text-xs md:grid-cols-3">
            <div>
              <div className="text-2xs text-fg-subtle">Addresses</div>
              <div className="selectable mt-0.5 font-mono text-fg-muted">{local.addresses.join(", ") || "—"}</div>
            </div>
            <div>
              <div className="text-2xs text-fg-subtle">Port</div>
              <div className="selectable mt-0.5 font-mono text-fg-muted">{local.port}</div>
            </div>
            <div className="col-span-2 md:col-span-1">
              <div className="text-2xs text-fg-subtle">Identity fingerprint</div>
              <div className="selectable mt-0.5 truncate font-mono text-fg-muted">{local.fingerprint}</div>
            </div>
          </div>
        </div>

        {incoming.length > 0 && (
          <section>
            <SectionTitle count={incoming.length}>Waiting for your approval</SectionTitle>
            <div className="divide-y divide-line overflow-hidden rounded-xl border border-warning/30 bg-warning-soft/40">
              {incoming.map((r) => (
                <div key={r.id} className="flex items-center gap-3 px-4 py-3">
                  <DeviceAvatar os={r.deviceOs} size={30} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium text-fg">{r.label}</div>
                    <div className="text-xs text-fg-muted">
                      From {r.deviceName} · {formatBytes(r.totalBytes)}
                    </div>
                  </div>
                  <Button size="sm" variant="ghost" onClick={() => api.respondIncoming(r.id, false).catch(fail)}>
                    Decline
                  </Button>
                  <Button size="sm" variant="primary" icon={Download} onClick={() => api.respondIncoming(r.id, true).catch(fail)}>
                    Accept
                  </Button>
                </div>
              ))}
            </div>
          </section>
        )}

        <section>
          <SectionTitle count={received.length}>Received</SectionTitle>
          <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            {received.length === 0 ? (
              <EmptyState
                icon={Inbox}
                title="Nothing received yet"
                description="When a trusted device sends you files they are saved to your download folder and listed here."
              />
            ) : (
              <div className="divide-y divide-line">
                <AnimatePresence initial={false}>
                  {received.map((d) => (
                    <TransferItem key={d.id} delivery={d} />
                  ))}
                </AnimatePresence>
              </div>
            )}
          </div>
        </section>
      </div>

      <div className="space-y-4 lg:sticky lg:top-0 lg:self-start">
        <Panel title="Receiving preferences" padded={false}>
          <Row
            title="Ask before accepting"
            description={
              autoAcceptCount
                ? `${autoAcceptCount} device${autoAcceptCount === 1 ? "" : "s"} can send without asking.`
                : "Every transfer needs your approval."
            }
          >
            <Switch checked={settings.requireApproval} onChange={(v) => save({ requireApproval: v })} />
          </Row>
          <div className="px-4 py-3.5">
            <div className="text-[13px] font-medium text-fg">Save files to</div>
            <div className="selectable mt-1 truncate rounded-md bg-surface-2 px-2.5 py-1.5 font-mono text-[11.5px] text-fg-muted">
              {settings.downloadDir}
            </div>
            <div className="mt-2 flex gap-2">
              <Button size="sm" icon={FolderOpen} onClick={chooseFolder}>
                Change
              </Button>
              <Button size="sm" variant="ghost" onClick={() => api.openPath(settings.downloadDir).catch(fail)}>
                Open folder
              </Button>
            </div>
          </div>
          <div className="border-t border-line px-4 py-3.5">
            <div className="text-[13px] font-medium text-fg">When a file already exists</div>
            <div className="mt-2">
              <Segmented
                size="sm"
                value={settings.conflictPolicy}
                onChange={(v) => save({ conflictPolicy: v })}
                options={[
                  { value: "rename", label: "Keep both" },
                  { value: "overwrite", label: "Replace" },
                ]}
              />
            </div>
          </div>
          <div className="border-t border-line">
            <Row title="Keep original dates" description="Preserve modification times of received files.">
              <Switch checked={settings.preserveTimestamps} onChange={(v) => save({ preserveTimestamps: v })} />
            </Row>
          </div>
        </Panel>
      </div>
    </div>
  );
}
