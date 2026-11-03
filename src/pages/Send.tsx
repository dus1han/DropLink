import { useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { FilePlus2, FolderPlus, Send, Trash2, UploadCloud, X } from "lucide-react";
import { FileIcon } from "@/components/FileIcon";
import { RecipientPicker } from "@/components/RecipientPicker";
import { TransferItem } from "@/components/TransferItem";
import { Button, IconButton } from "@/components/ui/Button";
import { Panel } from "@/components/ui/misc";
import { Spinner } from "@/components/ui/Spinner";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { baseName, formatBytes, pluralize } from "@/lib/format";
import type { PathInfo } from "@/lib/types";
import { useApp } from "@/store/app";

export function SendPage() {
  const staged = useApp((s) => s.staged);
  const stagePaths = useApp((s) => s.stagePaths);
  const unstage = useApp((s) => s.unstage);
  const clearStaged = useApp((s) => s.clearStaged);
  const preselect = useApp((s) => s.preselect);
  const groups = useApp((s) => s.groups);
  const deliveries = useApp((s) => s.deliveries);
  const { fail, toast } = useApp.getState();

  const [recipients, setRecipients] = useState<string[]>(preselect.recipients);
  const [groupId, setGroupId] = useState<string | undefined>(preselect.groupId);
  const [info, setInfo] = useState<Record<string, PathInfo>>({});
  const [sending, setSending] = useState(false);
  const [lastSession, setLastSession] = useState<string | null>(null);

  useEffect(() => {
    if (preselect.nonce === 0) return;
    const g = preselect.groupId ? groups.find((x) => x.id === preselect.groupId) : undefined;
    setRecipients(g ? g.memberIds : preselect.recipients);
    setGroupId(preselect.groupId);
  }, [preselect.nonce]);

  useEffect(() => {
    const missing = staged.filter((p) => !info[p]);
    if (!missing.length) return;
    api
      .inspectPaths(missing)
      .then((list) => setInfo((prev) => ({ ...prev, ...Object.fromEntries(list.map((i) => [i.path, i])) })))
      .catch(() => undefined);
  }, [staged, info]);

  const totals = useMemo(() => {
    let size = 0;
    let files = 0;
    let pending = false;
    for (const p of staged) {
      const i = info[p];
      if (!i) pending = true;
      else {
        size += i.size;
        files += i.fileCount;
      }
    }
    return { size, files, pending };
  }, [staged, info]);

  const pickFiles = async () => {
    const r = await open({ multiple: true, title: "Choose files to send" });
    if (Array.isArray(r)) stagePaths(r);
    else if (typeof r === "string") stagePaths([r]);
  };
  const pickFolder = async () => {
    const r = await open({ directory: true, multiple: true, title: "Choose folders to send" });
    if (Array.isArray(r)) stagePaths(r);
    else if (typeof r === "string") stagePaths([r]);
  };

  const send = async () => {
    setSending(true);
    try {
      const group = groupId ? groups.find((g) => g.id === groupId) : undefined;
      const sameAsGroup = group && group.memberIds.length === recipients.length && group.memberIds.every((id) => recipients.includes(id));
      const sid = sameAsGroup ? await api.sendToGroup(staged, group.id) : await api.sendFiles(staged, recipients);
      setLastSession(sid);
      toast({ tone: "success", title: `Sending ${pluralize(totals.files, "file")} to ${pluralize(recipients.length, "device")}` });
      clearStaged();
      setInfo({});
    } catch (e) {
      fail(e, "Could not start the transfer");
    } finally {
      setSending(false);
    }
  };

  const sessionDeliveries = lastSession ? Object.values(deliveries).filter((d) => d.sessionId === lastSession) : [];

  return (
    <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
      <div className="space-y-4">
        <motion.div
          layout
          className={cn(
            "relative overflow-hidden rounded-xl border-2 border-dashed transition-colors",
            staged.length ? "border-line bg-surface/50" : "border-line-strong bg-surface/60 hover:border-accent/40",
          )}
        >
          <div className={cn("flex flex-col items-center text-center", staged.length ? "py-6" : "py-14")}>
            <div className="relative mb-4">
              <div className="absolute inset-0 rounded-2xl brand-gradient opacity-25 blur-xl" />
              <div className="relative flex h-14 w-14 items-center justify-center rounded-2xl border border-line-strong bg-surface-2 surface-highlight">
                <UploadCloud size={24} className="text-accent" strokeWidth={1.6} />
              </div>
            </div>
            <div className="text-[14px] font-semibold text-fg">Drop files or folders anywhere</div>
            <div className="mt-1 text-[12.5px] text-fg-muted">
              Any size — streamed directly to the other device and verified with SHA-256
            </div>
            <div className="mt-4 flex gap-2">
              <Button icon={FilePlus2} onClick={pickFiles}>
                Choose files
              </Button>
              <Button icon={FolderPlus} onClick={pickFolder}>
                Choose folder
              </Button>
            </div>
          </div>
        </motion.div>

        {staged.length > 0 && (
          <Panel
            padded={false}
            title={`${pluralize(staged.length, "item")} selected`}
            subtitle={totals.pending ? "Calculating size…" : `${pluralize(totals.files, "file")} · ${formatBytes(totals.size)}`}
            actions={
              <Button size="sm" variant="ghost" icon={Trash2} onClick={clearStaged}>
                Clear
              </Button>
            }
          >
            <div className="max-h-[340px] divide-y divide-line overflow-y-auto">
              <AnimatePresence initial={false}>
                {staged.map((p) => {
                  const i = info[p];
                  return (
                    <motion.div
                      key={p}
                      layout
                      initial={{ opacity: 0, x: -6 }}
                      animate={{ opacity: 1, x: 0 }}
                      exit={{ opacity: 0, height: 0 }}
                      className="flex items-center gap-3 px-4 py-2.5"
                    >
                      <FileIcon name={p} isDir={i?.isDir} size={32} />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-[13px] font-medium text-fg">{i?.name ?? baseName(p)}</div>
                        <div className="selectable truncate text-2xs text-fg-subtle">{p}</div>
                      </div>
                      <div className="tabular shrink-0 text-right text-xs text-fg-muted">
                        {i ? (
                          <>
                            <div>{formatBytes(i.size)}</div>
                            {i.isDir && <div className="text-2xs text-fg-subtle">{pluralize(i.fileCount, "file")}</div>}
                          </>
                        ) : (
                          <Spinner size={12} />
                        )}
                      </div>
                      <IconButton icon={X} label="Remove" size="sm" onClick={() => unstage(p)} />
                    </motion.div>
                  );
                })}
              </AnimatePresence>
            </div>
          </Panel>
        )}

        {sessionDeliveries.length > 0 && (
          <Panel padded={false} title="Just sent" subtitle="Each device receives independently">
            <div className="divide-y divide-line">
              {sessionDeliveries.map((d) => (
                <TransferItem key={d.id} delivery={d} compact />
              ))}
            </div>
          </Panel>
        )}
      </div>

      <div className="lg:sticky lg:top-0 lg:self-start">
        <Panel title="Recipients" subtitle="Only trusted devices can receive from you">
          <RecipientPicker
            value={recipients}
            onChange={(v) => {
              setRecipients(v);
              setGroupId(undefined);
            }}
          />
          <Button
            variant="primary"
            size="lg"
            icon={Send}
            className="mt-4 w-full"
            loading={sending}
            disabled={!staged.length || !recipients.length || totals.pending}
            onClick={send}
          >
            {recipients.length > 1 ? `Send to ${recipients.length} devices` : "Send"}
          </Button>
          {!staged.length && <p className="mt-2 text-center text-2xs text-fg-subtle">Add files or folders to send</p>}
        </Panel>
      </div>
    </div>
  );
}
