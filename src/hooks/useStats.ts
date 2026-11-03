import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import { startOfToday } from "@/lib/format";
import type { Stats } from "@/lib/types";
import { useApp } from "@/store/app";

/** Dashboard numbers, refetched (debounced) whenever transfers or devices change. */
export function useStats(): Stats | null {
  const version = useApp((s) => s.statsVersion);
  const [stats, setStats] = useState<Stats | null>(null);
  useEffect(() => {
    const t = window.setTimeout(() => {
      api.stats(startOfToday()).then(setStats).catch(() => undefined);
    }, 250);
    return () => window.clearTimeout(t);
  }, [version]);
  return stats;
}
