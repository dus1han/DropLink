import { AnimatePresence, motion } from "framer-motion";
import { ConfirmHost } from "@/components/dialogs/ConfirmHost";
import { IncomingDialog } from "@/components/dialogs/IncomingDialog";
import { PairingDialog } from "@/components/dialogs/PairingDialog";
import { useApp, type PageId } from "@/store/app";
import { ActivityPage } from "@/pages/Activity";
import { ClipboardPage } from "@/pages/Clipboard";
import { DevicesPage } from "@/pages/Devices";
import { GroupsPage } from "@/pages/Groups";
import { OverviewPage } from "@/pages/Overview";
import { ReceivePage } from "@/pages/Receive";
import { SendPage } from "@/pages/Send";
import { SettingsPage } from "@/pages/Settings";
import { TextPage } from "@/pages/Text";
import { TrustedPage } from "@/pages/Trusted";
import { DropOverlay, Toaster } from "./Overlays";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";

const PAGES: Record<PageId, () => React.ReactElement | null> = {
  overview: OverviewPage,
  devices: DevicesPage,
  send: SendPage,
  text: TextPage,
  receive: ReceivePage,
  clipboard: ClipboardPage,
  activity: ActivityPage,
  groups: GroupsPage,
  trusted: TrustedPage,
  settings: SettingsPage,
};

export function Shell() {
  const page = useApp((s) => s.page);
  const Page = PAGES[page];
  return (
    <div className="app-backdrop flex h-full">
      <Sidebar />
      <main className="flex min-w-0 flex-1 flex-col">
        <TopBar />
        <div className="relative min-h-0 flex-1 overflow-y-auto">
          <AnimatePresence mode="wait">
            <motion.div
              key={page}
              initial={{ opacity: 0, y: 6 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -2, transition: { duration: 0.08 } }}
              transition={{ duration: 0.2, ease: [0.22, 1, 0.36, 1] }}
              className="mx-auto w-full max-w-[1180px] px-6 py-6"
            >
              <Page />
            </motion.div>
          </AnimatePresence>
        </div>
      </main>
      <DropOverlay />
      <PairingDialog />
      <IncomingDialog />
      <ConfirmHost />
      <Toaster />
    </div>
  );
}
