import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";

export interface MenuItem {
  label: string;
  icon?: LucideIcon;
  onSelect: () => void;
  tone?: "default" | "danger";
  disabled?: boolean;
  hidden?: boolean;
}

interface Props {
  trigger: (props: { onClick: (e: React.MouseEvent) => void; "aria-expanded": boolean }) => ReactNode;
  items: (MenuItem | "separator")[];
  align?: "start" | "end";
  width?: number;
}

export function Menu({ trigger, items, align = "end", width = 200 }: Props) {
  const [anchor, setAnchor] = useState<DOMRect | null>(null);
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number }>({ left: 0, top: 0 });

  useLayoutEffect(() => {
    if (!anchor) return;
    const h = ref.current?.offsetHeight ?? 0;
    let left = align === "end" ? anchor.right - width : anchor.left;
    left = Math.max(8, Math.min(left, window.innerWidth - width - 8));
    let top = anchor.bottom + 6;
    if (top + h > window.innerHeight - 8) top = anchor.top - h - 6;
    setPos({ left, top });
  }, [anchor, align, width]);

  useEffect(() => {
    if (!anchor) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setAnchor(null);
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setAnchor(null);
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKey);
    window.addEventListener("blur", () => setAnchor(null), { once: true });
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKey);
    };
  }, [anchor]);

  const visible = items.filter((i) => i === "separator" || !i.hidden);

  return (
    <>
      {trigger({
        onClick: (e) => {
          e.stopPropagation();
          setAnchor(anchor ? null : (e.currentTarget as HTMLElement).getBoundingClientRect());
        },
        "aria-expanded": !!anchor,
      })}
      {createPortal(
        <AnimatePresence>
          {anchor && (
            <motion.div
              ref={ref}
              role="menu"
              initial={{ opacity: 0, y: -4, scale: 0.98 }}
              animate={{ opacity: 1, y: 0, scale: 1 }}
              exit={{ opacity: 0, transition: { duration: 0.08 } }}
              transition={{ duration: 0.14, ease: [0.22, 1, 0.36, 1] }}
              style={{ left: pos.left, top: pos.top, width }}
              className="fixed z-[90] rounded-lg bg-elevated p-1 shadow-pop"
              onClick={(e) => e.stopPropagation()}
            >
              {visible.map((item, i) =>
                item === "separator" ? (
                  <div key={`s${i}`} className="mx-1 my-1 h-px bg-line" />
                ) : (
                  <button
                    key={item.label}
                    role="menuitem"
                    disabled={item.disabled}
                    onClick={() => {
                      setAnchor(null);
                      item.onSelect();
                    }}
                    className={cn(
                      "flex h-8 w-full items-center gap-2.5 rounded-md px-2 text-left text-[13px] transition-colors",
                      item.tone === "danger" ? "text-danger hover:bg-danger-soft" : "text-fg hover:bg-surface-3",
                      "disabled:pointer-events-none disabled:opacity-40",
                    )}
                  >
                    {item.icon && <item.icon size={14} className={item.tone === "danger" ? "" : "text-fg-muted"} />}
                    {item.label}
                  </button>
                ),
              )}
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}
    </>
  );
}
