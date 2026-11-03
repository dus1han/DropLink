import { useEffect, useState } from "react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input, Label } from "@/components/ui/Field";
import { api } from "@/lib/api";
import type { Device } from "@/lib/types";
import { useApp } from "@/store/app";

export function RenameDialog({ device, open, onClose }: { device: Device; open: boolean; onClose: () => void }) {
  const [value, setValue] = useState(device.alias ?? "");
  const [saving, setSaving] = useState(false);
  useEffect(() => {
    if (open) setValue(device.alias ?? "");
  }, [open, device.alias]);

  const save = async (alias: string | null) => {
    setSaving(true);
    try {
      await api.renameDevice(device.id, alias);
      onClose();
    } catch (e) {
      useApp.getState().fail(e, "Could not rename device");
    } finally {
      setSaving(false);
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Rename device"
      description={`Only changes how “${device.reportedName}” appears on this computer.`}
      footer={
        <>
          {device.alias && (
            <Button variant="ghost" onClick={() => save(null)} disabled={saving} className="mr-auto">
              Use original name
            </Button>
          )}
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" loading={saving} onClick={() => save(value.trim() || null)}>
            Save
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void save(value.trim() || null);
        }}
      >
        <Label>Display name</Label>
        <Input autoFocus value={value} onChange={(e) => setValue(e.target.value)} placeholder={device.reportedName} maxLength={64} />
      </form>
    </Dialog>
  );
}
