import { useState } from "react";
import { Globe } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input, Label } from "@/components/ui/Field";
import { api, errorMessage } from "@/lib/api";
import { useApp } from "@/store/app";

export function ConnectDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [address, setAddress] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const { startPairing, toast } = useApp.getState();

  const connect = async () => {
    if (!address.trim()) return;
    setBusy(true);
    setError(null);
    try {
      const device = await api.connectManual(address.trim());
      onClose();
      setAddress("");
      if (device.trust === "unknown") {
        await startPairing(device);
      } else {
        toast({ tone: "success", title: `Connected to ${device.name}` });
      }
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open={open}
      onClose={onClose}
      title="Connect by address"
      description="Use this when a device is on another network (through a VPN or port forwarding) or when local discovery is blocked."
      icon={
        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-accent-soft text-accent">
          <Globe size={17} />
        </div>
      }
      footer={
        <>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" onClick={connect} loading={busy} disabled={!address.trim()}>
            Connect
          </Button>
        </>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void connect();
        }}
      >
        <Label hint="IP address or host name, optionally with a port. The default port is 47100.">Address</Label>
        <Input
          autoFocus
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          placeholder="192.168.1.24 or studio.local:47100"
          className="font-mono"
        />
        {error && <div className="mt-2 text-xs text-danger">{error}</div>}
        <p className="mt-3 text-2xs leading-relaxed text-fg-subtle">
          The connection is end-to-end encrypted with TLS 1.3. The device's identity is verified by pairing, never by its
          address.
        </p>
      </form>
    </Dialog>
  );
}
