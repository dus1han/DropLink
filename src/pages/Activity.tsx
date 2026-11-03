import { useEffect, useMemo, useState } from "react";
import { AnimatePresence } from "framer-motion";
import { ArrowDownLeft, ArrowUpRight, History, ListChecks, RotateCw, Search, ShieldCheck, ShieldX, Trash2, Zap } from "lucide-react";
import { FileIcon } from "@/components/FileIcon";
import { TransferItem } from "@/components/TransferItem";
import { Badge } from "@/components/ui/Badge";
import { Button, IconButton } from "@/components/ui/Button";
import { Input, Select } from "@/components/ui/Field";
import { EmptyState, SectionTitle, Segmented } from "@/components/ui/misc";
import { Spinner } from "@/components/ui/Spinner";
import { api } from "@/lib/api";
import { cn } from "@/lib/cn";
import { STATUS_LABEL, formatBytes, formatDateTime, formatDuration, isActive, pluralize, startOfToday } from "@/lib/format";
import type { HistoryEntry } from "@/lib/types";
import { confirm } from "@/store/confirm";
import { useApp } from "@/store/app";

export function ActivityPage() {
  const [tab, setTab] = useState<"live" | "history">("live");
  const activeCount = useApp((s) => Object.values(s.deliveries).filter((d) => isActive(d.status)).length);
  return (
    <div>
      <div className="mb-5">
        <Segmented
          value={tab}
          onChange={setTab}
          options={[
            { value: "live", label: "Transfers", icon: Zap, count: activeCount },
            { value: "history", label: "History", icon: History },
          ]}
        />
      </div>
      {tab === "live" ? <LiveTransfers /> : <HistoryView />}
    </div>
  );
}

function LiveTransfers() {
  const deliveriesMap = useApp((s) => s.deliveries);
  const fail = useApp((s) => s.fail);
  const all = useMemo(
    () => Object.values(deliveriesMap).sort((a, b) => (b.startedAt ?? b.createdAt) - (a.startedAt ?? a.createdAt)),
    [deliveriesMap],
  );
  const active = all.filter((d) => !["completed", "failed", "cancelled", "rejected"].includes(d.status));
  const finished = all.filter((d) => ["completed", "failed", "cancelled", "rejected"].includes(d.status));

  const clear = async () => {
    try {
      await api.clearFinishedDeliveries();
      const fresh = await api.listDeliveries();
      useApp.setState({ deliveries: Object.fromEntries(fresh.map((d) => [d.id, d])) });
    } catch (e) {
      fail(e);
    }
  };

  return (
    <div className="space-y-6">
      <section>
        <SectionTitle count={active.length}>In progress</SectionTitle>
        <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
          {active.length === 0 ? (
            <EmptyState compact icon={Zap} title="No active transfers" description="Running, queued and paused transfers appear here with live progress." />
          ) : (
            <div className="divide-y divide-line">
              <AnimatePresence initial={false}>
                {active.map((d) => (
                  <TransferItem key={d.id} delivery={d} />
                ))}
              </AnimatePresence>
            </div>
          )}
        </div>
      </section>
      {finished.length > 0 && (
        <section>
          <SectionTitle
            count={finished.length}
            actions={
              <Button size="sm" variant="ghost" icon={ListChecks} onClick={clear}>
                Clear finished
              </Button>
            }
          >
            Finished
          </SectionTitle>
          <div className="divide-y divide-line overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
            <AnimatePresence initial={false}>
              {finished.map((d) => (
                <TransferItem key={d.id} delivery={d} />
              ))}
            </AnimatePresence>
          </div>
        </section>
      )}
    </div>
  );
}

type Range = "all" | "today" | "7d" | "30d";

function HistoryView() {
  const devices = useApp((s) => s.devices);
  const statsVersion = useApp((s) => s.statsVersion);
  const fail = useApp((s) => s.fail);
  const toast = useApp((s) => s.toast);
  const [search, setSearch] = useState("");
  const [status, setStatus] = useState("all");
  const [deviceId, setDeviceId] = useState("");
  const [direction, setDirection] = useState<"all" | "outgoing" | "incoming">("all");
  const [range, setRange] = useState<Range>("all");
  const [rows, setRows] = useState<HistoryEntry[] | null>(null);

  useEffect(() => {
    const fromMs =
      range === "today" ? startOfToday() : range === "7d" ? Date.now() - 7 * 864e5 : range === "30d" ? Date.now() - 30 * 864e5 : undefined;
    const t = window.setTimeout(() => {
      api
        .history({ search, status, deviceId: deviceId || undefined, direction: direction === "all" ? undefined : direction, fromMs })
        .then(setRows)
        .catch((e) => fail(e, "Could not load history"));
    }, 180);
    return () => window.clearTimeout(t);
  }, [search, status, deviceId, direction, range, statsVersion, fail]);

  const deviceOptions = useMemo(() => Object.values(devices).sort((a, b) => a.name.localeCompare(b.name)), [devices]);

  const retry = async (h: HistoryEntry) => {
    try {
      await api.resumeDelivery(h.deliveryId);
      toast({ tone: "info", title: `Retrying ${h.name}` });
    } catch (e) {
      fail(e, "Could not retry");
    }
  };

  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        <div className="w-64">
          <Input icon={Search} placeholder="Search files and devices" value={search} onChange={(e) => setSearch(e.target.value)} />
        </div>
        <Segmented
          size="sm"
          value={direction}
          onChange={setDirection}
          options={[
            { value: "all", label: "All" },
            { value: "outgoing", label: "Sent" },
            { value: "incoming", label: "Received" },
          ]}
        />
        <div className="w-36">
          <Select value={status} onChange={(e) => setStatus(e.target.value)}>
            <option value="all">Any status</option>
            <option value="completed">Successful</option>
            <option value="unsuccessful">Unsuccessful</option>
            <option value="failed">Failed</option>
            <option value="cancelled">Cancelled</option>
            <option value="rejected">Declined</option>
          </Select>
        </div>
        <div className="w-40">
          <Select value={deviceId} onChange={(e) => setDeviceId(e.target.value)}>
            <option value="">All devices</option>
            {deviceOptions.map((d) => (
              <option key={d.id} value={d.id}>
                {d.name}
              </option>
            ))}
          </Select>
        </div>
        <div className="w-32">
          <Select value={range} onChange={(e) => setRange(e.target.value as Range)}>
            <option value="all">All time</option>
            <option value="today">Today</option>
            <option value="7d">Last 7 days</option>
            <option value="30d">Last 30 days</option>
          </Select>
        </div>
        <div className="flex-1" />
        <Button
          size="sm"
          variant="ghost"
          icon={Trash2}
          disabled={!rows?.length}
          onClick={async () => {
            if (await confirm({ title: "Clear transfer history?", body: "Records of past transfers are deleted. Received files are not affected.", confirmLabel: "Clear history", tone: "danger" })) {
              await api.clearHistory().catch(fail);
              setRows([]);
            }
          }}
        >
          Clear history
        </Button>
      </div>

      <div className="overflow-hidden rounded-xl border border-line bg-surface surface-highlight">
        <div className="grid grid-cols-[minmax(0,1fr)_150px_90px_120px_110px_64px] items-center gap-3 border-b border-line px-4 py-2 text-2xs font-medium tracking-wide text-fg-subtle uppercase">
          <span>Transfer</span>
          <span>Device</span>
          <span className="text-right">Size</span>
          <span>Date</span>
          <span>Status</span>
          <span />
        </div>
        {rows === null ? (
          <div className="flex justify-center py-12 text-fg-subtle">
            <Spinner />
          </div>
        ) : rows.length === 0 ? (
          <EmptyState compact icon={History} title="No matching transfers" description="Try a different search or filter." />
        ) : (
          <div className="divide-y divide-line">
            {rows.map((h) => (
              <HistoryRow key={h.id} h={h} onRetry={() => retry(h)} />
            ))}
          </div>
        )}
      </div>
      {rows && rows.length > 0 && <div className="mt-2 text-right text-2xs text-fg-subtle">{pluralize(rows.length, "record")}</div>}
    </div>
  );
}

function HistoryRow({ h, onRetry }: { h: HistoryEntry; onRetry: () => void }) {
  const out = h.direction === "outgoing";
  const tone = h.status === "completed" ? "success" : h.status === "cancelled" ? "neutral" : "danger";
  return (
    <div className="grid grid-cols-[minmax(0,1fr)_150px_90px_120px_110px_64px] items-center gap-3 px-4 py-2.5">
      <div className="flex min-w-0 items-center gap-3">
        <FileIcon name={h.name} kind={h.kind} size={30} />
        <div className="min-w-0">
          <div className="truncate text-[13px] font-medium text-fg">{h.name}</div>
          <div className="truncate text-2xs text-fg-subtle">
            {h.kind === "text" ? "Text message" : pluralize(h.fileCount, "file")}
            {h.durationMs != null && h.status === "completed" && ` · ${formatDuration(h.durationMs / 1000)}`}
            {h.error && h.status !== "completed" && <span className="text-fg-muted"> · {h.error}</span>}
          </div>
        </div>
      </div>
      <div className="flex min-w-0 items-center gap-1.5 text-xs text-fg-muted">
        {out ? <ArrowUpRight size={12} className="shrink-0 text-accent" /> : <ArrowDownLeft size={12} className="shrink-0 text-info" />}
        <span className="truncate">{out ? h.recipientName : h.senderName}</span>
      </div>
      <div className="tabular text-right text-xs text-fg-muted">{formatBytes(h.totalBytes)}</div>
      <div className="tabular text-xs text-fg-muted">{formatDateTime(h.finishedAt)}</div>
      <div className="flex items-center gap-1.5">
        <Badge tone={tone}>{STATUS_LABEL[h.status]}</Badge>
        {h.verified === true && (
          <span title="SHA-256 verified" className="text-success">
            <ShieldCheck size={13} />
          </span>
        )}
        {h.verified === false && (
          <span title="Integrity check failed" className="text-danger">
            <ShieldX size={13} />
          </span>
        )}
      </div>
      <div className={cn("flex justify-end")}>
        {h.retryable && <IconButton icon={RotateCw} label="Retry" size="sm" onClick={onRetry} />}
      </div>
    </div>
  );
}
