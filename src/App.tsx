import { useEffect } from "react";
import { motion } from "framer-motion";
import { TriangleAlert } from "lucide-react";
import { LogoMark } from "@/components/Logo";
import { useTheme } from "@/hooks/useTheme";
import { Shell } from "@/layout/Shell";
import { useApp } from "@/store/app";
import { useUpdate } from "@/store/update";

export default function App() {
  const ready = useApp((s) => s.ready);
  const fatal = useApp((s) => s.fatal);
  const theme = useApp((s) => s.settings?.theme);
  const init = useApp((s) => s.init);
  useTheme(theme);

  useEffect(() => {
    void init();
  }, [init]);

  // Look for a new release shortly after startup, then every six hours.
  useEffect(() => {
    if (!ready) return;
    const run = () => void useUpdate.getState().checkForUpdates(true);
    const first = window.setTimeout(run, 8_000);
    const every = window.setInterval(run, 6 * 3_600_000);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(every);
    };
  }, [ready]);

  if (fatal) {
    return (
      <div className="flex h-full items-center justify-center bg-bg p-8">
        <div className="max-w-md text-center">
          <TriangleAlert className="mx-auto text-danger" size={32} />
          <h1 className="mt-4 text-[15px] font-semibold text-fg">DropLink could not start</h1>
          <p className="selectable mt-2 text-[13px] leading-relaxed text-fg-muted">{fatal}</p>
        </div>
      </div>
    );
  }

  if (!ready) {
    return (
      <div data-tauri-drag-region className="flex h-full items-center justify-center bg-bg">
        <motion.div animate={{ opacity: [0.5, 1, 0.5], scale: [0.98, 1, 0.98] }} transition={{ duration: 1.6, repeat: Infinity }}>
          <LogoMark size={44} />
        </motion.div>
      </div>
    );
  }

  return <Shell />;
}
