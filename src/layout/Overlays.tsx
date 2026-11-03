import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { AnimatePresence, motion } from "framer-motion";
import { CircleAlert, CircleCheck, Info, UploadCloud, X } from "lucide-react";
import { cn } from "@/lib/cn";
import { useApp } from "@/store/app";

/** Full-window drop target: dropping files anywhere stages them for sending. */
export function DropOverlay() {
  const [hovering, setHovering] = useState(false);
  const stagePaths = useApp((s) => s.stagePaths);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") setHovering(true);
        else if (p.type === "leave") setHovering(false);
        else if (p.type === "drop") {
          setHovering(false);
          if (p.paths.length) stagePaths(p.paths, true);
        }
      })
      .then((u) => (unlisten = u));
    return () => unlisten?.();
  }, [stagePaths]);

  return (
    <AnimatePresence>
      {hovering && (
        <motion.div
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.15 }}
          className="pointer-events-none fixed inset-0 z-[80] flex items-center justify-center bg-overlay p-6 backdrop-blur-sm"
        >
          <motion.div
            initial={{ scale: 0.96 }}
            animate={{ scale: 1 }}
            className="flex h-full w-full flex-col items-center justify-center rounded-2xl border-2 border-dashed border-accent/60 bg-accent-soft/40"
          >
            <motion.div animate={{ y: [0, -6, 0] }} transition={{ duration: 1.6, repeat: Infinity, ease: "easeInOut" }}>
              <UploadCloud size={44} className="text-accent" strokeWidth={1.5} />
            </motion.div>
            <div className="mt-4 text-[17px] font-semibold text-fg">Drop to send</div>
            <div className="mt-1 text-[13px] text-fg-muted">Files and folders will be added to a new transfer</div>
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}

export function Toaster() {
  const toasts = useApp((s) => s.toasts);
  const dismiss = useApp((s) => s.dismissToast);
  return (
    <div className="pointer-events-none fixed right-5 bottom-5 z-[95] flex w-[340px] flex-col gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => {
          const Icon = t.tone === "success" ? CircleCheck : t.tone === "error" ? CircleAlert : Info;
          return (
            <motion.div
              key={t.id}
              layout
              initial={{ opacity: 0, y: 12, scale: 0.97 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, x: 24, transition: { duration: 0.15 } }}
              transition={{ type: "spring", stiffness: 420, damping: 32 }}
              className="pointer-events-auto flex gap-3 rounded-xl bg-elevated p-3.5 shadow-pop"
            >
              <Icon
                size={16}
                className={cn(
                  "mt-px shrink-0",
                  t.tone === "success" ? "text-success" : t.tone === "error" ? "text-danger" : "text-info",
                )}
              />
              <div className="min-w-0 flex-1">
                <div className="text-[12.5px] font-medium text-fg">{t.title}</div>
                {t.body && <div className="selectable mt-0.5 text-xs leading-relaxed break-words text-fg-muted">{t.body}</div>}
              </div>
              <button onClick={() => dismiss(t.id)} className="-m-1 h-6 w-6 shrink-0 rounded-md p-1 text-fg-subtle hover:bg-surface-3 hover:text-fg">
                <X size={14} />
              </button>
            </motion.div>
          );
        })}
      </AnimatePresence>
    </div>
  );
}
