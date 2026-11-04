import { create } from "zustand";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { confirm } from "@/store/confirm";

/**
 * Self-update from GitHub Releases. Builds are signed in CI; the updater
 * verifies the signature against the public key in tauri.conf.json before
 * installing, so only official builds can be applied.
 */
type Status = "idle" | "checking" | "upToDate" | "available" | "downloading" | "installing" | "error";

interface UpdateState {
  status: Status;
  version: string | null;
  notes: string | null;
  progress: number | null;
  error: string | null;
  /** `silent`: no prompt or error if nothing is found (startup check). */
  checkForUpdates: (silent?: boolean) => Promise<void>;
  install: () => Promise<void>;
}

let pending: Update | null = null;

export const useUpdate = create<UpdateState>((set, get) => ({
  status: "idle",
  version: null,
  notes: null,
  progress: null,
  error: null,

  checkForUpdates: async (silent = false) => {
    const busy = get().status;
    if (busy === "checking" || busy === "downloading" || busy === "installing") return;
    set({ status: "checking", error: null });
    try {
      const update = await check();
      if (!update) {
        pending = null;
        set({ status: "upToDate", version: null, notes: null });
        return;
      }
      pending = update;
      set({ status: "available", version: update.version, notes: update.body ?? null });
      const ok = await confirm({
        title: `DropLink ${update.version} is available`,
        body: "Download and install it now? DropLink restarts when the update is installed; transfers in progress resume afterwards.",
        confirmLabel: "Install & restart",
      });
      if (ok) await get().install();
    } catch (e) {
      // Offline or no release published yet: stay quiet on the startup check.
      set({ status: silent ? "idle" : "error", error: String(e) });
    }
  },

  install: async () => {
    if (!pending) return;
    let total = 0;
    let done = 0;
    set({ status: "downloading", progress: 0, error: null });
    try {
      await pending.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          set({ progress: total ? done / total : null });
        } else if (ev.event === "Finished") set({ status: "installing", progress: 1 });
      });
      await relaunch();
    } catch (e) {
      set({ status: "error", error: String(e) });
    }
  },
}));
