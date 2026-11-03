import { useMemo } from "react";
import { Link2, Users } from "lucide-react";
import { DeviceAvatar } from "./DeviceAvatar";
import { Checkbox } from "./ui/Field";
import { EmptyState } from "./ui/misc";
import { Button } from "./ui/Button";
import { cn } from "@/lib/cn";
import { osLabel } from "@/lib/format";
import { sortDevices, useApp } from "@/store/app";

const GROUP_DOT: Record<string, string> = {
  blue: "bg-[#4fb6f2]",
  violet: "bg-[#8584ff]",
  green: "bg-[#3ccf8e]",
  amber: "bg-[#f2b64a]",
  rose: "bg-[#f2585d]",
  slate: "bg-[#9b9ea8]",
};
export const groupDot = (c: string) => GROUP_DOT[c] ?? GROUP_DOT.blue;

/** Multi-select list of trusted devices with "all online" and group shortcuts. */
export function RecipientPicker({
  value,
  onChange,
  onlyOnline = false,
}: {
  value: string[];
  onChange: (ids: string[]) => void;
  onlyOnline?: boolean;
}) {
  const devices = useApp((s) => s.devices);
  const groups = useApp((s) => s.groups);
  const navigate = useApp((s) => s.navigate);

  const trusted = useMemo(
    () => sortDevices(Object.values(devices).filter((d) => d.trust === "trusted" && (!onlyOnline || d.online))),
    [devices, onlyOnline],
  );
  const online = trusted.filter((d) => d.online);
  const selected = new Set(value);
  const toggle = (id: string) => onChange(selected.has(id) ? value.filter((v) => v !== id) : [...value, id]);
  const allOnlineSelected = online.length > 0 && online.every((d) => selected.has(d.id));

  if (trusted.length === 0) {
    return (
      <EmptyState
        compact
        icon={Link2}
        title="No trusted devices yet"
        description="Pair with a nearby device before sending. Only paired devices can receive from you."
        action={
          <Button size="sm" variant="subtle" onClick={() => navigate("devices")}>
            Find devices
          </Button>
        }
      />
    );
  }

  const groupsWithMembers = groups
    .map((g) => ({ ...g, members: g.memberIds.filter((id) => devices[id]?.trust === "trusted") }))
    .filter((g) => g.members.length > 0);

  return (
    <div>
      <div className="mb-2.5 flex flex-wrap gap-1.5">
        <Chip
          active={allOnlineSelected}
          disabled={online.length === 0}
          onClick={() =>
            onChange(allOnlineSelected ? value.filter((v) => !online.some((d) => d.id === v)) : Array.from(new Set([...value, ...online.map((d) => d.id)])))
          }
        >
          All online <span className="tabular text-fg-subtle">{online.length}</span>
        </Chip>
        {groupsWithMembers.map((g) => {
          const active = g.members.every((id) => selected.has(id));
          return (
            <Chip
              key={g.id}
              active={active}
              onClick={() =>
                onChange(active ? value.filter((v) => !g.members.includes(v)) : Array.from(new Set([...value, ...g.members])))
              }
            >
              <span className={cn("h-1.5 w-1.5 rounded-full", groupDot(g.color))} />
              {g.name}
              <span className="tabular text-fg-subtle">{g.members.length}</span>
            </Chip>
          );
        })}
        {groupsWithMembers.length === 0 && groups.length === 0 && (
          <button onClick={() => navigate("groups")} className="inline-flex h-6 items-center gap-1 px-1 text-2xs text-fg-subtle hover:text-fg-muted">
            <Users size={11} /> Create a group
          </button>
        )}
      </div>
      <div className="overflow-hidden rounded-lg border border-line">
        {trusted.map((d) => {
          const on = selected.has(d.id);
          return (
            <div
              key={d.id}
              role="button"
              onClick={() => toggle(d.id)}
              className={cn(
                "flex items-center gap-3 px-3 py-2.5 transition-colors [&+&]:border-t [&+&]:border-line",
                on ? "bg-accent-soft/60" : "hover:bg-surface-2",
              )}
            >
              <Checkbox checked={on} onChange={() => toggle(d.id)} label={d.name} />
              <DeviceAvatar os={d.os} size={28} online={d.online} />
              <div className="min-w-0 flex-1">
                <div className="truncate text-[13px] font-medium text-fg">{d.name}</div>
                <div className="text-2xs text-fg-subtle">
                  {osLabel(d.os)} · {d.online ? <span className="text-success">Online</span> : "Offline — sends when it reconnects"}
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function Chip({
  active,
  children,
  onClick,
  disabled,
}: {
  active: boolean;
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "inline-flex h-6 items-center gap-1.5 rounded-full border px-2.5 text-2xs font-medium transition-colors",
        active ? "border-accent/50 bg-accent-soft text-accent" : "border-line-strong text-fg-muted hover:bg-surface-2 hover:text-fg",
        "disabled:opacity-40",
      )}
    >
      {children}
    </button>
  );
}
