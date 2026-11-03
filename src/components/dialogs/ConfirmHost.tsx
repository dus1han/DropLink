import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { useConfirm } from "@/store/confirm";

export function ConfirmHost() {
  const request = useConfirm((s) => s.request);
  const settle = useConfirm((s) => s.settle);
  return (
    <Dialog
      open={!!request}
      onClose={() => settle(false)}
      title={request?.title}
      description={request?.body}
      width={400}
      footer={
        <>
          <Button variant="ghost" onClick={() => settle(false)}>
            Cancel
          </Button>
          <Button autoFocus variant={request?.tone === "danger" ? "danger" : "primary"} onClick={() => settle(true)}>
            {request?.confirmLabel ?? "Confirm"}
          </Button>
        </>
      }
    >
      {null}
    </Dialog>
  );
}
