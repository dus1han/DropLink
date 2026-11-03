import { Laptop, Monitor, MonitorSmartphone, Server } from "lucide-react";
import { cn } from "@/lib/cn";
import type { Os } from "@/lib/types";

const STYLE: Record<string, { icon: typeof Laptop; tint: string }> = {
  macos: { icon: Laptop, tint: "from-[#8b8cff]/25 to-[#c3c6d4]/10 text-[#b9b8ff]" },
  windows: { icon: Monitor, tint: "from-[#2bb8f0]/25 to-[#3b82f6]/10 text-[#6fcbf5]" },
  linux: { icon: Server, tint: "from-[#f2b64a]/25 to-[#f2585d]/10 text-[#f4c36a]" },
};

export function DeviceAvatar({
  os,
  size = 36,
  online,
  className,
}: {
  os: Os;
  size?: number;
  online?: boolean;
  className?: string;
}) {
  const s = STYLE[os] ?? { icon: MonitorSmartphone, tint: "from-fg-subtle/20 to-fg-subtle/5 text-fg-muted" };
  const Icon = s.icon;
  return (
    <div className={cn("relative shrink-0", className)} style={{ width: size, height: size }}>
      <div
        className={cn(
          "flex h-full w-full items-center justify-center rounded-[28%] border border-line-strong bg-gradient-to-br surface-highlight",
          s.tint,
        )}
      >
        <Icon size={Math.round(size * 0.46)} strokeWidth={1.7} />
      </div>
      {online != null && (
        <span
          className={cn(
            "absolute -right-0.5 -bottom-0.5 h-2.5 w-2.5 rounded-full border-2 border-surface",
            online ? "bg-success" : "bg-fg-subtle",
          )}
        />
      )}
    </div>
  );
}
