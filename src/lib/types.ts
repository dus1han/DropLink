// Mirrors the serde types in crates/droplink-core/src/models.rs, events.rs
// and src-tauri/src/commands.rs. Keep both sides in sync.

export type Os = "macos" | "windows" | "linux" | "unknown" | string;
export type TrustState = "unknown" | "trusted" | "blocked";
export type ConnectionType = "lan" | "manual";
export type Direction = "outgoing" | "incoming";
export type DeliveryStatus =
  | "queued"
  | "connecting"
  | "waiting"
  | "transferring"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled"
  | "rejected";

export interface Device {
  id: string;
  name: string;
  reportedName: string;
  alias: string | null;
  os: Os;
  appVersion: string | null;
  online: boolean;
  address: string | null;
  port: number | null;
  lastSeen: number;
  connectionType: ConnectionType;
  trust: TrustState;
  pairedAt: number | null;
  autoAccept: boolean;
  clipboardSync: boolean;
  groupIds: string[];
  fingerprint: string;
}

export interface LocalDevice {
  id: string;
  name: string;
  os: Os;
  port: number;
  fingerprint: string;
  addresses: string[];
  appVersion: string;
  discoveryActive: boolean;
}

export interface Delivery {
  id: string;
  sessionId: string;
  direction: Direction;
  label: string;
  deviceId: string;
  deviceName: string;
  deviceOs: Os;
  status: DeliveryStatus;
  bytesDone: number;
  totalBytes: number;
  filesDone: number;
  totalFiles: number;
  error: string | null;
  verified: boolean | null;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  destDir: string | null;
  firstFile: string | null;
}

export interface Progress {
  deliveryId: string;
  bytesDone: number;
  totalBytes: number;
  filesDone: number;
  totalFiles: number;
  speedBps: number;
  etaSeconds: number | null;
  currentFile: string | null;
}

export interface IncomingRequest {
  id: string;
  deviceId: string;
  deviceName: string;
  deviceOs: Os;
  label: string;
  totalBytes: number;
  totalFiles: number;
  items: { path: string; size: number; isDir: boolean }[];
  conflicts: number;
  destDir: string;
  createdAt: number;
}

export interface HistoryEntry {
  id: number;
  deliveryId: string;
  direction: Direction;
  kind: "files" | "folder" | "text";
  name: string;
  totalBytes: number;
  fileCount: number;
  senderId: string;
  senderName: string;
  recipientId: string;
  recipientName: string;
  status: DeliveryStatus;
  error: string | null;
  verified: boolean | null;
  startedAt: number | null;
  finishedAt: number;
  durationMs: number | null;
  retryable: boolean;
}

export interface HistoryQuery {
  search?: string;
  status?: string;
  deviceId?: string;
  direction?: string;
  fromMs?: number;
  toMs?: number;
  limit?: number;
}

export interface TextMessage {
  id: string;
  direction: Direction;
  deviceId: string;
  deviceName: string;
  content: string;
  createdAt: number;
}

export interface ClipboardEntry {
  id: number;
  origin: "local" | "remote";
  deviceId: string | null;
  deviceName: string | null;
  kind: "text" | "image";
  text: string | null;
  width: number | null;
  height: number | null;
  byteSize: number;
  createdAt: number;
}

export interface Group {
  id: string;
  name: string;
  color: string;
  autoAccept: boolean;
  clipboardSync: boolean;
  memberIds: string[];
  createdAt: number;
}

export interface SecurityEvent {
  id: number;
  kind: string;
  deviceId: string | null;
  deviceName: string | null;
  detail: string;
  createdAt: number;
}

export interface Stats {
  knownDevices: number;
  trustedDevices: number;
  onlineDevices: number;
  activeTransfers: number;
  filesToday: number;
  bytesToday: number;
  bytesTotal: number;
  transfersTotal: number;
}

export interface NotificationSettings {
  enabled: boolean;
  incomingRequests: boolean;
  transferCompleted: boolean;
  transferFailed: boolean;
  deviceDiscovered: boolean;
  pairingRequests: boolean;
  deviceDisconnected: boolean;
}

export type Theme = "system" | "dark" | "light";

export interface Settings {
  deviceName: string;
  downloadDir: string;
  theme: Theme;
  language: string;
  launchAtLogin: boolean;
  closeToTray: boolean;
  discoveryEnabled: boolean;
  port: number;
  preferredInterface: string | null;
  allowRemoteConnections: boolean;
  requireApproval: boolean;
  conflictPolicy: "rename" | "overwrite";
  maxConcurrentTransfers: number;
  bandwidthLimitMbps: number;
  preserveTimestamps: boolean;
  clipboardAutoSync: boolean;
  clipboardImages: boolean;
  clipboardHistoryEnabled: boolean;
  clipboardHistoryLimit: number;
  historyRetentionDays: number;
  notifications: NotificationSettings;
}

export interface SendResult {
  deviceId: string;
  deviceName: string;
  ok: boolean;
  error: string | null;
}

export interface PathInfo {
  path: string;
  name: string;
  isDir: boolean;
  size: number;
  fileCount: number;
}

export interface CacheInfo {
  partialTransfers: number;
  partialBytes: number;
  dataDir: string;
  logDir: string;
}

export interface NetworkInterface {
  name: string;
  address: string;
}

export interface Bootstrap {
  local: LocalDevice;
  settings: Settings;
  devices: Device[];
  groups: Group[];
  deliveries: Delivery[];
  incomingRequests: IncomingRequest[];
  platform: Os;
  clipboardAvailable: boolean;
  autostartEnabled: boolean;
  sendToInstalled: boolean;
  pendingPaths: string[];
}

export type EngineEvent =
  | { type: "deviceDiscovered"; device: Device }
  | { type: "deviceUpdated"; device: Device }
  | { type: "deviceDisconnected"; deviceId: string; name: string }
  | { type: "deviceRemoved"; deviceId: string }
  | { type: "localDeviceUpdated"; device: LocalDevice }
  | {
      type: "pairingCode";
      pairingId: string;
      deviceId: string;
      deviceName: string;
      deviceOs: Os;
      code: string;
      role: "initiator" | "responder";
    }
  | { type: "pairingFinished"; pairingId: string; deviceId: string; deviceName: string; success: boolean; message: string }
  | { type: "incomingRequest"; request: IncomingRequest }
  | { type: "incomingRequestResolved"; requestId: string }
  | { type: "deliveryUpdated"; delivery: Delivery }
  | { type: "deliveryProgress"; progress: Progress }
  | { type: "textReceived"; message: TextMessage }
  | { type: "clipboardChanged"; entry: ClipboardEntry }
  | { type: "securityEvent"; event: SecurityEvent }
  | { type: "groupsChanged" };
