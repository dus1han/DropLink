import { useEffect } from "react";
import type { Theme } from "@/lib/types";

/** Applies the theme setting to <html data-theme>, following the OS for "system". */
export function useTheme(theme: Theme | undefined) {
  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      const resolved = theme === "system" || !theme ? (media.matches ? "dark" : "light") : theme;
      document.documentElement.dataset.theme = resolved;
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
}
