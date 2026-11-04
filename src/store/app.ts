import { create } from "zustand";
import { listen } from "@tauri-apps/api/event";
import { api, errorMessage } from "@/lib/api";
import { formatBytes, isTerminal } from "@/lib/format";
import type {
  ClipboardEntry,
  Delivery,
  Device,
  EngineEvent,
  Group,
  IncomingRequest,
  LocalDevice,
  Os,
  Progress,
  Settings,
  TextMessage,
} from "@/lib/types";

export type PageId =
  | "overview"
  | "devices"
  | "send"
  | "text"
  | "receive"
  | "clipboard"
  | "activity"
  | "groups"
  | "trusted"
  | "settings";

export interface PairingState {
  pairingId: string;
  deviceId: string;
  deviceName: string;
  deviceOs: Os;
  code: string | null;
  role: "initiator" | "responder";
  phase: "connecting" | "code" | "confirmed" | "success" | "failed";
  message?: string;
}

export interface AppNotification {
  id: string;
  kind: "incoming" | "completed" | "failed" | "device" | "pairing" | "text" | "security";
  title: string;
  body: string;
  at: number;
  read: boolean;
  page?: PageId;
}

export interface Toast {
  id: string;
  tone: "success" | "error" | "info";
  title: string;
  body?: string;
}

interface NavigateOptions {
  /** Devices to preselect as recipients on the Send / Text pages. */
  recipients?: string[];
  groupId?: string;
}

interface AppState {
  ready: boolean;
  fatal: string | null;
  page: PageId;
  local: LocalDevice | null;
  settings: Settings | null;
  platform: Os;
  clipboardAvailable: boolean;
  autostartEnabled: boolean;
  sendToInstalled: boolean;
  devices: Record<string, Device>;
  groups: Group[];
  deliveries: Record<string, Delivery>;
  progress: Record<string, Progress>;
  incoming: IncomingRequest[];
  pairings: Record<string, PairingState>;
  notifications: AppNotification[];
  toasts: Toast[];
  staged: string[];
  preselect: { recipients: string[]; groupId?: string; nonce: number };
  texts: TextMessage[] | null;
  clips: ClipboardEntry[] | null;
  /** Bumped when something that feeds the dashboard numbers changes. */
  statsVersion: number;
  sidebarCollapsed: boolean;

  init: () => Promise<void>;
  navigate: (page: PageId, opts?: NavigateOptions) => void;
  saveSettings: (patch: Partial<Settings>) => Promise<void>;
  toast: (t: Omit<Toast, "id">) => void;
  dismissToast: (id: string) => void;
  fail: (e: unknown, title?: string) => void;
  stagePaths: (paths: string[], navigate?: boolean) => void;
  unstage: (path: string) => void;
  clearStaged: () => void;
  startPairing: (device: Device) => Promise<void>;
  dismissPairing: (pairingId: string) => void;
  markAllRead: () => void;
  clearNotifications: () => void;
  setSidebarCollapsed: (v: boolean) => void;
  setSendTo: (v: boolean) => void;
  loadTexts: () => Promise<void>;
  loadClips: () => Promise<void>;
  refreshGroups: () => Promise<void>;
}

const uid = () => Math.random().toString(36).slice(2, 10);
const notifiedDeliveries = new Set<string>();
let initStarted = false;

function readCollapsed(): boolean {
  try {
    return localStorage.getItem("droplink.sidebarCollapsed") === "1";
  } catch {
    return false;
  }
}

/** Marks every unfinished pairing with `deviceId` as successful. */
function resolvePairings(
  pairings: Record<string, PairingState>,
  deviceId: string,
  message: string,
): Record<string, PairingState> {
  let changed = false;
  const next = { ...pairings };
  for (const p of Object.values(pairings)) {
    if (p.deviceId === deviceId && p.phase !== "success" && p.phase !== "failed") {
      next[p.pairingId] = { ...p, phase: "success", message };
      changed = true;
    }
  }
  return changed ? next : pairings;
}

export const useApp = create<AppState>((set, get) => {
  const notify = (n: Omit<AppNotification, "id" | "at" | "read">) =>
    set((s) => ({
      notifications: [{ ...n, id: uid(), at: Date.now(), read: false }, ...s.notifications].slice(0, 100),
    }));

  const handle = (e: EngineEvent) => {
    switch (e.type) {
      case "deviceDiscovered":
        if (e.device.trust !== "blocked") {
          notify({ kind: "device", title: "New device nearby", body: `${e.device.name} is available`, page: "devices" });
        }
        set((s) => ({ devices: { ...s.devices, [e.device.id]: e.device } }));
        break;
      case "deviceUpdated":
        set((s) => ({
          devices: { ...s.devices, [e.device.id]: e.device },
          statsVersion: s.statsVersion + 1,
          // The device is trusted, so any dialog still waiting on a pairing
          // with it has in fact succeeded (e.g. a missed pairingFinished).
          pairings:
            e.device.trust === "trusted"
              ? resolvePairings(s.pairings, e.device.id, `${e.device.name} is now a trusted device`)
              : s.pairings,
        }));
        break;
      case "deviceDisconnected":
        set((s) => {
          const d = s.devices[e.deviceId];
          return d ? { devices: { ...s.devices, [e.deviceId]: { ...d, online: false } } } : {};
        });
        break;
      case "deviceRemoved":
        set((s) => {
          const devices = { ...s.devices };
          delete devices[e.deviceId];
          return { devices, statsVersion: s.statsVersion + 1 };
        });
        break;
      case "localDeviceUpdated":
        set({ local: e.device });
        break;
      case "pairingCode":
        set((s) => ({
          pairings: {
            ...s.pairings,
            [e.pairingId]: {
              pairingId: e.pairingId,
              deviceId: e.deviceId,
              deviceName: e.deviceName,
              deviceOs: e.deviceOs,
              code: e.code,
              role: e.role,
              phase: "code",
            },
          },
        }));
        if (e.role === "responder") {
          notify({ kind: "pairing", title: "Pairing request", body: `${e.deviceName} wants to pair with this device` });
        }
        break;
      case "pairingFinished":
        set((s) => {
          const prev = s.pairings[e.pairingId];
          if (!prev) {
            return e.success
              ? { pairings: resolvePairings(s.pairings, e.deviceId, e.message) }
              : {
                  toasts: [...s.toasts, { id: uid(), tone: "error" as const, title: "Pairing failed", body: e.message }],
                };
          }
          return {
            pairings: {
              ...s.pairings,
              [e.pairingId]: { ...prev, phase: e.success ? "success" : "failed", message: e.message },
            },
          };
        });
        break;
      case "incomingRequest":
        set((s) => ({ incoming: [...s.incoming.filter((r) => r.id !== e.request.id), e.request] }));
        notify({
          kind: "incoming",
          title: "Incoming transfer",
          body: `${e.request.deviceName} wants to send ${e.request.label} (${formatBytes(e.request.totalBytes)})`,
          page: "receive",
        });
        break;
      case "incomingRequestResolved":
        set((s) => ({ incoming: s.incoming.filter((r) => r.id !== e.requestId) }));
        break;
      case "deliveryUpdated": {
        const d = e.delivery;
        set((s) => {
          const progress = isTerminal(d.status) ? (({ [d.id]: _, ...rest }) => rest)(s.progress) : s.progress;
          return {
            deliveries: { ...s.deliveries, [d.id]: d },
            progress,
            statsVersion: isTerminal(d.status) ? s.statsVersion + 1 : s.statsVersion,
          };
        });
        const key = `${d.id}:${d.status}`;
        if ((d.status === "completed" || d.status === "failed") && !notifiedDeliveries.has(key)) {
          notifiedDeliveries.add(key);
          const outgoing = d.direction === "outgoing";
          notify(
            d.status === "completed"
              ? {
                  kind: "completed",
                  title: outgoing ? "Transfer complete" : "Transfer received",
                  body: `${d.label} ${outgoing ? "sent to" : "from"} ${d.deviceName}`,
                  page: "activity",
                }
              : {
                  kind: "failed",
                  title: "Transfer failed",
                  body: `${d.label} — ${d.error ?? "unknown error"}`,
                  page: "activity",
                },
          );
        }
        break;
      }
      case "deliveryProgress":
        set((s) => ({ progress: { ...s.progress, [e.progress.deliveryId]: e.progress } }));
        break;
      case "textReceived":
        set((s) => ({ texts: s.texts ? [e.message, ...s.texts] : s.texts }));
        notify({ kind: "text", title: `Text from ${e.message.deviceName}`, body: e.message.content.slice(0, 140), page: "text" });
        break;
      case "clipboardChanged":
        if (e.entry.id !== 0) {
          set((s) => ({ clips: s.clips ? [e.entry, ...s.clips.filter((c) => c.id !== e.entry.id)] : s.clips }));
        }
        break;
      case "securityEvent": {
        const important = ["blocked_connection", "identity_mismatch", "integrity_failure", "pairing_tampered", "session_hijack"];
        if (important.includes(e.event.kind)) {
          notify({ kind: "security", title: "Security notice", body: e.event.detail, page: "trusted" });
        }
        break;
      }
      case "groupsChanged":
        void get().refreshGroups();
        break;
    }
  };

  return {
    ready: false,
    fatal: null,
    page: "overview",
    local: null,
    settings: null,
    platform: "unknown",
    clipboardAvailable: false,
    autostartEnabled: false,
    sendToInstalled: false,
    devices: {},
    groups: [],
    deliveries: {},
    progress: {},
    incoming: [],
    pairings: {},
    notifications: [],
    toasts: [],
    staged: [],
    preselect: { recipients: [], nonce: 0 },
    texts: null,
    clips: null,
    statsVersion: 0,
    sidebarCollapsed: readCollapsed(),

    init: async () => {
      if (initStarted) return;
      initStarted = true;
      try {
        await listen<EngineEvent>("droplink://event", (ev) => handle(ev.payload));
        await listen<string[]>("droplink://open-paths", () => {
          // The paths are also queued on the Rust side; take them from there
          // so nothing is staged twice.
          void api.takePendingPaths().then((p) => get().stagePaths(p, true));
        });
        const b = await api.bootstrap();
        set({
          ready: true,
          local: b.local,
          settings: b.settings,
          platform: b.platform,
          clipboardAvailable: b.clipboardAvailable,
          autostartEnabled: b.autostartEnabled,
          sendToInstalled: b.sendToInstalled,
          devices: Object.fromEntries(b.devices.map((d) => [d.id, d])),
          groups: b.groups,
          deliveries: Object.fromEntries(b.deliveries.map((d) => [d.id, d])),
          incoming: b.incomingRequests,
        });
        if (b.pendingPaths.length) get().stagePaths(b.pendingPaths, true);
      } catch (e) {
        set({ fatal: errorMessage(e) });
      }
    },

    navigate: (page, opts) =>
      set((s) => ({
        page,
        preselect: opts ? { recipients: opts.recipients ?? [], groupId: opts.groupId, nonce: s.preselect.nonce + 1 } : s.preselect,
      })),

    saveSettings: async (patch) => {
      const current = get().settings;
      if (!current) return;
      const optimistic = { ...current, ...patch };
      set({ settings: optimistic });
      try {
        const saved = await api.updateSettings(optimistic);
        set({ settings: saved, autostartEnabled: saved.launchAtLogin });
      } catch (e) {
        set({ settings: current });
        get().fail(e, "Could not save settings");
      }
    },

    toast: (t) => {
      const id = uid();
      set((s) => ({ toasts: [...s.toasts, { ...t, id }].slice(-4) }));
      setTimeout(() => get().dismissToast(id), t.tone === "error" ? 6500 : 3800);
    },
    dismissToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
    fail: (e, title = "Something went wrong") => get().toast({ tone: "error", title, body: errorMessage(e) }),

    stagePaths: (paths, navigate = false) => {
      if (!paths.length) return;
      set((s) => ({
        staged: Array.from(new Set([...s.staged, ...paths])),
        page: navigate ? "send" : s.page,
      }));
    },
    unstage: (path) => set((s) => ({ staged: s.staged.filter((p) => p !== path) })),
    clearStaged: () => set({ staged: [] }),

    startPairing: async (device) => {
      try {
        const pairingId = await api.pairDevice(device.id);
        set((s) => ({
          pairings: {
            ...s.pairings,
            // Events may already have advanced this pairing.
            [pairingId]: s.pairings[pairingId] ?? {
              pairingId,
              deviceId: device.id,
              deviceName: device.name,
              deviceOs: device.os,
              code: null,
              role: "initiator",
              phase: "connecting",
            },
          },
        }));
      } catch (e) {
        get().fail(e, "Could not start pairing");
      }
    },
    dismissPairing: (pairingId) =>
      set((s) => {
        const pairings = { ...s.pairings };
        delete pairings[pairingId];
        return { pairings };
      }),

    markAllRead: () => set((s) => ({ notifications: s.notifications.map((n) => ({ ...n, read: true })) })),
    clearNotifications: () => set({ notifications: [] }),
    setSidebarCollapsed: (v) => {
      try {
        localStorage.setItem("droplink.sidebarCollapsed", v ? "1" : "0");
      } catch {
        /* storage unavailable: keep the in-memory value */
      }
      set({ sidebarCollapsed: v });
    },
    setSendTo: (v) => set({ sendToInstalled: v }),

    loadTexts: async () => {
      try {
        set({ texts: await api.listTexts() });
      } catch (e) {
        get().fail(e, "Could not load messages");
      }
    },
    loadClips: async () => {
      try {
        set({ clips: await api.clipboardHistory() });
      } catch (e) {
        get().fail(e, "Could not load clipboard history");
      }
    },
    refreshGroups: async () => {
      try {
        set({ groups: await api.listGroups() });
        const devices = await api.listDevices();
        set({ devices: Object.fromEntries(devices.map((d) => [d.id, d])) });
      } catch (e) {
        get().fail(e, "Could not load groups");
      }
    },
  };
});

/** Devices sorted: online first, then trusted, then most recently seen. */
export function sortDevices(list: Device[]): Device[] {
  return [...list].sort(
    (a, b) =>
      Number(b.online) - Number(a.online) ||
      Number(b.trust === "trusted") - Number(a.trust === "trusted") ||
      b.lastSeen - a.lastSeen,
  );
}
