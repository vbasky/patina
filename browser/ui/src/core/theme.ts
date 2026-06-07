// Light/dark theme with three modes:
//   "system" — follow the OS preference live (default),
//   "light" / "dark" — an explicit user override.
// The resolved value toggles `.dark` on <html>; the chosen mode is persisted.

export type ThemeMode = "system" | "light" | "dark";

const KEY = "patina-theme";
const mql = window.matchMedia("(prefers-color-scheme: dark)");
const listeners = new Set<() => void>();

export function getThemeMode(): ThemeMode {
  const saved = localStorage.getItem(KEY);
  return saved === "light" || saved === "dark" ? saved : "system";
}

function resolveDark(mode: ThemeMode): boolean {
  return mode === "system" ? mql.matches : mode === "dark";
}

function apply(mode: ThemeMode): void {
  document.documentElement.classList.toggle("dark", resolveDark(mode));
  for (const fn of listeners) fn();
}

export function initTheme(): void {
  apply(getThemeMode());
  // Follow OS changes live while no explicit override is set.
  mql.addEventListener("change", () => {
    if (getThemeMode() === "system") apply("system");
  });
}

export function isDark(): boolean {
  return document.documentElement.classList.contains("dark");
}

export function setThemeMode(mode: ThemeMode): void {
  if (mode === "system") localStorage.removeItem(KEY);
  else localStorage.setItem(KEY, mode);
  apply(mode);
}

// Cycle System → Light → Dark → System.
export function cycleTheme(): ThemeMode {
  const next: Record<ThemeMode, ThemeMode> = {
    system: "light",
    light: "dark",
    dark: "system",
  };
  const mode = next[getThemeMode()];
  setThemeMode(mode);
  return mode;
}

// Subscribe to resolved-theme changes (manual switch or live OS change).
// Returns an unsubscribe function.
export function onThemeChange(fn: () => void): () => void {
  listeners.add(fn);
  return () => {
    listeners.delete(fn);
  };
}
