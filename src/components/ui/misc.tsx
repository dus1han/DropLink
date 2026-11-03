import type { ReactNode } from "react";
import { motion } from "framer-motion";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";

export function ProgressBar({
  value,
  tone = "accent",
  indeterminate,
  className,
}: {
  value: number;
  tone?: "accent" | "success" | "warning" | "danger" | "neutral";
  indeterminate?: boolean;
  className?: string;
}) {
  const color = {
    accent: "brand-gradient",
    success: "bg-success",
    warning: "bg-warning",
    danger: "bg-danger",
    neutral: "bg-fg-subtle",
  }[tone];
  return (
    <div className={cn("relative h-1.5 w-full overflow-hidden rounded-full bg-surface-3", className)}>
      {indeterminate ? (
        <motion.div
          className={cn("absolute inset-y-0 w-1/3 rounded-full opacity-70", color)}
          animate={{ x: ["-100%", "300%"] }}
          transition={{ duration: 1.4, repeat: Infinity, ease: "easeInOut" }}
        />
      ) : (
        <motion.div
          className={cn("absolute inset-y-0 left-0 rounded-full", color)}
          initial={false}
          animate={{ width: `${Math.max(0, Math.min(1, value)) * 100}%` }}
          transition={{ type: "spring", stiffness: 120, damping: 24 }}
        />
      )}
    </div>
  );
}

export function Segmented<T extends string>({
  value,
  onChange,
  options,
  size = "md",
}: {
  value: T;
  onChange: (v: T) => void;
  options: { value: T; label: string; icon?: LucideIcon; count?: number }[];
  size?: "sm" | "md";
}) {
  return (
    <div className="inline-flex rounded-lg border border-line bg-surface-2 p-0.5">
      {options.map((o) => {
        const active = o.value === value;
        return (
          <button
            key={o.value}
            onClick={() => onChange(o.value)}
            className={cn(
              "relative inline-flex items-center gap-1.5 rounded-md font-medium transition-colors duration-150",
              size === "sm" ? "h-6 px-2 text-xs" : "h-7 px-3 text-[12.5px]",
              active ? "text-fg" : "text-fg-muted hover:text-fg",
            )}
          >
            {active && (
              <motion.span
                layoutId={`seg-${options.map((x) => x.value).join("")}`}
                className="absolute inset-0 rounded-md bg-surface-3 shadow-soft"
                transition={{ type: "spring", stiffness: 500, damping: 38 }}
              />
            )}
            <span className="relative flex items-center gap-1.5">
              {o.icon && <o.icon size={13} />}
              {o.label}
              {o.count != null && o.count > 0 && <span className="tabular text-fg-subtle">{o.count}</span>}
            </span>
          </button>
        );
      })}
    </div>
  );
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  compact,
}: {
  icon: LucideIcon;
  title: string;
  description?: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      className={cn("flex flex-col items-center justify-center text-center", compact ? "py-8" : "py-16")}
    >
      <div className="relative mb-4">
        <div className="absolute inset-0 rounded-2xl brand-gradient opacity-20 blur-xl" />
        <div className="relative flex h-12 w-12 items-center justify-center rounded-xl border border-line-strong bg-surface-2 surface-highlight">
          <Icon size={20} className="text-fg-muted" strokeWidth={1.7} />
        </div>
      </div>
      <div className="text-[13.5px] font-semibold text-fg">{title}</div>
      {description && <div className="mt-1 max-w-sm text-[12.5px] leading-relaxed text-fg-muted">{description}</div>}
      {action && <div className="mt-4">{action}</div>}
    </motion.div>
  );
}

export function Panel({
  title,
  subtitle,
  actions,
  children,
  className,
  padded = true,
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
  padded?: boolean;
}) {
  return (
    <section className={cn("rounded-xl border border-line bg-surface surface-highlight", className)}>
      {(title || actions) && (
        <header className="flex items-center gap-3 border-b border-line px-4 py-3">
          <div className="min-w-0 flex-1">
            {title && <h3 className="text-[13px] font-semibold text-fg">{title}</h3>}
            {subtitle && <p className="mt-0.5 text-xs text-fg-subtle">{subtitle}</p>}
          </div>
          {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
        </header>
      )}
      <div className={padded ? "p-4" : undefined}>{children}</div>
    </section>
  );
}

export function SectionTitle({ children, actions, count }: { children: ReactNode; actions?: ReactNode; count?: number }) {
  return (
    <div className="mb-3 flex items-center gap-2">
      <h2 className="text-xs font-semibold tracking-wide text-fg-muted uppercase">{children}</h2>
      {count != null && <span className="tabular text-xs text-fg-subtle">{count}</span>}
      <div className="flex-1" />
      {actions}
    </div>
  );
}

/** A settings-style row: label + description on the left, control on the right. */
export function Row({
  title,
  description,
  children,
  icon: Icon,
}: {
  title: ReactNode;
  description?: ReactNode;
  children?: ReactNode;
  icon?: LucideIcon;
}) {
  return (
    <div className="flex items-center gap-4 px-4 py-3.5 [&+&]:border-t [&+&]:border-line">
      {Icon && (
        <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg bg-surface-2 text-fg-muted">
          <Icon size={15} />
        </div>
      )}
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-fg">{title}</div>
        {description && <div className="mt-0.5 text-xs leading-relaxed text-fg-muted">{description}</div>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}
