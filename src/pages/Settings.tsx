import { useEffect, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { motion } from "framer-motion";
import {
  Bell,
  ClipboardList,
  FolderOpen,
  Globe,
  HardDrive,
  Info,
  MonitorCog,
  Network,
  Shield,
  SlidersHorizontal,
  type LucideIcon,
} from "lucide-react";
import { ConnectDialog } from "@/components/dialogs/ConnectDialog";
import { LogoMark } from "@/components/Logo";
import { Button } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { Row, Segmented } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatBytes, osLabel, pluralize } from "@/lib/format";
import type { CacheInfo, NetworkInterface, NotificationSettings } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";

type Section = "general" | "network" | "transfers" | "clipboard" | "security" | "notifications" | "storage" | "about";

const SECTIONS: { id: Section; label: string; icon: LucideIcon }[] = [
  { id: "general", label: "General", icon: MonitorCog },
  { id: "network", label: "Network", icon: Network },
  { id: "transfers", label: "Transfers", icon: SlidersHorizontal },
  { id: "clipboard", label: "Clipboard", icon: ClipboardList },
  { id: "security", label: "Security", icon: Shield },
  { id: "notifications", label: "Notifications", icon: Bell },
  { id: "storage", label: "Storage", icon: HardDrive },
  { id: "about", label: "About", icon: Info },
];

export function SettingsPage() {
  const [section, setSection] = useState<Section>("general");
  return (
    <div className="grid gap-6 md:grid-cols-[200px_minmax(0,1fr)]">
      <nav className="md:sticky md:top-0 md:self-start">
        {SECTIONS.map((s) => (
          <button
            key={s.id}
            onClick={() => setSection(s.id)}
            className={cn(
              "relative mb-0.5 flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-[13px] font-medium transition-colors",
              section === s.id ? "text-fg" : "text-fg-muted hover:bg-surface-2 hover:text-fg",
            )}
          >
            {section === s.id && (
              <motion.span layoutId="settings-nav" className="absolute inset-0 rounded-md bg-surface-2" transition={{ type: "spring", stiffness: 500, damping: 40 }} />
            )}
            <s.icon size={15} className="relative" />
            <span className="relative">{s.label}</span>
          </button>
        ))}
      </nav>
      <motion.div key={section} initial={{ opacity: 0, x: 4 }} animate={{ opacity: 1, x: 0 }} transition={{ duration: 0.18 }} className="max-w-[720px] space-y-5">
        {section === "general" && <General />}
        {section === "network" && <NetworkSection />}
        {section === "transfers" && <Transfers />}
        {section === "clipboard" && <ClipboardSection />}
        {section === "security" && <Security />}
        {section === "notifications" && <Notifications />}
        {section === "storage" && <Storage />}
        {section === "about" && <About />}
      </motion.div>
    </div>
  );
}

function Card({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="text-[13.5px] font-semibold text-fg">{title}</h2>
      {description && <p className="mt-0.5 text-xs text-fg-muted">{description}</p>}
      <div className="mt-3 overflow-hidden rounded-xl border border-line bg-surface surface-highlight">{children}</div>
    </section>
  );
}

function useSettings() {
  const settings = useApp((s) => s.settings)!;
  const save = useApp((s) => s.saveSettings);
  return { s: settings, save };
}

function General() {
  const { s, save } = useSettings();
  const platform = useApp((st) => st.platform);
  const sendTo = useApp((st) => st.sendToInstalled);
  const setSendTo = useApp((st) => st.setSendTo);
  const fail = useApp((st) => st.fail);
  const [name, setName] = useState(s.deviceName);
  useEffect(() => setName(s.deviceName), [s.deviceName]);

  const chooseFolder = async () => {
    const r = await open({ directory: true, defaultPath: s.downloadDir, title: "Default download folder" });
    if (typeof r === "string") await save({ downloadDir: r });
  };

  return (
    <>
      <Card title="This device">
        <Row title="Device name" description="How this computer appears to your other devices.">
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              if (name.trim() && name !== s.deviceName) void save({ deviceName: name.trim() });
            }}
          >
            <div className="w-56">
              <Input value={name} onChange={(e) => setName(e.target.value)} maxLength={64} onBlur={() => name.trim() && name !== s.deviceName && save({ deviceName: name.trim() })} />
            </div>
          </form>
        </Row>
        <Row title="Default download folder" description={<span className="selectable font-mono text-[11.5px]">{s.downloadDir}</span>}>
          <Button size="sm" icon={FolderOpen} onClick={chooseFolder}>
            Change
          </Button>
        </Row>
      </Card>
      <Card title="Startup">
        <Row title="Open at login" description="Start DropLink minimised when you sign in, so it can receive right away.">
          <Switch checked={s.launchAtLogin} onChange={(v) => save({ launchAtLogin: v })} />
        </Row>
        <Row title="Keep running in the background" description="Closing the window keeps DropLink in the system tray, ready to receive.">
          <Switch checked={s.closeToTray} onChange={(v) => save({ closeToTray: v })} />
        </Row>
        {platform === "windows" && (
          <Row title="Add to Explorer’s “Send to” menu" description="Right-click files in Explorer and choose Send to → DropLink.">
            <Switch
              checked={sendTo}
              onChange={(v) =>
                api
                  .setSendTo(v)
                  .then(setSendTo)
                  .catch((e) => fail(e, "Could not update the Send to menu"))
              }
            />
          </Row>
        )}
      </Card>
      <Card title="Appearance">
        <Row title="Theme">
          <Segmented
            size="sm"
            value={s.theme}
            onChange={(v) => save({ theme: v })}
            options={[
              { value: "system", label: "System" },
              { value: "dark", label: "Dark" },
              { value: "light", label: "Light" },
            ]}
          />
        </Row>
        <Row title="Language" description="DropLink is currently available in English.">
          <div className="w-36">
            <Select value={s.language} onChange={(e) => save({ language: e.target.value })}>
              <option value="en">English</option>
            </Select>
          </div>
        </Row>
      </Card>
    </>
  );
}

function NetworkSection() {
  const { s, save } = useSettings();
  const local = useApp((st) => st.local);
  const [ifaces, setIfaces] = useState<NetworkInterface[]>([]);
  const [port, setPort] = useState(String(s.port));
  const [connectOpen, setConnectOpen] = useState(false);
  useEffect(() => {
    api.networkInterfaces().then(setIfaces).catch(() => undefined);
  }, []);
  const portNum = Number(port);
  const portValid = Number.isInteger(portNum) && portNum >= 1024 && portNum <= 65535;

  return (
    <>
      <Card title="Discovery" description="Uses mDNS (Bonjour) on the local network.">
        <Row title="Automatic discovery" description="Advertise this computer and find other DropLink devices nearby.">
          <Switch checked={s.discoveryEnabled} onChange={(v) => save({ discoveryEnabled: v })} />
        </Row>
        <Row title="Network interface" description="Limit discovery to one adapter, e.g. to avoid a VPN or virtual network.">
          <div className="w-56">
            <Select value={s.preferredInterface ?? ""} onChange={(e) => save({ preferredInterface: e.target.value || null })}>
              <option value="">All interfaces</option>
              {ifaces.map((i) => (
                <option key={i.name + i.address} value={i.name}>
                  {i.name} — {i.address}
                </option>
              ))}
            </Select>
          </div>
        </Row>
      </Card>
      <Card title="Connections">
        <Row title="Transfer port" description={`Currently listening on ${local?.port}. If the port is busy the next free one is used.`}>
          <div className="w-24">
            <Input value={port} onChange={(e) => setPort(e.target.value.replace(/\D/g, ""))} className="tabular font-mono" />
          </div>
          <Button size="sm" disabled={!portValid || portNum === s.port} onClick={() => save({ port: portNum })}>
            Apply
          </Button>
        </Row>
        <Row title="Manual connection" description="Reach a device by IP address or host name when discovery cannot see it.">
          <Button size="sm" icon={Globe} onClick={() => setConnectOpen(true)}>
            Connect by address
          </Button>
        </Row>
      </Card>
      <Card
        title="Across networks"
        description="Transfers are always direct, peer-to-peer connections. DropLink does not use relay or cloud servers, so your files never pass through a third party."
      >
        <Row
          title="Accept connections from other networks"
          description="Allow devices outside your local network (public IP addresses) to connect — for port forwarding setups. Not needed for VPNs like Tailscale, which use private ranges."
        >
          <Switch checked={s.allowRemoteConnections} onChange={(v) => save({ allowRemoteConnections: v })} />
        </Row>
        <div className="border-t border-line px-4 py-3 text-xs leading-relaxed text-fg-muted">
          To send across networks, connect both computers to the same VPN, or forward TCP port{" "}
          <span className="font-mono text-fg">{local?.port}</span> to this computer and use <em>Connect by address</em>{" "}
          on the other one. Pairing and encryption work exactly as on a local network.
        </div>
      </Card>
      <ConnectDialog open={connectOpen} onClose={() => setConnectOpen(false)} />
    </>
  );
}

function Transfers() {
  const { s, save } = useSettings();
  const [limit, setLimit] = useState(s.bandwidthLimitMbps ? String(s.bandwidthLimitMbps) : "");
  return (
    <>
      <Card title="Incoming">
        <Row title="Ask before accepting" description="Devices or groups with auto-accept skip the prompt. Untrusted devices are always refused.">
          <Switch checked={s.requireApproval} onChange={(v) => save({ requireApproval: v })} />
        </Row>
        <Row title="When a file already exists">
          <Segmented
            size="sm"
            value={s.conflictPolicy}
            onChange={(v) => save({ conflictPolicy: v })}
            options={[
              { value: "rename", label: "Keep both" },
              { value: "overwrite", label: "Replace" },
            ]}
          />
        </Row>
        <Row title="Keep original dates" description="Preserve file and folder modification times.">
          <Switch checked={s.preserveTimestamps} onChange={(v) => save({ preserveTimestamps: v })} />
        </Row>
      </Card>
      <Card title="Performance">
        <Row title="Simultaneous transfers" description="Deliveries beyond this number wait in the queue.">
          <div className="w-20">
            <Select value={s.maxConcurrentTransfers} onChange={(e) => save({ maxConcurrentTransfers: Number(e.target.value) })}>
              {[1, 2, 3, 4, 6, 8].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </Select>
          </div>
        </Row>
        <Row title="Upload speed limit" description="Megabytes per second shared by all outgoing transfers. Leave empty for no limit.">
          <form
            className="flex items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              void save({ bandwidthLimitMbps: Number(limit) || 0 });
            }}
          >
            <div className="w-24">
              <Input
                value={limit}
                placeholder="Unlimited"
                onChange={(e) => setLimit(e.target.value.replace(/[^\d.]/g, ""))}
                onBlur={() => save({ bandwidthLimitMbps: Number(limit) || 0 })}
                className="tabular"
              />
            </div>
            <span className="text-xs text-fg-subtle">MB/s</span>
          </form>
        </Row>
      </Card>
    </>
  );
}

function ClipboardSection() {
  const { s, save } = useSettings();
  return (
    <Card title="Universal clipboard" description="Per-device permissions are on the Clipboard and Trusted Devices pages.">
      <Row title="Automatic sync" description="Share copied content with trusted devices automatically.">
        <Switch checked={s.clipboardAutoSync} onChange={(v) => save({ clipboardAutoSync: v })} />
      </Row>
      <Row title="Include images">
        <Switch checked={s.clipboardImages} onChange={(v) => save({ clipboardImages: v })} />
      </Row>
      <Row title="Keep clipboard history" description="Turning this off deletes the stored history.">
        <Switch checked={s.clipboardHistoryEnabled} onChange={(v) => save({ clipboardHistoryEnabled: v })} />
      </Row>
      <Row title="History size">
        <div className="w-24">
          <Select value={s.clipboardHistoryLimit} disabled={!s.clipboardHistoryEnabled} onChange={(e) => save({ clipboardHistoryLimit: Number(e.target.value) })}>
            {[20, 50, 100, 200].map((n) => (
              <option key={n} value={n}>
                {n} items
              </option>
            ))}
          </Select>
        </div>
      </Row>
    </Card>
  );
}

function Security() {
  const local = useApp((st) => st.local);
  const devices = useApp((st) => st.devices);
  const navigate = useApp((st) => st.navigate);
  const trusted = Object.values(devices).filter((d) => d.trust === "trusted").length;
  const blocked = Object.values(devices).filter((d) => d.trust === "blocked").length;
  return (
    <>
      <Card title="Device identity">
        <Row title="Fingerprint" description="Compare this with what other devices show for this computer.">
          <span className="selectable font-mono text-[11.5px] text-fg-muted">{local?.fingerprint}</span>
        </Row>
        <Row title="Key" description="ECDSA P-256 key pair stored only on this computer, with a self-signed certificate." />
      </Card>
      <Card title="Encryption">
        <Row title="Transport" description="TLS 1.3 with mutual certificate authentication. Older protocol versions are refused." />
        <Row title="Integrity" description="Each file is hashed with SHA-256 on both sides while streaming; mismatches are discarded." />
        <Row title="Pairing" description="A 6-digit code derived from both device keys and fresh random values, confirmed on both screens." />
      </Card>
      <Card title="Trust">
        <Row title="Trusted devices" description={`${pluralize(trusted, "device")} trusted, ${blocked} blocked.`}>
          <Button size="sm" onClick={() => navigate("trusted")}>
            Manage
          </Button>
        </Row>
      </Card>
    </>
  );
}

function Notifications() {
  const { s, save } = useSettings();
  const n = s.notifications;
  const set = (patch: Partial<NotificationSettings>) => save({ notifications: { ...n, ...patch } });
  const rows: [keyof NotificationSettings, string, string][] = [
    ["incomingRequests", "Incoming transfers and messages", "When a device wants to send you files or text."],
    ["pairingRequests", "Pairing requests", "When a device asks to pair with this one."],
    ["transferCompleted", "Transfer completed", "When a transfer finishes successfully."],
    ["transferFailed", "Transfer failed", "When a transfer cannot be completed."],
    ["deviceDiscovered", "New device discovered", "When a DropLink device appears for the first time."],
    ["deviceDisconnected", "Device disconnected", "When a trusted device goes offline."],
  ];
  return (
    <Card title="Desktop notifications" description="Shown when DropLink is in the background.">
      <Row title="Allow notifications">
        <Switch checked={n.enabled} onChange={(v) => set({ enabled: v })} />
      </Row>
      {rows.map(([key, title, desc]) => (
        <Row key={key} title={title} description={desc}>
          <Switch checked={n[key] as boolean} disabled={!n.enabled} onChange={(v) => set({ [key]: v })} />
        </Row>
      ))}
    </Card>
  );
}

function Storage() {
  const { s, save } = useSettings();
  const fail = useApp((st) => st.fail);
  const toast = useApp((st) => st.toast);
  const [cache, setCache] = useState<CacheInfo | null>(null);
  const refresh = () => api.cacheInfo().then(setCache).catch(() => undefined);
  useEffect(() => {
    void refresh();
  }, []);

  return (
    <>
      <Card title="Transfer history">
        <Row title="Keep history for">
          <div className="w-32">
            <Select value={s.historyRetentionDays} onChange={(e) => save({ historyRetentionDays: Number(e.target.value) })}>
              {[7, 30, 90, 365, 3650].map((d) => (
                <option key={d} value={d}>
                  {d === 3650 ? "Forever" : d === 365 ? "1 year" : `${d} days`}
                </option>
              ))}
            </Select>
          </div>
        </Row>
        <Row title="Clear history" description="Deletes transfer records. Received files are not touched.">
          <Button
            size="sm"
            variant="danger"
            onClick={async () => {
              if (await confirm({ title: "Clear all transfer history?", confirmLabel: "Clear", tone: "danger" })) {
                api
                  .clearHistory()
                  .then((n) => toast({ tone: "success", title: `Removed ${pluralize(n, "record")}` }))
                  .catch(fail);
              }
            }}
          >
            Clear
          </Button>
        </Row>
      </Card>
      <Card title="Cache">
        <Row
          title="Partial downloads"
          description={
            cache
              ? cache.partialTransfers
                ? `${pluralize(cache.partialTransfers, "paused transfer")} keeping ${formatBytes(cache.partialBytes)} so they can resume. Unused data is removed after 7 days.`
                : "No partial data stored."
              : "…"
          }
        >
          <Button
            size="sm"
            disabled={!cache?.partialTransfers}
            onClick={async () => {
              if (await confirm({ title: "Discard partial downloads?", body: "Paused incoming transfers will no longer be resumable.", confirmLabel: "Discard", tone: "danger" })) {
                await api.clearPartialCache().catch(fail);
                void refresh();
              }
            }}
          >
            Discard
          </Button>
        </Row>
        {cache && (
          <>
            <Row title="App data" description={<span className="selectable font-mono text-[11.5px]">{cache.dataDir}</span>}>
              <Button size="sm" variant="ghost" icon={FolderOpen} onClick={() => api.openPath(cache.dataDir).catch(fail)}>
                Open
              </Button>
            </Row>
            <Row title="Logs" description={<span className="selectable font-mono text-[11.5px]">{cache.logDir}</span>}>
              <Button size="sm" variant="ghost" icon={FolderOpen} onClick={() => api.openPath(cache.logDir).catch(fail)}>
                Open
              </Button>
            </Row>
          </>
        )}
      </Card>
    </>
  );
}

function UpdateButton() {
  const { status, version, progress, error, checkForUpdates, install } = useUpdate();
  const busy = status === "checking" || status === "downloading" || status === "installing";
  const label =
    status === "available"
      ? `Install ${version} & restart`
      : status === "downloading"
        ? `Downloading${progress != null ? ` ${Math.round(progress * 100)}%` : "…"}`
        : status === "installing"
          ? "Installing…"
          : "Check for updates";
  const note =
    status === "upToDate" ? "DropLink is up to date." : status === "error" ? `Update failed: ${error}` : null;
  return (
    <>
      <Button
        size="sm"
        variant="primary"
        loading={busy}
        onClick={() => (status === "available" ? install() : checkForUpdates())}
      >
        {label}
      </Button>
      {note && <span className="text-xs text-fg-muted">{note}</span>}
    </>
  );
}

function About() {
  const local = useApp((st) => st.local);
  const platform = useApp((st) => st.platform);
  return (
    <div className="rounded-xl border border-line bg-surface p-6 surface-highlight">
      <div className="flex items-center gap-4">
        <LogoMark size={52} />
        <div>
          <div className="text-[18px] font-semibold tracking-tight text-fg">DropLink</div>
          <div className="tabular text-xs text-fg-muted">
            Version {local?.appVersion} · {osLabel(platform)}
          </div>
        </div>
      </div>
      <p className="mt-4 max-w-lg text-[12.5px] leading-relaxed text-fg-muted">
        Send files, folders, text and clipboard contents between your computers — directly, encrypted end to end, with
        no cloud storage.
      </p>
      <p className="mt-3 text-xs text-fg-subtle">Developed by Dushan Fernando · © 2026 Dushan Fernando</p>
      <div className="mt-5 flex flex-wrap items-center gap-2">
        <UpdateButton />
        <Button size="sm" variant="danger" onClick={() => api.quit()}>
          Quit DropLink
        </Button>
      </div>
    </div>
  );
}
