import { useEffect, useMemo, useState } from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Boxes, MoreHorizontal, Pencil, Plus, Send, Trash2, Type } from "lucide-react";
import { DeviceAvatar } from "@/components/DeviceAvatar";
import { groupDot } from "@/components/RecipientPicker";
import { Button, IconButton } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Checkbox, Input, Label } from "@/components/ui/Field";
import { Menu } from "@/components/ui/Menu";
import { EmptyState } from "@/components/ui/misc";
import { Switch } from "@/components/ui/Switch";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { osLabel, pluralize } from "@/lib/format";
import type { Group } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { sortDevices, useApp } from "@/store/app";

const COLORS = ["blue", "violet", "green", "amber", "rose", "slate"];
const SUGGESTIONS = ["Personal Devices", "Office Computers", "Development Machines", "Testing Devices"];

export function GroupsPage() {
  const groups = useApp((s) => s.groups);
  const [editing, setEditing] = useState<Group | "new" | null>(null);
  const [preset, setPreset] = useState("");

  return (
    <div>
      <div className="mb-5 flex items-center gap-3">
        <p className="flex-1 text-[12.5px] text-fg-muted">
          Groups let you send to several devices at once. Permissions set on a group apply to every trusted member.
        </p>
        <Button variant="primary" icon={Plus} onClick={() => (setPreset(""), setEditing("new"))}>
          New group
        </Button>
      </div>

      {groups.length === 0 ? (
        <div className="rounded-xl border border-dashed border-line-strong">
          <EmptyState
            icon={Boxes}
            title="No groups yet"
            description="Organise devices by where and how you use them."
            action={
              <div className="flex flex-wrap justify-center gap-1.5">
                {SUGGESTIONS.map((s) => (
                  <Button
                    key={s}
                    size="sm"
                    variant="secondary"
                    icon={Plus}
                    onClick={() => {
                      setPreset(s);
                      setEditing("new");
                    }}
                  >
                    {s}
                  </Button>
                ))}
              </div>
            }
          />
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-3 md:grid-cols-2 xl:grid-cols-3">
          <AnimatePresence>
            {groups.map((g) => (
              <GroupCard key={g.id} group={g} onEdit={() => setEditing(g)} />
            ))}
          </AnimatePresence>
        </div>
      )}
      <GroupDialog group={editing} preset={preset} onClose={() => setEditing(null)} />
    </div>
  );
}

function GroupCard({ group, onEdit }: { group: Group; onEdit: () => void }) {
  const devices = useApp((s) => s.devices);
  const navigate = useApp((s) => s.navigate);
  const fail = useApp((s) => s.fail);
  const members = group.memberIds.map((id) => devices[id]).filter(Boolean);
  const trusted = members.filter((d) => d.trust === "trusted");
  const online = trusted.filter((d) => d.online);

  const update = (patch: Parameters<typeof api.updateGroup>[1]) => api.updateGroup(group.id, patch).catch(fail);

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.98 }}
      className="flex flex-col rounded-xl border border-line bg-surface surface-highlight"
    >
      <div className="flex items-start gap-3 p-4">
        <span className={cn("mt-1.5 h-2.5 w-2.5 shrink-0 rounded-full", groupDot(group.color))} />
        <div className="min-w-0 flex-1">
          <div className="truncate text-[14px] font-semibold text-fg">{group.name}</div>
          <div className="text-xs text-fg-muted">
            {pluralize(members.length, "device")} · {online.length} online
          </div>
        </div>
        <Menu
          items={[
            { label: "Edit group", icon: Pencil, onSelect: onEdit },
            { label: "Send text to group", icon: Type, onSelect: () => navigate("text", { recipients: trusted.map((d) => d.id) }), disabled: !trusted.length },
            "separator",
            {
              label: "Delete group",
              icon: Trash2,
              tone: "danger",
              onSelect: async () => {
                if (await confirm({ title: `Delete “${group.name}”?`, body: "The devices themselves are not affected.", confirmLabel: "Delete", tone: "danger" })) {
                  api.deleteGroup(group.id).catch(fail);
                }
              },
            },
          ]}
          trigger={(p) => <IconButton icon={MoreHorizontal} label="Group actions" size="sm" className="-mt-1 -mr-1" {...p} />}
        />
      </div>

      <div className="flex min-h-[34px] items-center px-4">
        {members.length === 0 ? (
          <button onClick={onEdit} className="text-xs text-fg-subtle hover:text-fg-muted">
            No devices — add some
          </button>
        ) : (
          <div className="flex -space-x-2">
            {members.slice(0, 6).map((d) => (
              <div key={d.id} title={d.name} className="rounded-[30%] ring-2 ring-surface">
                <DeviceAvatar os={d.os} size={28} online={d.online} />
              </div>
            ))}
            {members.length > 6 && (
              <div className="flex h-7 w-7 items-center justify-center rounded-[30%] bg-surface-3 text-2xs text-fg-muted ring-2 ring-surface">
                +{members.length - 6}
              </div>
            )}
          </div>
        )}
      </div>

      <div className="mt-3 border-t border-line px-4 py-2.5">
        <label className="flex items-center justify-between py-1 text-xs text-fg-muted">
          Accept files without asking
          <Switch size="sm" checked={group.autoAccept} onChange={(v) => update({ autoAccept: v })} />
        </label>
        <label className="flex items-center justify-between py-1 text-xs text-fg-muted">
          Share clipboard
          <Switch size="sm" checked={group.clipboardSync} onChange={(v) => update({ clipboardSync: v })} />
        </label>
      </div>
      <div className="mt-auto border-t border-line p-3">
        <Button
          size="sm"
          icon={Send}
          className="w-full"
          disabled={!trusted.length}
          onClick={() => navigate("send", { recipients: trusted.map((d) => d.id), groupId: group.id })}
        >
          Send to group
        </Button>
      </div>
    </motion.div>
  );
}

function GroupDialog({ group, preset, onClose }: { group: Group | "new" | null; preset: string; onClose: () => void }) {
  const devicesMap = useApp((s) => s.devices);
  const fail = useApp((s) => s.fail);
  const isNew = group === "new";
  const [name, setName] = useState("");
  const [color, setColor] = useState("blue");
  const [members, setMembers] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (group === "new") {
      setName(preset);
      setColor(COLORS[Math.floor(Math.random() * COLORS.length)]);
      setMembers([]);
    } else if (group) {
      setName(group.name);
      setColor(group.color);
      setMembers(group.memberIds);
    }
  }, [group, preset]);

  const candidates = useMemo(() => sortDevices(Object.values(devicesMap).filter((d) => d.trust !== "blocked")), [devicesMap]);

  const saveGroup = async () => {
    setSaving(true);
    try {
      if (isNew) await api.createGroup(name, color, members);
      else if (group) await api.updateGroup(group.id, { name, color, members });
      onClose();
    } catch (e) {
      fail(e, "Could not save group");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={!!group}
      onClose={onClose}
      title={isNew ? "New group" : "Edit group"}
      width={460}
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={saveGroup} loading={saving} disabled={!name.trim()}>
            {isNew ? "Create group" : "Save"}
          </Button>
        </>
      }
    >
      <Label>Name</Label>
      <Input autoFocus value={name} onChange={(e) => setName(e.target.value)} placeholder="e.g. Office Computers" maxLength={48} />
      <div className="mt-4">
        <Label>Colour</Label>
        <div className="flex gap-2">
          {COLORS.map((c) => (
            <button
              key={c}
              aria-label={c}
              onClick={() => setColor(c)}
              className={cn(
                "h-6 w-6 rounded-full transition-transform",
                groupDot(c),
                color === c ? "scale-110 ring-2 ring-fg/60 ring-offset-2 ring-offset-elevated" : "opacity-70 hover:opacity-100",
              )}
            />
          ))}
        </div>
      </div>
      <div className="mt-4">
        <Label hint="Untrusted members are kept in the group but never receive anything until paired.">Devices</Label>
        {candidates.length === 0 ? (
          <div className="rounded-lg border border-line px-3 py-4 text-center text-xs text-fg-subtle">No known devices yet.</div>
        ) : (
          <div className="max-h-60 overflow-y-auto rounded-lg border border-line">
            {candidates.map((d) => {
              const on = members.includes(d.id);
              const toggle = () => setMembers(on ? members.filter((m) => m !== d.id) : [...members, d.id]);
              return (
                <div key={d.id} role="button" onClick={toggle} className="flex items-center gap-3 px-3 py-2 hover:bg-surface-2 [&+&]:border-t [&+&]:border-line">
                  <Checkbox checked={on} onChange={toggle} />
                  <DeviceAvatar os={d.os} size={26} online={d.online} />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] text-fg">{d.name}</div>
                    <div className="text-2xs text-fg-subtle">
                      {osLabel(d.os)} · {d.trust === "trusted" ? "Trusted" : "Not paired"}
                    </div>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </Dialog>
  );
}
