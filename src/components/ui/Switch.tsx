import { motion } from "framer-motion";
import { cn } from "@/lib/cn";

interface Props {
  checked: boolean;
  onChange: (v: boolean) => void;
  disabled?: boolean;
  label?: string;
  size?: "sm" | "md";
}

export function Switch({ checked, onChange, disabled, label, size = "md" }: Props) {
  const w = size === "sm" ? 26 : 32;
  const h = size === "sm" ? 16 : 18;
  const knob = h - 4;
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      style={{ width: w, height: h }}
      className={cn(
        "relative shrink-0 rounded-full transition-colors duration-200",
        checked ? "bg-accent" : "bg-surface-3 shadow-[inset_0_0_0_1px_var(--line-strong)]",
        "disabled:opacity-40",
      )}
    >
      <motion.span
        className="absolute top-[2px] left-[2px] rounded-full bg-white shadow-[0_1px_2px_rgba(0,0,0,0.3)]"
        style={{ width: knob, height: knob }}
        animate={{ x: checked ? w - knob - 4 : 0 }}
        transition={{ type: "spring", stiffness: 600, damping: 34 }}
      />
    </button>
  );
}
