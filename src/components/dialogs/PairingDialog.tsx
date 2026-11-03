import { useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { CheckCircle2, ShieldAlert, ShieldCheck, XCircle } from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Spinner } from "@/components/ui/Spinner";
import { api } from "@/lib/api";
import { osLabel } from "@/lib/format";
import { useApp, type PairingState } from "@/store/app";

/** Shows the active pairing (if any). Both users must confirm matching codes. */
export function PairingDialog() {
  const pairings = useApp((s) => s.pairings);
  const current = Object.values(pairings)[0];
  return <PairingView key={current?.pairingId ?? "none"} pairing={current} />;
}

function PairingView({ pairing }: { pairing: PairingState | undefined }) {
  const dismiss = useApp((s) => s.dismissPairing);
  const fail = useApp((s) => s.fail);
  const local = useApp((s) => s.local);
  const [sending, setSending] = useState(false);
  const open = !!pairing;
  const p = pairing;

  const respond = async (accept: boolean) => {
    if (!p) return;
    setSending(true);
    try {
      await api.respondPairing(p.pairingId, accept);
      if (accept) {
        useApp.setState((s) => ({ pairings: { ...s.pairings, [p.pairingId]: { ...p, phase: "confirmed" } } }));
      } else {
        dismiss(p.pairingId);
      }
    } catch (e) {
      fail(e, "Pairing failed");
      dismiss(p.pairingId);
    } finally {
      setSending(false);
    }
  };

  const done = p?.phase === "success" || p?.phase === "failed";

  return (
    <Dialog open={open} width={420} dismissable={done} onClose={() => p && dismiss(p.pairingId)}>
      {p && (
        <div className="flex flex-col items-center pt-2 text-center">
          <div className="flex items-center gap-3">
            <DeviceAvatar os={local?.os ?? "unknown"} size={44} />
            <motion.div
              className="h-px w-10 brand-gradient"
              animate={{ opacity: p.phase === "success" ? 1 : [0.3, 1, 0.3] }}
              transition={{ duration: 1.6, repeat: p.phase === "success" ? 0 : Infinity }}
            />
            <DeviceAvatar os={p.deviceOs} size={44} />
          </div>
          <h2 className="mt-4 text-[15px] font-semibold text-fg">
            {p.role === "responder" ? `${p.deviceName} wants to pair` : `Pairing with ${p.deviceName}`}
          </h2>
          <p className="mt-1 text-xs text-fg-muted">{osLabel(p.deviceOs)}</p>

          <AnimatePresence mode="wait">
            {p.phase === "connecting" && (
              <motion.div key="c" {...fade} className="my-8 flex items-center gap-2 text-[13px] text-fg-muted">
                <Spinner /> Establishing a secure connection…
              </motion.div>
            )}
            {(p.phase === "code" || p.phase === "confirmed") && p.code && (
              <motion.div key="code" {...fade} className="w-full">
                <p className="mx-auto mt-5 max-w-[300px] text-[12.5px] leading-relaxed text-fg-muted">
                  Check that <span className="text-fg">{p.deviceName}</span> shows the same code. If the codes differ,
                  someone may be intercepting the connection — do not confirm.
                </p>
                <div className="my-5 flex justify-center gap-1.5">
                  {p.code.split("").map((ch, i) => (
                    <motion.span
                      key={i}
                      initial={{ opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: i * 0.04 }}
                      className={`selectable flex h-12 w-10 items-center justify-center rounded-lg border border-line-strong bg-surface-2 font-mono text-[22px] font-semibold text-fg ${i === 2 ? "mr-2" : ""}`}
                    >
                      {ch}
                    </motion.span>
                  ))}
                </div>
                {p.phase === "code" ? (
                  <div className="flex gap-2">
                    <Button variant="secondary" className="flex-1" onClick={() => respond(false)} disabled={sending}>
                      Codes don’t match
                    </Button>
                    <Button variant="primary" className="flex-1" icon={ShieldCheck} onClick={() => respond(true)} loading={sending}>
                      Codes match
                    </Button>
                  </div>
                ) : (
                  <div className="flex items-center justify-center gap-2 py-2 text-[13px] text-fg-muted">
                    <Spinner /> Waiting for {p.deviceName} to confirm…
                  </div>
                )}
              </motion.div>
            )}
            {p.phase === "success" && (
              <motion.div key="ok" {...fade} className="my-6 flex flex-col items-center">
                <CheckCircle2 size={36} className="text-success" />
                <p className="mt-3 text-[13px] text-fg">{p.message}</p>
                <p className="mt-1 text-xs text-fg-muted">You can now send files, text and clipboard content.</p>
                <Button variant="primary" className="mt-5" onClick={() => dismiss(p.pairingId)}>
                  Done
                </Button>
              </motion.div>
            )}
            {p.phase === "failed" && (
              <motion.div key="fail" {...fade} className="my-6 flex flex-col items-center">
                {p.message?.includes("declined") ? (
                  <XCircle size={36} className="text-fg-subtle" />
                ) : (
                  <ShieldAlert size={36} className="text-danger" />
                )}
                <p className="mt-3 text-[13px] text-fg">Pairing did not complete</p>
                <p className="mt-1 max-w-[300px] text-xs text-fg-muted">{p.message}</p>
                <Button variant="secondary" className="mt-5" onClick={() => dismiss(p.pairingId)}>
                  Close
                </Button>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      )}
    </Dialog>
  );
}

const fade = {
  initial: { opacity: 0, y: 4 },
  animate: { opacity: 1, y: 0 },
  exit: { opacity: 0, y: -4 },
  transition: { duration: 0.18 },
};
