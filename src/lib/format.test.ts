import { describe, expect, it } from "vitest";
import { baseName, fileExtension, formatBytes, formatDuration, formatRelative, joinPath, pluralize } from "./format";

describe("formatBytes", () => {
  it("uses decimal units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(999)).toBe("999 B");
    expect(formatBytes(1500)).toBe("1.5 KB");
    expect(formatBytes(1_000_000)).toBe("1.0 MB");
    expect(formatBytes(250_000_000)).toBe("250 MB");
    expect(formatBytes(1_073_741_824)).toBe("1.1 GB");
  });
  it("handles invalid input", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(NaN)).toBe("—");
  });
});

describe("formatDuration", () => {
  it("formats seconds, minutes and hours", () => {
    expect(formatDuration(5.4)).toBe("5s");
    expect(formatDuration(65)).toBe("1m 05s");
    expect(formatDuration(3720)).toBe("1h 02m");
    expect(formatDuration(null)).toBe("—");
  });
});

describe("formatRelative", () => {
  it("describes recent times", () => {
    const now = 1_000_000_000_000;
    expect(formatRelative(now - 2000, now)).toBe("just now");
    expect(formatRelative(now - 45_000, now)).toBe("45s ago");
    expect(formatRelative(now - 5 * 60_000, now)).toBe("5m ago");
    expect(formatRelative(now - 3 * 3600_000, now)).toBe("3h ago");
  });
});

describe("paths", () => {
  it("extracts names and extensions across separators", () => {
    expect(baseName("C:\\Users\\me\\Report.PDF")).toBe("Report.PDF");
    expect(baseName("/Users/me/folder/")).toBe("folder");
    expect(fileExtension("dir/archive.tar.gz")).toBe("gz");
    expect(fileExtension(".bashrc")).toBe("");
    expect(joinPath("C:\\Downloads", "a/b.txt")).toBe("C:\\Downloads\\a\\b.txt");
    expect(joinPath("/Users/me/Downloads/", "x.txt")).toBe("/Users/me/Downloads/x.txt");
  });
  it("pluralizes", () => {
    expect(pluralize(1, "file")).toBe("1 file");
    expect(pluralize(3, "file")).toBe("3 files");
  });
});
