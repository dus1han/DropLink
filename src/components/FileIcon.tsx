import {
  File,
  FileArchive,
  FileAudio,
  FileCode,
  FileImage,
  FileSpreadsheet,
  FileText,
  FileVideo,
  Folder,
  Type,
} from "lucide-react";
import { cn } from "@/lib/cn";
import { fileExtension } from "@/lib/format";

const GROUPS: [string[], typeof File, string][] = [
  [["jpg", "jpeg", "png", "gif", "webp", "heic", "svg", "bmp", "tiff", "raw", "psd"], FileImage, "text-[#c084fc]"],
  [["mp4", "mov", "mkv", "avi", "webm", "m4v"], FileVideo, "text-[#f472b6]"],
  [["mp3", "wav", "flac", "aac", "m4a", "ogg"], FileAudio, "text-[#fb923c]"],
  [["zip", "rar", "7z", "tar", "gz", "dmg", "iso", "pkg", "msi", "exe"], FileArchive, "text-[#fbbf24]"],
  [["xls", "xlsx", "csv", "numbers"], FileSpreadsheet, "text-[#34d399]"],
  [["pdf", "doc", "docx", "txt", "md", "rtf", "pages", "key", "ppt", "pptx"], FileText, "text-[#60a5fa]"],
  [["js", "ts", "tsx", "rs", "py", "go", "json", "html", "css", "swift", "java", "c", "cpp", "sh"], FileCode, "text-[#a3e635]"],
];

export function FileIcon({
  name,
  isDir,
  kind,
  size = 32,
  className,
}: {
  name?: string | null;
  isDir?: boolean;
  kind?: "files" | "folder" | "text";
  size?: number;
  className?: string;
}) {
  let Icon: typeof File = File;
  let color = "text-fg-muted";
  if (kind === "text") {
    Icon = Type;
    color = "text-accent";
  } else if (isDir || kind === "folder") {
    Icon = Folder;
    color = "text-[#7cc4fa]";
  } else {
    const ext = fileExtension(name);
    for (const [exts, I, c] of GROUPS) {
      if (exts.includes(ext)) {
        Icon = I;
        color = c;
        break;
      }
    }
  }
  return (
    <div
      className={cn("flex shrink-0 items-center justify-center rounded-lg border border-line bg-surface-2", color, className)}
      style={{ width: size, height: size }}
    >
      <Icon size={Math.round(size * 0.5)} strokeWidth={1.7} />
    </div>
  );
}
