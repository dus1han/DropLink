import { create } from "zustand";

export interface ConfirmRequest {
  title: string;
  body?: string;
  confirmLabel?: string;
  tone?: "default" | "danger";
}

interface ConfirmState {
  request: (ConfirmRequest & { resolve: (v: boolean) => void }) | null;
  ask: (r: ConfirmRequest) => Promise<boolean>;
  settle: (v: boolean) => void;
}

/** Promise-based confirmation dialog rendered once in the shell. */
export const useConfirm = create<ConfirmState>((set, get) => ({
  request: null,
  ask: (r) =>
    new Promise<boolean>((resolve) => {
      get().request?.resolve(false);
      set({ request: { ...r, resolve } });
    }),
  settle: (v) => {
    get().request?.resolve(v);
    set({ request: null });
  },
}));

export const confirm = (r: ConfirmRequest) => useConfirm.getState().ask(r);
