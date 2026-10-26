export type Theme = "system" | "light" | "dark";
const KEY = "tandem.theme";

export const savedTheme = (): Theme => {
  const v = localStorage.getItem(KEY);
  return v === "light" || v === "dark" ? v : "system";
};

// "system" removes the override so prefers-color-scheme decides.
export function applyTheme(t: Theme) {
  localStorage.setItem(KEY, t);
  if (t === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = t;
  syncIcon();
}

const dark = () => matchMedia("(prefers-color-scheme: dark)");
export const isDark = () => (document.documentElement.dataset.theme ? document.documentElement.dataset.theme === "dark" : dark().matches);

/** Swaps the window (and taskbar) icon to match the effective theme. */
export async function syncIcon() {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const bytes = new Uint8Array(await (await fetch(isDark() ? "/icon-dark.png" : "/icon-light.png")).arrayBuffer());
    await getCurrentWindow().setIcon(bytes);
    await (await import("./api")).api.setTrayIcon(isDark());
  } catch { /* icon is cosmetic */ }
}
