import { useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowDownLeft, ArrowUpRight, Copy, Download, MessageSquareText, Send, Trash2 } from "lucide-react";
import { RecipientPicker } from "@/components/RecipientPicker";
import { Button, IconButton } from "@/components/ui/Button";
import { Textarea } from "@/components/ui/Field";
import { EmptyState, Panel, Segmented } from "@/components/ui/misc";
import { Kbd } from "@/components/ui/Badge";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { formatDateTime } from "@/lib/format";
import type { TextMessage } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { useApp } from "@/store/app";

const MAX_BYTES = 1_000_000;

export function TextPage() {
  const preselect = useApp((s) => s.preselect);
  const texts = useApp((s) => s.texts);
  const loadTexts = useApp((s) => s.loadTexts);
  const platform = useApp((s) => s.platform);
  const { fail, toast } = useApp.getState();
  const [content, setContent] = useState("");
  const [recipients, setRecipients] = useState<string[]>(preselect.recipients);
  const [sending, setSending] = useState(false);
  const [filter, setFilter] = useState<"all" | "incoming" | "outgoing">("all");

  useEffect(() => {
    void loadTexts();
  }, [loadTexts]);
  useEffect(() => {
    if (preselect.nonce) setRecipients(preselect.recipients);
  }, [preselect.nonce]);

  const bytes = useMemo(() => new TextEncoder().encode(content).length, [content]);
  const canSend = content.trim().length > 0 && recipients.length > 0 && bytes <= MAX_BYTES;

  const send = async () => {
    if (!canSend) return;
    setSending(true);
    try {
      const results = await api.sendText(content, recipients);
      const failed = results.filter((r) => !r.ok);
      if (failed.length === 0) {
        toast({ tone: "success", title: results.length > 1 ? `Sent to ${results.length} devices` : `Sent to ${results[0].deviceName}` });
        setContent("");
      } else {
        toast({
          tone: "error",
          title: `Not delivered to ${failed.map((f) => f.deviceName).join(", ")}`,
          body: failed[0].error ?? undefined,
        });
      }
      await loadTexts();
    } catch (e) {
      fail(e, "Could not send text");
    } finally {
      setSending(false);
    }
  };

  const list = (texts ?? []).filter((t) => filter === "all" || t.direction === filter);
  const mod = platform === "macos" ? "⌘" : "Ctrl";

  return (
    <div className="grid gap-6 lg:grid-cols-[400px_minmax(0,1fr)]">
      <div className="space-y-4 lg:sticky lg:top-0 lg:self-start">
        <Panel title="Compose" subtitle="Plain text keeps line breaks, spacing and indentation">
          <Textarea
            autoFocus
            rows={9}
            value={content}
            onChange={(e) => setContent(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                e.preventDefault();
                void send();
              }
            }}
            placeholder="Write or paste text, a link, a code snippet…"
          />
          <div className="mt-1.5 flex items-center justify-between text-2xs text-fg-subtle">
            <span className={cn("tabular", bytes > MAX_BYTES && "text-danger")}>
              {content.length.toLocaleString()} characters{bytes > MAX_BYTES && " — too large, send it as a file"}
            </span>
            <span className="flex items-center gap-1">
              <Kbd>{mod}</Kbd>
              <Kbd>↵</Kbd> to send
            </span>
          </div>
          <div className="mt-4">
            <RecipientPicker value={recipients} onChange={setRecipients} />
          </div>
          <Button variant="primary" size="lg" icon={Send} className="mt-4 w-full" disabled={!canSend} loading={sending} onClick={send}>
            {recipients.length > 1 ? `Send to ${recipients.length} devices` : "Send text"}
          </Button>
        </Panel>
      </div>

      <div>
        <div className="mb-3 flex items-center gap-2">
          <Segmented
            value={filter}
            onChange={setFilter}
            options={[
              { value: "all", label: "All" },
              { value: "incoming", label: "Received" },
              { value: "outgoing", label: "Sent" },
            ]}
          />
          <div className="flex-1" />
          {!!texts?.length && (
            <Button
              size="sm"
              variant="ghost"
              icon={Trash2}
              onClick={async () => {
                if (await confirm({ title: "Delete all messages?", body: "Sent and received text messages are removed from this computer.", confirmLabel: "Delete all", tone: "danger" })) {
                  await api.clearTexts().catch(fail);
                  await loadTexts();
                }
              }}
            >
              Clear
            </Button>
          )}
        </div>
        {list.length === 0 ? (
          <div className="rounded-xl border border-line bg-surface">
            <EmptyState
              icon={MessageSquareText}
              title={texts === null ? "Loading…" : "No messages"}
              description="Text you send or receive appears here, ready to copy or save."
            />
          </div>
        ) : (
          <div className="space-y-3">
            <AnimatePresence initial={false}>
              {list.map((m) => (
                <MessageCard key={m.id} message={m} onDeleted={loadTexts} />
              ))}
            </AnimatePresence>
          </div>
        )}
      </div>
    </div>
  );
}

function MessageCard({ message: m, onDeleted }: { message: TextMessage; onDeleted: () => void }) {
  const { fail, toast } = useApp.getState();
  const incoming = m.direction === "incoming";
  const lines = m.content.split("\n").length;
  const [expanded, setExpanded] = useState(lines <= 14 && m.content.length < 1400);

  const copy = async () => {
    try {
      await api.copyText(m.content);
      toast({ tone: "success", title: "Copied to clipboard" });
    } catch {
      await navigator.clipboard.writeText(m.content).catch(fail);
    }
  };
  const saveTxt = async () => {
    const stamp = new Date(m.createdAt).toISOString().slice(0, 16).replace(/[:T]/g, "-");
    const path = await save({ defaultPath: `DropLink text ${stamp}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
    if (!path) return;
    try {
      await api.saveTextFile(path, m.content);
      toast({ tone: "success", title: "Saved", body: path });
    } catch (e) {
      fail(e, "Could not save the file");
    }
  };

  return (
    <motion.article
      layout="position"
      initial={{ opacity: 0, y: 6 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.98 }}
      className="group rounded-xl border border-line bg-surface surface-highlight"
    >
      <header className="flex items-center gap-2 px-4 pt-3">
        <span className={cn("flex h-5 w-5 items-center justify-center rounded-full", incoming ? "bg-info-soft text-info" : "bg-accent-soft text-accent")}>
          {incoming ? <ArrowDownLeft size={11} /> : <ArrowUpRight size={11} />}
        </span>
        <span className="text-xs text-fg-muted">
          {incoming ? "From" : "To"} <span className="font-medium text-fg">{m.deviceName}</span>
        </span>
        <span className="text-2xs text-fg-subtle">· {formatDateTime(m.createdAt)}</span>
        <div className="ml-auto flex gap-0.5 opacity-70 transition-opacity group-hover:opacity-100">
          <IconButton icon={Copy} label="Copy" size="sm" onClick={copy} />
          <IconButton icon={Download} label="Save as .txt" size="sm" onClick={saveTxt} />
          <IconButton
            icon={Trash2}
            label="Delete"
            size="sm"
            tone="danger"
            onClick={() =>
              api
                .deleteText(m.id)
                .then(onDeleted)
                .catch(fail)
            }
          />
        </div>
      </header>
      <div className="relative px-4 pt-2 pb-3.5">
        <pre
          className={cn(
            "selectable font-sans text-[13px] leading-relaxed break-words whitespace-pre-wrap text-fg",
            !expanded && "max-h-[260px] overflow-hidden",
          )}
        >
          {m.content}
        </pre>
        {!expanded && (
          <div className="absolute inset-x-0 bottom-0 flex h-20 items-end justify-center rounded-b-xl bg-gradient-to-t from-surface to-transparent pb-3">
            <Button size="sm" variant="secondary" onClick={() => setExpanded(true)}>
              Show all {lines} lines
            </Button>
          </div>
        )}
      </div>
    </motion.article>
  );
}
