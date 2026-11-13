import { getCurrentWindow } from "@tauri-apps/api/window";
import { onLangChange, t } from "./i18n";
import { IS_LINUX, IS_MAC, OS } from "./platform";

export function initWindowChrome(version: string, beforeClose: () => Promise<boolean> = async () => true) {
  if (document.getElementById("app-titlebar")) return;
  const win = () => getCurrentWindow();
  const bar = document.createElement("header");
  bar.id = "app-titlebar";
  bar.className = "app-titlebar";
  const identity = document.createElement("div");
  identity.className = "app-titlebar-identity";
  const icon = document.createElement("img");
  icon.src = "/favicon.png"; icon.alt = ""; icon.draggable = false; icon.width = 16; icon.height = 16;
  const name = document.createElement("span");
  name.textContent = "MK Skin Studio";
  const ver = document.createElement("span");
  ver.className = "app-titlebar-version";
  ver.textContent = version;
  identity.append(icon, name, ver);
  const status = document.createElement("span");
  status.className = "window-chrome-status";
  status.setAttribute("role", "status");
  status.setAttribute("aria-live", "polite");

  const controls = document.createElement("div");
  controls.className = "window-controls";
  const button = (id: string, glyph: string, extra = "") => {
    const b = document.createElement("button");
    b.type = "button"; b.id = id; b.className = `window-control ${extra}`.trim();
    const g = document.createElement("span");
    g.className = `window-glyph ${glyph}`; g.setAttribute("aria-hidden", "true");
    b.append(g);
    return b;
  };
  const minimize = button("window-minimize", "window-glyph-minimize");
  const maximize = button("window-maximize", "window-glyph-maximize");
  const close = button("window-close", "window-glyph-close", "window-control-close");
  controls.append(minimize, maximize, close);
  bar.append(identity, status, controls);
  document.body.prepend(bar);
  document.body.classList.add("window-chrome-mounted");
  document.documentElement.dataset.os = OS;
  if (IS_LINUX && (window as { __MKSS_ROUNDED__?: boolean }).__MKSS_ROUNDED__) document.documentElement.classList.add("window-rounded");
  if (IS_MAC) {
    bar.classList.add("app-titlebar-mac");
    controls.hidden = true;
  }
  if (IS_LINUX) mountResizeEdges(win, () => maximized);

  const fail = () => { status.textContent = t("window.error"); status.title = status.textContent; };
  const run = async (f: () => Promise<unknown>) => {
    status.textContent = ""; status.removeAttribute("title");
    try { await f(); } catch { fail(); }
  };
  let maximized = false;
  const syncMax = async () => {
    try { maximized = await win().isMaximized(); } catch {}
    maximize.querySelector(".window-glyph")!.className = `window-glyph ${maximized ? "window-glyph-restore" : "window-glyph-maximize"}`;
    const label = t(maximized ? "window.restore" : "window.maximize");
    maximize.setAttribute("aria-label", label); maximize.title = label;
    document.body.classList.toggle("window-maximized", maximized);
  };
  minimize.addEventListener("click", () => void run(() => win().minimize()));
  maximize.addEventListener("click", () => void run(() => win().toggleMaximize()));
  close.addEventListener("click", () => void run(async () => { if (await beforeClose()) await win().close(); }));
  bar.addEventListener("mousedown", (e) => {
    if (e.button !== 0 || (e.target instanceof Element && e.target.closest("button"))) return;
    e.preventDefault();
    if (e.detail === 2) void run(() => win().toggleMaximize());
    else void run(() => win().startDragging());
  });
  try { void win().onResized(() => void syncMax()).catch(() => {}); } catch {}

  const translate = () => {
    minimize.setAttribute("aria-label", t("window.minimize")); minimize.title = t("window.minimize");
    close.setAttribute("aria-label", t("window.close")); close.title = t("window.close");
    if (status.textContent) fail();
    void syncMax();
  };
  translate();
  onLangChange(translate);
}

type ResizeDirection = "North" | "South" | "East" | "West" | "NorthEast" | "NorthWest" | "SouthEast" | "SouthWest";

function mountResizeEdges(win: () => ReturnType<typeof getCurrentWindow>, isMaximized: () => boolean) {
  const edges: [ResizeDirection, string][] = [
    ["North", "n"], ["South", "s"], ["East", "e"], ["West", "w"],
    ["NorthEast", "ne"], ["NorthWest", "nw"], ["SouthEast", "se"], ["SouthWest", "sw"],
  ];
  const host = document.createElement("div");
  host.className = "window-resize-edges";
  host.setAttribute("aria-hidden", "true");
  for (const [dir, cls] of edges) {
    const edge = document.createElement("div");
    edge.className = `window-resize-edge window-resize-${cls}`;
    edge.addEventListener("mousedown", (e) => {
      if (e.button !== 0 || isMaximized()) return;
      e.preventDefault();
      e.stopPropagation();
      try { void win().startResizeDragging(dir).catch(() => {}); } catch {}
    });
    host.append(edge);
  }
  document.body.append(host);
}
