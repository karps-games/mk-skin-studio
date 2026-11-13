import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemePref = "system" | "light" | "dark";
const KEY = "karps.theme";
const media = window.matchMedia("(prefers-color-scheme: dark)");
const isPref = (value: string | null): value is ThemePref => value === "system" || value === "light" || value === "dark";

export function themePref(): ThemePref {
  try {
    const saved = localStorage.getItem(KEY);
    return isPref(saved) ? saved : "system";
  } catch {
    return "system";
  }
}

export const resolvedTheme = (pref: ThemePref = themePref()): "light" | "dark" =>
  pref === "system" ? (media.matches ? "dark" : "light") : pref;

function apply(pref: ThemePref) {
  const root = document.documentElement;
  root.dataset.theme = resolvedTheme(pref);
  root.dataset.themePref = pref;
}

function applyNative(pref: ThemePref) {
  if (!("__TAURI_INTERNALS__" in window)) return;
  try {
    getCurrentWindow().setTheme(pref === "system" ? null : pref).catch(() => {});
  } catch {}
}

export function setThemePref(pref: ThemePref) {
  try {
    localStorage.setItem(KEY, pref);
  } catch {}
  apply(pref);
  applyNative(pref);
}

export function initTheme() {
  const pref = themePref();
  apply(pref);
  applyNative(pref);
  media.addEventListener("change", () => {
    if (themePref() === "system") apply("system");
  });
  const select = document.getElementById("opt-theme") as HTMLSelectElement | null;
  if (!select) return;
  select.value = pref;
  select.addEventListener("change", () => setThemePref(isPref(select.value) ? select.value : "system"));
}
