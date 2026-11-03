import { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import { X } from "lucide-react";
import { cn } from "@/lib/cn";
import { IconButton } from "./Button";

interface Props {
  open: boolean;
  onClose?: () => void;
  title?: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** Dialogs that require a decision cannot be dismissed by clicking outside. */
  dismissable?: boolean;
  icon?: ReactNode;
}

export function Dialog({ open, onClose, title, description, children, footer, width = 440, dismissable = true, icon }: Props) {
  useEffect(() => {
    if (!open || !dismissable) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose?.();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, dismissable, onClose]);

  return createPortal(
    <AnimatePresence>
      {open && (
        <motion.div
          className="fixed inset-0 z-50 flex items-center justify-center p-6"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.16 }}
        >
          <div
            className="absolute inset-0 bg-overlay backdrop-blur-[3px]"
            onClick={() => dismissable && onClose?.()}
          />
          <motion.div
            role="dialog"
            aria-modal
            initial={{ opacity: 0, scale: 0.965, y: 8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.98, y: 4 }}
            transition={{ type: "spring", stiffness: 420, damping: 32 }}
            style={{ width }}
            className={cn("relative max-h-[86vh] max-w-full overflow-hidden rounded-xl bg-elevated shadow-pop", "flex flex-col")}
          >
            {(title || onClose) && (
              <div className="flex items-start gap-3 px-5 pt-5 pb-1">
                {icon}
                <div className="min-w-0 flex-1">
                  {title && <h2 className="text-[15px] font-semibold tracking-tight text-fg">{title}</h2>}
                  {description && <p className="mt-1 text-[12.5px] leading-relaxed text-fg-muted">{description}</p>}
                </div>
                {onClose && dismissable && <IconButton icon={X} label="Close" size="sm" onClick={onClose} className="-mt-1 -mr-1" />}
              </div>
            )}
            {children != null ? (
              <div className="min-h-0 flex-1 overflow-y-auto px-5 py-4">{children}</div>
            ) : (
              <div className="h-3" />
            )}
            {footer && <div className="flex items-center justify-end gap-2 border-t border-line bg-surface/40 px-5 py-3">{footer}</div>}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>,
    document.body,
  );
}
