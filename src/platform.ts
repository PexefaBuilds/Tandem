export const isMac = /Mac/i.test(navigator.userAgent);
export const inTauri = "__TAURI_INTERNALS__" in window;

/** Terminal choices; "wt" doubles as "the default terminal" (Terminal.app on a Mac). */
export const terminalChoices: [Terminal, string][] = isMac
  ? [["wt", "Terminal"], ["iterm", "iTerm"], ["ghostty", "Ghostty"]]
  : [["wt", "Windows Terminal"], ["powershell", "PowerShell"]];
export type Terminal = "wt" | "powershell" | "iterm" | "ghostty";
