import { cloneElement, useRef, useState, type ReactElement } from "react";
import { createPortal } from "react-dom";
import { AnimatePresence, motion } from "framer-motion";

interface Props {
  label: string;
  children: ReactElement<Record<string, unknown>>;
  side?: "top" | "bottom" | "right";
}

/** Small delayed tooltip rendered in a portal so it is never clipped. */
export function Tooltip({ label, children, side = "bottom" }: Props) {
  const [pos, setPos] = useState<{ x: number; y: number } | null>(null);
  const timer = useRef<number | undefined>(undefined);

  const show = (el: HTMLElement) => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      const r = el.getBoundingClientRect();
      if (side === "right") setPos({ x: r.right + 8, y: r.top + r.height / 2 });
      else if (side === "top") setPos({ x: r.left + r.width / 2, y: r.top - 8 });
      else setPos({ x: r.left + r.width / 2, y: r.bottom + 8 });
    }, 450);
  };
  const hide = () => {
    window.clearTimeout(timer.current);
    setPos(null);
  };

  const child = cloneElement(children, {
    onMouseEnter: (e: React.MouseEvent<HTMLElement>) => show(e.currentTarget),
    onMouseLeave: hide,
    onMouseDown: hide,
  });

  const transform =
    side === "right" ? "translate(0, -50%)" : side === "top" ? "translate(-50%, -100%)" : "translate(-50%, 0)";

  return (
    <>
      {child}
      {createPortal(
        <AnimatePresence>
          {pos && (
            <motion.div
              initial={{ opacity: 0, scale: 0.96 }}
              animate={{ opacity: 1, scale: 1 }}
              exit={{ opacity: 0 }}
              transition={{ duration: 0.12 }}
              style={{ left: pos.x, top: pos.y, transform }}
              className="pointer-events-none fixed z-[100] rounded-md border border-line-strong bg-elevated px-2 py-1 text-2xs font-medium whitespace-nowrap text-fg shadow-pop"
            >
              {label}
            </motion.div>
          )}
        </AnimatePresence>,
        document.body,
      )}
    </>
  );
}
