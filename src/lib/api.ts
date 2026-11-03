import { invoke } from "@tauri-apps/api/core";
import type {
  Bootstrap,
  CacheInfo,
  ClipboardEntry,
  Delivery,
  Device,
  Group,
  HistoryEntry,
  HistoryQuery,
  IncomingRequest,
  LocalDevice,
  NetworkInterface,
  PathInfo,
  SecurityEvent,
  SendResult,
  Settings,
  Stats,
  TextMessage,
} from "./types";

/** Typed wrappers for every Rust command (src-tauri/src/commands.rs). */
export const api = {
  bootstrap: () => invoke<Bootstrap>("bootstrap"),

  listDevices: () => invoke<Device[]>("list_devices"),
  localDevice: () => invoke<LocalDevice>("local_device"),
  connectManual: (address: string) => invoke<Device>("connect_manual", { address }),
  probeDevice: (id: string) => invoke<boolean>("probe_device", { id }),
  renameDevice: (id: string, alias: string | null) => invoke<void>("rename_device", { id, alias }),
  forgetDevice: (id: string) => invoke<void>("forget_device", { id }),
  pairDevice: (id: string) => invoke<string>("pair_device", { id }),
  respondPairing: (pairingId: string, accept: boolean) => invoke<void>("respond_pairing", { pairingId, accept }),
  revokeDevice: (id: string) => invoke<void>("revoke_device", { id }),
  blockDevice: (id: string) => invoke<void>("block_device", { id }),
  unblockDevice: (id: string) => invoke<void>("unblock_device", { id }),
  setDevicePermissions: (id: string, perms: { autoAccept?: boolean; clipboardSync?: boolean }) =>
    invoke<void>("set_device_permissions", { id, autoAccept: perms.autoAccept, clipboardSync: perms.clipboardSync }),

  listGroups: () => invoke<Group[]>("list_groups"),
  createGroup: (name: string, color: string, members: string[]) =>
    invoke<Group>("create_group", { name, color, members }),
  updateGroup: (
    id: string,
    patch: Partial<Pick<Group, "name" | "color" | "autoAccept" | "clipboardSync">> & { members?: string[] },
  ) => invoke<void>("update_group", { id, patch }),
  deleteGroup: (id: string) => invoke<void>("delete_group", { id }),

  sendFiles: (paths: string[], recipients: string[]) => invoke<string>("send_files", { paths, recipients }),
  sendToGroup: (paths: string[], groupId: string) => invoke<string>("send_to_group", { paths, groupId }),
  inspectPaths: (paths: string[]) => invoke<PathInfo[]>("inspect_paths", { paths }),
  listDeliveries: (limit = 200) => invoke<Delivery[]>("list_deliveries", { limit }),
  pauseDelivery: (id: string) => invoke<void>("pause_delivery", { id }),
  resumeDelivery: (id: string) => invoke<void>("resume_delivery", { id }),
  cancelDelivery: (id: string) => invoke<void>("cancel_delivery", { id }),
  clearFinishedDeliveries: () => invoke<number>("clear_finished_deliveries"),
  listIncomingRequests: () => invoke<IncomingRequest[]>("list_incoming_requests"),
  respondIncoming: (id: string, accept: boolean, destDir?: string | null, overwrite?: boolean) =>
    invoke<void>("respond_incoming", { id, accept, destDir: destDir ?? null, overwrite: overwrite ?? null }),

  sendText: (content: string, recipients: string[]) => invoke<SendResult[]>("send_text", { content, recipients }),
  listTexts: (limit = 200) => invoke<TextMessage[]>("list_texts", { limit }),
  deleteText: (id: string) => invoke<void>("delete_text", { id }),
  clearTexts: () => invoke<void>("clear_texts"),
  saveTextFile: (path: string, content: string) => invoke<void>("save_text_file", { path, content }),

  clipboardHistory: (limit = 100) => invoke<ClipboardEntry[]>("clipboard_history", { limit }),
  sendClipboard: (recipients: string[]) => invoke<SendResult[]>("send_clipboard", { recipients }),
  sendClipEntry: (id: number, recipients: string[]) => invoke<SendResult[]>("send_clip_entry", { id, recipients }),
  copyText: (text: string) => invoke<void>("copy_text", { text }),
  copyClip: (id: number) => invoke<void>("copy_clip", { id }),
  deleteClip: (id: number) => invoke<void>("delete_clip", { id }),
  clearClipboardHistory: () => invoke<void>("clear_clipboard_history"),

  history: (query: HistoryQuery) => invoke<HistoryEntry[]>("history", { query }),
  clearHistory: () => invoke<number>("clear_history"),
  stats: (startOfDay: number) => invoke<Stats>("stats", { startOfDay }),
  securityEvents: (limit = 100) => invoke<SecurityEvent[]>("security_events", { limit }),

  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  networkInterfaces: () => invoke<NetworkInterface[]>("network_interfaces"),
  cacheInfo: () => invoke<CacheInfo>("cache_info"),
  clearPartialCache: () => invoke<number>("clear_partial_cache"),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  setSendTo: (enabled: boolean) => invoke<boolean>("set_send_to", { enabled }),
  takePendingPaths: () => invoke<string[]>("take_pending_paths"),
  quit: () => invoke<void>("quit_app"),
};

/** Errors from Rust arrive as plain strings. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return "Something went wrong";
}
