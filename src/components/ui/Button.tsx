import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/cn";
import { Spinner } from "./Spinner";
import { Tooltip } from "./Tooltip";

type Variant = "primary" | "secondary" | "ghost" | "danger" | "subtle";
type Size = "sm" | "md" | "lg";

const VARIANTS: Record<Variant, string> = {
  primary:
    "bg-accent text-accent-fg hover:bg-accent-strong shadow-[inset_0_1px_0_rgba(255,255,255,0.18),0_1px_2px_rgba(0,0,0,0.2)]",
  secondary: "bg-surface-2 text-fg border border-line-strong hover:bg-surface-3 surface-highlight",
  ghost: "text-fg-muted hover:text-fg hover:bg-surface-2",
  subtle: "bg-accent-soft text-accent hover:brightness-110",
  danger: "bg-danger-soft text-danger hover:bg-danger hover:text-white",
};

const SIZES: Record<Size, string> = {
  sm: "h-7 px-2.5 text-xs gap-1.5 rounded-md",
  md: "h-8 px-3 text-[13px] gap-2 rounded-md",
  lg: "h-10 px-4 text-sm gap-2 rounded-lg",
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: LucideIcon;
  iconRight?: LucideIcon;
  loading?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "secondary", size = "md", icon: Icon, iconRight: IconRight, loading, className, children, disabled, ...rest },
  ref,
) {
  const iconSize = size === "sm" ? 13 : size === "lg" ? 16 : 14;
  return (
    <button
      ref={ref}
      disabled={disabled || loading}
      className={cn(
        "inline-flex shrink-0 select-none items-center justify-center font-medium whitespace-nowrap",
        "transition-[background-color,color,filter,box-shadow,transform] duration-150 active:scale-[0.98]",
        "disabled:pointer-events-none disabled:opacity-45",
        VARIANTS[variant],
        SIZES[size],
        className,
      )}
      {...rest}
    >
      {loading ? <Spinner size={iconSize} /> : Icon ? <Icon size={iconSize} strokeWidth={2} /> : null}
      {children}
      {IconRight && !loading && <IconRight size={iconSize} strokeWidth={2} className="opacity-70" />}
    </button>
  );
});

export interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon: LucideIcon;
  label: string;
  size?: "sm" | "md";
  tone?: "default" | "danger" | "accent";
  active?: boolean;
  badge?: ReactNode;
}

export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { icon: Icon, label, size = "md", tone = "default", active, badge, className, ...rest },
  ref,
) {
  return (
    <Tooltip label={label}>
      <button
        ref={ref}
        aria-label={label}
        className={cn(
          "relative inline-flex shrink-0 items-center justify-center rounded-md transition-colors duration-150",
          size === "sm" ? "h-7 w-7" : "h-8 w-8",
          tone === "danger"
            ? "text-fg-muted hover:bg-danger-soft hover:text-danger"
            : tone === "accent"
              ? "text-accent hover:bg-accent-soft"
              : "text-fg-muted hover:bg-surface-2 hover:text-fg",
          active && "bg-surface-2 text-fg",
          "disabled:pointer-events-none disabled:opacity-40",
          className,
        )}
        {...rest}
      >
        <Icon size={size === "sm" ? 14 : 16} strokeWidth={1.9} />
        {badge}
      </button>
    </Tooltip>
  );
});
