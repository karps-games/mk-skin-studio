import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import { openUrl, revealItemInDir } from "@tauri-apps/plugin-opener";
import { applyStatic, errText, locale, mountLangPicker, onLangChange, t, type Key } from "./i18n";
import { initTheme } from "./theme";
import { initWindowChrome } from "./window-chrome";
import { initTutorial } from "./tour";
import { GameDiscovery } from "./game-discovery";
import { Editor, type CharacterView } from "./editor/editor";
import { apply as applyProject, parseManifest, serialize, type Project } from "./editor/project";
import { composite } from "./editor/doc";

const VERSION = "1.0.0";
type Tab = "projects" | "new" | "characters" | "studio";
interface CharacterSummary { code: string; name: string; files: number; portrait?: string | null }
interface GameInfo { root: string; source: "folder" | "image"; id?: string | null; characters: CharacterSummary[] }
interface CharEntry { code: string; name: string; savedAt: number; edits: string[] }
interface Meta { id: string; name: string; created: number; updated: number; game: string; gamePath: string; gameId: string; characters: CharEntry[] }
interface ExportResult { path: string; bytes: number; files: number; textures: number }
interface Progress { done: number; total: number; file: string }

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;

const state = {
  tab: "projects" as Tab,
  projects: [] as Meta[],
  project: null as Meta | null,
  game: null as GameInfo | null,
  gamePath: null as string | null,
  character: null as CharacterView | null,
  savedAt: 0,
  autosavedAt: 0,
  newMode: "new" as "new" | "attach",
  attachId: null as string | null,
  exporting: false,
  lastExport: null as (ExportResult & { characters: number }) | null,
  closeApproved: false,
};
let editor: Editor;
let discovery: GameDiscovery;

let toastTimer = 0;
function toast(msg: string, kind: "error" | "ok" = "error") {
  const el = $("toast");
  el.textContent = msg;
  el.className = `toast ${kind}`;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => el.classList.add("hidden"), kind === "error" ? 8000 : 4000);
}
function busy(text: string | null) {
  $("busy").classList.toggle("hidden", text === null);
  if (text !== null) $("busy-text").textContent = text;
}
async function run<T>(label: string | null, fn: () => Promise<T>): Promise<T | undefined> {
  busy(label);
  try {
    return await fn();
  } catch (e) {
    toast(errText(e instanceof Error ? e.message : e));
    return undefined;
  } finally {
    busy(null);
  }
}
const fmtBytes = (n: number) => new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }).format(n / 1024 / 1024) + " MB";
const fmtTime = (ms: number) => new Date(ms).toLocaleString(locale(), { dateStyle: "medium", timeStyle: "short" });
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);
const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const LAST_GAME = "mkss.lastGamePath";
const lastGame = () => { try { return localStorage.getItem(LAST_GAME); } catch { return null; } };

function ask(o: { title: string; text?: string; input?: { label: string; value: string }; buttons: { id: string; label: string; primary?: boolean; danger?: boolean }[] }): Promise<{ id: string; value: string }> {
  const dlg = $<HTMLDialogElement>("ask-dialog");
  $("ask-title").textContent = o.title;
  $("ask-text").textContent = o.text ?? "";
  $("ask-text").classList.toggle("hidden", !o.text);
  const field = $("ask-field"), input = $<HTMLInputElement>("ask-input");
  field.classList.toggle("hidden", !o.input);
  if (o.input) {
    $("ask-label").textContent = o.input.label;
    input.value = o.input.value;
  }
  const actions = $("ask-actions");
  actions.innerHTML = o.buttons.map((b) => `<button type="button" data-id="${esc(b.id)}" class="${b.primary ? "primary" : b.danger ? "danger" : ""}">${esc(b.label)}</button>`).join("");
  return new Promise((resolve) => {
    let done = false;
    const finish = (id: string) => {
      if (done) return;
      done = true;
      dlg.close();
      resolve({ id, value: input.value.trim() });
    };
    actions.querySelectorAll<HTMLButtonElement>("button").forEach((b) => (b.onclick = () => finish(b.dataset.id!)));
    input.onkeydown = (e) => {
      if (e.key === "Enter") finish(o.buttons.find((b) => b.primary)?.id ?? "ok");
    };
    dlg.onclose = () => finish("cancel");
    dlg.showModal();
    if (o.input) {
      input.focus();
      input.select();
    } else actions.querySelector<HTMLButtonElement>("button.primary, button.danger")?.focus();
  });
}

const dirty = () => !!state.character && editor.changes !== state.savedAt;
function tabBlocked(tab: Tab): Key | null {
  if (tab === "characters" && !(state.project && state.game)) return "tab.needProject";
  if (tab === "studio" && !state.character) return "tab.needCharacter";
  return null;
}
function refreshTabs() {
  document.querySelectorAll<HTMLButtonElement>(".app-tabs button").forEach((b) => {
    const tab = b.dataset.tab as Tab;
    const why = tabBlocked(tab);
    b.setAttribute("aria-disabled", String(!!why));
    b.title = why ? t(why) : t(`tab.${tab}` as Key);
    b.setAttribute("aria-current", String(tab === "projects"));
    b.setAttribute("aria-label", t(`tab.${tab}` as Key));
  });
}
function go(tab: Tab) {
  state.tab = tab;
  const views: Record<Tab, string> = { projects: "view-projects", new: "view-new", characters: "view-characters", studio: "view-edit" };
  for (const [k, id] of Object.entries(views)) $(id).classList.toggle("hidden", k !== tab);
  document.body.classList.toggle("editing", tab === "studio");
  document.body.classList.toggle("project-open", tab === "characters" || tab === "studio");
  $("proj-menu").classList.add("hidden");
  if (tab === "projects") renderProjects();
  if (tab === "characters") renderCharacters();
  if (tab === "studio") {
    renderStudioHead();
    requestAnimationFrame(() => editor.view.layout());
  }
  refreshTabs();
}
async function confirmLeave(): Promise<boolean> {
  if (!dirty()) return true;
  const ch = state.character!;
  const r = await ask({
    title: t("leave.title", { name: ch.name }),
    text: t("leave.text", { project: state.project?.name ?? "" }),
    buttons: [
      { id: "cancel", label: t("leave.cancel") },
      { id: "discard", label: t("leave.discard") },
      { id: "save", label: t("leave.save"), primary: true },
    ],
  });
  if (r.id === "save") return saveCharacter();
  if (r.id === "discard") {
    await discardCharacter();
    return true;
  }
  return false;
}
async function requestTab(tab: Tab) {
  const why = tabBlocked(tab);
  if (why) {
    toast(t(why));
    return;
  }
  if (tab === state.tab) return;
  if (state.tab === "studio" && !(await confirmLeave())) return;
  go(tab);
}

function regionOf(id?: string | null) {
  const r: Record<string, Key> = { E: "game.regionUsa", P: "game.regionEurope", J: "game.regionJapan", K: "game.regionKorea" };
  return id && r[id[3]] ? t(r[id[3]]) : "";
}
function rememberPortraits(g: GameInfo) {
  try {
    const map: Record<string, string> = JSON.parse(localStorage.getItem("mkss.portraits") ?? "{}");
    for (const c of g.characters) if (c.portrait?.startsWith("data:image/png;base64,")) map[c.code] = c.portrait;
    localStorage.setItem("mkss.portraits", JSON.stringify(map));
  } catch {}
}
function portraitOf(code: string): string | null {
  const live = state.game?.characters.find((c) => c.code === code)?.portrait;
  if (live) return live;
  try {
    return (JSON.parse(localStorage.getItem("mkss.portraits") ?? "{}") as Record<string, string>)[code] ?? null;
  } catch {
    return null;
  }
}
const samePath = (a?: string | null, b?: string | null) => !!a && !!b && a.replace(/\\/g, "/").toLowerCase() === b.replace(/\\/g, "/").toLowerCase();
async function openGameAt(path: string, isImage: boolean): Promise<boolean> {
  if (state.game && samePath(state.gamePath, path)) return true;
  discovery.invalidate();
  const g = await run(t(isImage ? "game.extracting" : "game.opening"), () => invoke<GameInfo>("open_game", { path }));
  if (!g) return false;
  if (isImage) discovery.remember(path);
  state.game = g;
  state.gamePath = path;
  state.character = null;
  rememberPortraits(g);
  return true;
}
function gameLabel(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  while (parts.length > 1 && /^(files|data|disc)$/i.test(parts[parts.length - 1])) parts.pop();
  return parts.pop() ?? path;
}
function gameRef(path: string) {
  return { game: gameLabel(path), gamePath: path, gameId: state.game?.id ?? "" };
}
async function ensureGame(meta: Meta): Promise<boolean> {
  if (state.game && (!meta.gamePath || samePath(state.gamePath, meta.gamePath))) {
    if (!meta.gamePath && state.gamePath) await linkGame(meta.id, state.gamePath);
    return true;
  }
  const path = meta.gamePath || state.gamePath || lastGame();
  if (path && (await openGameAt(path, /\.(iso|wbfs|rvz|wia|ciso|gcz)$/i.test(path)))) {
    if (!meta.gamePath) await linkGame(meta.id, path);
    return true;
  }
  showNew("attach", meta);
  return false;
}
async function linkGame(id: string, path: string) {
  const m = await run(null, () => invoke<Meta>("project_set_game", { id, ...gameRef(path) }));
  if (m) upsert(m);
}

function upsert(m: Meta) {
  const i = state.projects.findIndex((p) => p.id === m.id);
  if (i >= 0) state.projects[i] = m;
  else state.projects.unshift(m);
  state.projects.sort((a, b) => b.updated - a.updated);
  if (state.project?.id === m.id) state.project = m;
}
async function loadProjects() {
  state.projects = (await run(null, () => invoke<Meta[]>("projects_list"))) ?? [];
}
const PORTRAIT_FALLBACK = '<svg viewBox="0 0 64 64" aria-hidden="true"><circle cx="32" cy="22" r="12"/><path d="M10 58c0-15 9-23 22-23s22 8 22 23Z"/></svg>';
function face(code: string, name: string, cls: string) {
  const p = portraitOf(code);
  return `<span class="${cls}" title="${esc(name)}">${p?.startsWith("data:image/png;base64,") ? `<img src="${esc(p)}" alt="" width="64" height="64" draggable="false">` : PORTRAIT_FALLBACK}</span>`;
}
function fallbackBroken(root: HTMLElement) {
  root.querySelectorAll<HTMLImageElement>("img").forEach((img) => {
    const fallback = () => { if (img.parentElement) img.parentElement.innerHTML = PORTRAIT_FALLBACK; };
    img.addEventListener("error", fallback, { once: true });
    if (img.complete && img.naturalWidth === 0) fallback();
  });
}
function renderProjects() {
  const list = $("proj-list");
  const empty = !state.projects.length;
  $("proj-empty").classList.toggle("hidden", !empty);
  list.classList.toggle("hidden", empty);
  $("btn-new-project").classList.toggle("hidden", empty);
  list.innerHTML = state.projects
    .map((p) => {
      const n = p.characters.length;
      const faces = p.characters.slice(0, 7).map((c) => face(c.code, c.name, "proj-face")).join("") + (n > 7 ? `<span class="proj-more">+${n - 7}</span>` : "");
      const game = p.gameId ? t("projects.game", { game: `${p.gameId}${regionOf(p.gameId) ? ` · ${regionOf(p.gameId)}` : ""}` }) : p.game ? t("projects.game", { game: p.game }) : t("projects.noGame");
      const current = state.project?.id === p.id;
      return `<li class="proj-card${current ? " current" : ""}" data-id="${esc(p.id)}">
        <div class="proj-main">
          <h2 class="proj-name">${esc(p.name)}</h2>
          <div class="proj-faces">${faces || `<span class="muted small">${esc(t("projects.charsNone"))}</span>`}</div>
          <p class="proj-meta">${n ? esc(t("projects.chars", { n })) + " · " : ""}${esc(t("projects.modified", { time: fmtTime(p.updated) }))}</p>
          <p class="proj-game" title="${esc(p.gamePath || p.game)}">${esc(game)}</p>
        </div>
        <div class="proj-actions">
          <button type="button" class="proj-open" data-act="open">${esc(t(current ? "projects.current" : "projects.open"))}</button>
          <button type="button" class="icon-btn proj-menu-btn" data-act="menu" aria-haspopup="menu" title="${esc(t("projects.more"))}" aria-label="${esc(t("projects.more"))}"><svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="5" cy="12" r="2" fill="currentColor"/><circle cx="12" cy="12" r="2" fill="currentColor"/><circle cx="19" cy="12" r="2" fill="currentColor"/></svg></button>
        </div>
      </li>`;
    })
    .join("");
  fallbackBroken(list);
}
let menuFor: string | null = null;
function openMenu(id: string, anchor: HTMLElement) {
  const menu = $("proj-menu");
  if (!menu.classList.contains("hidden") && menuFor === id) {
    menu.classList.add("hidden");
    return;
  }
  menuFor = id;
  const p = state.projects.find((x) => x.id === id);
  (menu.querySelector("[data-act=export]") as HTMLButtonElement).disabled = !p?.characters.length;
  menu.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const host = $("view-projects").getBoundingClientRect();
  const top = Math.min(r.bottom + 4, window.innerHeight - menu.offsetHeight - 8);
  menu.style.top = `${top - host.top}px`;
  menu.style.left = `${Math.max(8, r.right - menu.offsetWidth) - host.left}px`;
  menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
}
async function projectAction(id: string, act: string) {
  $("proj-menu").classList.add("hidden");
  const p = state.projects.find((x) => x.id === id);
  if (!p) return;
  if (act === "open") await openProject(id);
  else if (act === "rename") {
    const r = await ask({ title: t("projects.renameTitle"), input: { label: t("projects.nameLabel"), value: p.name }, buttons: [{ id: "cancel", label: t("leave.cancel") }, { id: "ok", label: t("projects.renameOk"), primary: true }] });
    if (r.id === "ok" && r.value) await renameProject(id, r.value);
  } else if (act === "duplicate") {
    const r = await ask({ title: t("projects.duplicateTitle"), input: { label: t("projects.nameLabel"), value: t("projects.copyName", { name: p.name }) }, buttons: [{ id: "cancel", label: t("leave.cancel") }, { id: "ok", label: t("projects.duplicateOk"), primary: true }] });
    if (r.id !== "ok" || !r.value) return;
    const m = await run(t("busy.wait"), () => invoke<Meta>("project_duplicate", { id, name: r.value }));
    if (m) {
      upsert(m);
      renderProjects();
      toast(t("projects.duplicated", { name: m.name }), "ok");
    }
  } else if (act === "delete") {
    const r = await ask({ title: t("projects.deleteTitle"), text: t("projects.deleteText", { name: p.name }), buttons: [{ id: "cancel", label: t("leave.cancel") }, { id: "delete", label: t("projects.deleteBtn"), danger: true }] });
    if (r.id !== "delete") return;
    const ok = await run(t("busy.wait"), () => invoke("project_delete", { id }).then(() => true));
    if (!ok) return;
    state.projects = state.projects.filter((x) => x.id !== id);
    if (state.project?.id === id) {
      state.project = null;
      state.character = null;
    }
    renderProjects();
    refreshTabs();
    toast(t("projects.deleted"), "ok");
  } else if (act === "copy") {
    const path = await save({ title: t("projects.saveCopyTitle"), defaultPath: `${p.name.replace(/[\\/:*?"<>|]/g, " ").trim() || "project"}.mkss`, filters: [{ name: t("proj.filter"), extensions: ["mkss"] }] });
    if (!path) return;
    const ok = await run(t("busy.wait"), () => invoke<number>("project_export_file", { id, path }).then(() => true));
    if (ok) toast(t("projects.copySaved", { file: baseName(path) }), "ok");
  } else if (act === "export") {
    if (await openProject(id)) await exportMod();
  }
}
async function renameProject(id: string, name: string) {
  const m = await run(null, () => invoke<Meta>("project_rename", { id, name }));
  if (m) upsert(m);
  if (state.tab === "projects") renderProjects();
  renderCharactersHead();
  renderStudioHead();
}
async function openProject(id: string): Promise<boolean> {
  const meta = state.projects.find((p) => p.id === id);
  if (!meta) return false;
  if (state.project?.id !== id) {
    if (dirty() && !(await confirmLeave())) return false;
    state.character = null;
    state.lastExport = null;
  }
  state.project = meta;
  refreshTabs();
  if (!(await ensureGame(meta))) return false;
  go("characters");
  return true;
}
async function importProjectFile() {
  const path = await open({ title: t("projects.openTitle"), filters: [{ name: t("proj.filter"), extensions: ["mkss"] }] });
  if (typeof path !== "string") return;
  await openMkssFile(path);
}
async function openMkssFile(path: string) {
  const r = await run(t("busy.wait"), () => invoke<{ meta: Meta; existing: boolean }>("project_import", { path, name: baseName(path).replace(/\.mkss$/i, "") }));
  if (!r) return;
  upsert(r.meta);
  renderProjects();
  toast(t(r.existing ? "projects.reopened" : "projects.imported", { name: r.meta.name }), "ok");
  await openProject(r.meta.id);
}
async function takeOpenRequests() {
  let paths: string[] = [];
  try {
    paths = await invoke<string[]>("take_open_requests");
  } catch {
    return;
  }
  for (const p of paths) await openMkssFile(p);
}
function defaultProjectName() {
  for (let n = 1; ; n++) {
    const name = t("projects.defaultName", { n });
    if (!state.projects.some((p) => p.name === name)) return name;
  }
}
function showNew(mode: "new" | "attach", meta?: Meta) {
  state.newMode = mode;
  state.attachId = meta?.id ?? null;
  $("new-name-card").classList.toggle("hidden", mode === "attach");
  $("new-title").textContent = mode === "attach" ? t("new.attachTitle", { name: meta?.name ?? "" }) : t("new.title");
  $("new-lead").textContent = t(mode === "attach" ? "new.attachLead" : "new.lead");
  if (mode === "new") ($("new-name") as HTMLInputElement).value = defaultProjectName();
  go("new");
}
async function chooseGame(path: string, isImage: boolean): Promise<boolean> {
  if (!(await openGameAt(path, isImage))) return false;
  if (state.newMode === "attach" && state.attachId) {
    const m = await run(null, () => invoke<Meta>("project_set_game", { id: state.attachId, ...gameRef(path) }));
    if (!m) return false;
    upsert(m);
    state.project = m;
  } else {
    const name = ($("new-name") as HTMLInputElement).value.trim() || defaultProjectName();
    const m = await run(t("busy.wait"), () => invoke<Meta>("project_create", { name, ...gameRef(path) }));
    if (!m) return false;
    upsert(m);
    state.project = m;
    state.character = null;
    state.lastExport = null;
  }
  go("characters");
  return true;
}
async function pickFolder() {
  const path = await open({ directory: true, title: t("game.pickFolder") });
  if (typeof path === "string") await chooseGame(path, false);
}
async function pickImage() {
  const path = await open({ title: t("game.pickImage"), filters: [{ name: t("game.imageFilter"), extensions: ["iso", "wbfs", "rvz", "wia"] }] });
  if (typeof path !== "string") return;
  discovery.choose(path);
}

const TAGS = ["body", "eyes", "bike", "low"] as const;
function renderCharactersHead() {
  const p = state.project;
  const name = $<HTMLInputElement>("proj-name");
  if (document.activeElement !== name) name.value = p?.name ?? "";
  const n = p?.characters.length ?? 0;
  const btn = $<HTMLButtonElement>("btn-export-mod");
  btn.disabled = !n || state.exporting;
  $("export-why").textContent = n ? t("chars.exportCount", { n }) : t("chars.exportNone");
  $("export-why").classList.toggle("warn-text", !n);
  btn.title = n ? "" : t("chars.exportNone");
}
function renderCharacters() {
  renderCharactersHead();
  const grid = $("char-grid");
  const p = state.project;
  grid.innerHTML = (state.game?.characters ?? [])
    .map((c) => {
      const e = p?.characters.find((x) => x.code === c.code);
      const tags = e ? TAGS.filter((tg) => e.edits?.includes(tg)).map((tg) => t(`tag.${tg}` as Key)).join(" · ") : "";
      const status = e ? `<span class="char-status"><i class="b ed">${esc(t("chars.edited"))}</i>${tags ? `<small>${esc(tags)}</small>` : ""}</span>` : `<span class="char-status muted">${esc(t("chars.original"))}</span>`;
      return `<div class="char-cell${e ? " edited" : ""}${state.character?.code === c.code ? " open" : ""}">
        <button type="button" class="char${state.character?.code === c.code ? " on" : ""}" data-code="${esc(c.code)}" title="${esc(t("chars.editTitle", { name: c.name }))}">
          <span class="char-portrait" aria-hidden="true">${c.portrait?.startsWith("data:image/png;base64,") ? `<img src="${esc(c.portrait)}" alt="" width="64" height="64" draggable="false">` : PORTRAIT_FALLBACK}</span>
          <span class="char-name">${esc(c.name)}</span>${status}</button>
        ${e ? `<button type="button" class="icon-btn char-revert" data-code="${esc(c.code)}" title="${esc(t("chars.revert"))}" aria-label="${esc(t("chars.revert"))} · ${esc(c.name)}"><svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M12.5 8c-2.6 0-5 1-6.8 2.6L3 8v8h8l-3-3c1.3-1.2 2.9-1.9 4.5-1.9 3 0 5.6 2 6.5 4.8l2.1-.7C20 10.9 16.6 8 12.5 8Z"/></svg></button>` : ""}
      </div>`;
    })
    .join("");
  fallbackBroken(grid);
  grid.querySelectorAll<HTMLButtonElement>("button.char").forEach((b) => b.addEventListener("click", () => void openCharacter(b.dataset.code!)));
  grid.querySelectorAll<HTMLButtonElement>("button.char-revert").forEach((b) => b.addEventListener("click", () => void revertCharacter(b.dataset.code!)));
  const d = state.lastExport;
  $("export-done").classList.toggle("hidden", !d);
  if (d) {
    $("export-done-text").textContent = t("exp.doneMod", { chars: d.characters, files: d.files, size: fmtBytes(d.bytes) });
    $("export-done-path").textContent = d.path;
  }
}
async function revertCharacter(code: string) {
  const p = state.project;
  const e = p?.characters.find((c) => c.code === code);
  if (!p || !e) return;
  const r = await ask({ title: t("chars.revertTitle", { name: e.name }), text: t("chars.revertText", { name: e.name }), buttons: [{ id: "cancel", label: t("leave.cancel") }, { id: "revert", label: t("chars.revertBtn"), danger: true }] });
  if (r.id !== "revert") return;
  const m = await run(t("busy.wait"), () => invoke<Meta>("project_char_write", { id: p.id, code, name: e.name, project: null, edits: [] }));
  if (!m) return;
  await invoke("autosave_clear", { id: p.id, code }).catch(() => {});
  upsert(m);
  if (state.character?.code === code) state.character = null;
  renderCharacters();
  refreshTabs();
  toast(t("chars.reverted", { name: e.name }), "ok");
}
async function loadCharacter(code: string): Promise<{ view: CharacterView; mismatched: number }> {
  const p = state.project!;
  const view = await invoke<CharacterView>("load_character", { code });
  await editor.load(view);
  const saved = await invoke<Project | null>("project_char_read", { id: p.id, code });
  let mismatched = 0;
  if (saved) {
    parseManifest(saved);
    mismatched = (await applyProject(saved, editor.docs)).mismatched.length;
    editor.applied();
  }
  return { view, mismatched };
}
async function openCharacter(code: string): Promise<boolean> {
  if (!state.project || !state.game) return false;
  if (state.character?.code === code) {
    go("studio");
    return true;
  }
  if (dirty() && !(await confirmLeave())) return false;
  const name = state.game.characters.find((c) => c.code === code)?.name ?? code;
  const res = await run(t("char.loading", { name }), () => loadCharacter(code));
  if (!res) return false;
  state.character = res.view;
  state.savedAt = state.autosavedAt = editor.changes;
  if (res.mismatched) toast(t("proj.hashMismatch", { n: res.mismatched }));
  go("studio");
  void offerRecovery();
  return true;
}

function renderStudioHead() {
  const ch = state.character;
  $("crumb-project").textContent = state.project?.name ?? "";
  $("crumb-char").textContent = ch?.name ?? "";
  const unsaved = dirty();
  const s = $("save-state");
  s.textContent = ch ? t(unsaved ? "studio.unsaved" : "studio.saved") : "";
  s.classList.toggle("unsaved", unsaved);
  s.classList.toggle("ok", !unsaved);
  $<HTMLButtonElement>("btn-proj-save").disabled = !unsaved;
  $("edit-char").textContent = ch?.name ?? "";
  $("edit-count").textContent = ch ? t("edit.count", { n: editor.editedDocs().length }) : "";
}
async function saveCharacter(): Promise<boolean> {
  const ch = state.character, p = state.project;
  if (!ch || !p) return false;
  const changes = editor.changes;
  const m = await run(t("busy.wait"), async () => {
    const project = await serialize(editor.docs.values(), { code: ch.code, name: ch.name }, VERSION);
    const meta = await invoke<Meta>("project_char_write", { id: p.id, code: ch.code, name: ch.name, project, edits: editor.editTags() });
    await invoke("autosave_clear", { id: p.id, code: ch.code }).catch(() => {});
    return meta;
  });
  if (!m) return false;
  upsert(m);
  state.savedAt = state.autosavedAt = changes;
  renderStudioHead();
  toast(t("studio.savedToast", { name: p.name }), "ok");
  return true;
}
async function discardCharacter() {
  const ch = state.character, p = state.project;
  if (ch && p) await invoke("autosave_clear", { id: p.id, code: ch.code }).catch(() => {});
  state.character = null;
  $("recover-banner").classList.add("hidden");
  refreshTabs();
}
async function done() {
  if (!(await confirmLeave())) return;
  go("characters");
}
async function exportOriginal() {
  const d = editor.current;
  if (!d) return;
  const path = await save({ title: t("edit.saveOriginal"), defaultPath: `${d.name}.png`, filters: [{ name: "PNG", extensions: ["png"] }] });
  if (!path) return;
  const ok = await run(null, () => invoke("export_original", { name: d.name, path }).then(() => true));
  if (ok) toast(t("edit.saved", { file: baseName(path) }), "ok");
}
async function autosave() {
  const ch = state.character, p = state.project;
  if (!ch || !p || !dirty() || editor.changes === state.autosavedAt) return;
  const changes = editor.changes;
  try {
    await invoke("autosave_write", { id: p.id, code: ch.code, project: await serialize(editor.docs.values(), { code: ch.code, name: ch.name }, VERSION) });
    state.autosavedAt = changes;
  } catch {}
}
async function offerRecovery() {
  const ch = state.character, p = state.project;
  if (!ch || !p) return;
  let r: Project | null = null;
  try {
    r = await invoke<Project | null>("autosave_read", { id: p.id, code: ch.code });
  } catch {
    return;
  }
  if (!r) return;
  let m;
  try {
    m = parseManifest(r);
  } catch {
    return;
  }
  if (m.character.code !== ch.code || !m.textures.length) return;
  $("recover-text").textContent = t("proj.recover", { time: new Date(m.savedAt).toLocaleString(locale()), name: m.character.name });
  $("recover-banner").classList.remove("hidden");
  const rec = r;
  $("btn-recover").onclick = async () => {
    $("recover-banner").classList.add("hidden");
    const res = await run(t("busy.wait"), () => applyProject(rec, editor.docs));
    if (!res) return;
    editor.applied();
    renderStudioHead();
    toast(t("proj.recovered"), "ok");
  };
  $("btn-recover-discard").onclick = async () => {
    $("recover-banner").classList.add("hidden");
    await invoke("autosave_clear", { id: p.id, code: ch.code }).catch(() => {});
  };
}

async function exportMod() {
  const p = state.project;
  if (!p || !p.characters.length || state.exporting) return;
  if (!(await ensureGame(p))) return;
  let defaultPath: string;
  try {
    defaultPath = await invoke<string>("pack_file_name", { name: p.name });
  } catch (e) {
    toast(errText(e));
    return;
  }
  const out = await save({ title: t("exp.saveTitle"), defaultPath, filters: [{ name: t("exp.zipFilter"), extensions: ["zip"] }] });
  if (!out) return;
  state.exporting = true;
  state.lastExport = null;
  $("toast").classList.add("hidden");
  if (state.tab !== "characters") go("characters");
  renderCharacters();
  $("export-progress").classList.remove("hidden");
  const bar = $<HTMLProgressElement>("build-bar");
  const text = $("build-text");
  bar.max = p.characters.length + 1;
  bar.value = 0;
  let unlisten: (() => void) | null = null;
  try {
    await invoke("pack_begin");
    let staged = 0;
    for (const [i, c] of p.characters.entries()) {
      text.textContent = t("exp.preparing", { name: c.name, n: i + 1, total: p.characters.length });
      bar.value = i;
      const saved = await invoke<Project | null>("project_char_read", { id: p.id, code: c.code });
      if (!saved) continue;
      const view = await invoke<CharacterView>("load_character", { code: c.code });
      await editor.load(view);
      await editor.modelReady;
      await applyProject(saved, editor.docs);
      editor.applied();
      await editor.commitAll();
      if ((await invoke<number>("pack_stage")) > 0) staged++;
    }
    state.character = null;
    unlisten = await listen<Progress>("mkss-progress", (ev) => {
      const pr = ev.payload;
      bar.max = pr.total;
      bar.value = pr.done;
      text.textContent = pr.done >= pr.total ? t("exp.compressing") : t("exp.building", { file: pr.file });
    });
    const res = await invoke<ExportResult>("build_pack", { name: p.name, out });
    state.lastExport = { ...res, characters: staged };
  } catch (e) {
    toast(errText(e instanceof Error ? e.message : e));
  } finally {
    unlisten?.();
    state.character = null;
    state.exporting = false;
    $("export-progress").classList.add("hidden");
    refreshTabs();
    if (state.tab === "characters") renderCharacters();
  }
}

async function beforeClose(): Promise<boolean> {
  if (state.closeApproved || !dirty()) return true;
  if (state.tab !== "studio") go("studio");
  const ok = await confirmLeave();
  state.closeApproved = ok;
  return ok;
}

async function init() {
  initTheme();
  applyStatic();
  $("ver").textContent = `v${VERSION}`;
  initWindowChrome(`v${VERSION}`, beforeClose);
  try {
    void getCurrentWindow()
      .onCloseRequested(async (e) => {
        if (!(await beforeClose())) e.preventDefault();
      })
      .catch(() => {});
  } catch {}
  mountLangPicker($("lang-picker"));
  initTutorial();
  $("btn-discord").addEventListener("click", () => void openUrl("https://discord.gg/93vfR7597T").catch((e) => toast(errText(e))));
  $("btn-donate").addEventListener("click", () => void openUrl("https://ko-fi.com/m1t5").catch((e) => toast(errText(e))));
  const settings = $<HTMLDialogElement>("settings-dialog");
  $("btn-settings").addEventListener("click", () => settings.showModal());
  $("btn-settings-close").addEventListener("click", () => settings.close());
  settings.addEventListener("click", (e) => {
    if (e.target !== settings) return;
    const r = settings.getBoundingClientRect();
    if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) settings.close();
  });
  settings.addEventListener("close", () => {
    settings.querySelector(".lang-menu")?.classList.add("hidden");
    $("btn-settings").focus();
  });
  editor = new Editor();
  discovery = new GameDiscovery((path) => chooseGame(path, true));
  editor.toast = toast;
  editor.onEdited = renderStudioHead;
  editor.onExportPng = () => void exportOriginal();
  editor.onSaveShortcut = () => void saveCharacter();
  applyStatic();
  onLangChange(() => {
    refreshTabs();
    if (state.tab === "projects") renderProjects();
    if (state.tab === "characters") renderCharacters();
    if (state.tab === "new") {
      $("new-title").textContent = state.newMode === "attach" ? t("new.attachTitle", { name: state.projects.find((p) => p.id === state.attachId)?.name ?? "" }) : t("new.title");
      $("new-lead").textContent = t(state.newMode === "attach" ? "new.attachLead" : "new.lead");
    }
    renderStudioHead();
    editor.relabel();
  });
  document.querySelectorAll<HTMLButtonElement>(".app-tabs button").forEach((b) => b.addEventListener("click", () => void requestTab(b.dataset.tab as Tab)));
  $("btn-new-project").addEventListener("click", () => showNew("new"));
  $("btn-new-project-empty").addEventListener("click", () => showNew("new"));
  $("btn-import-project").addEventListener("click", () => void importProjectFile());
  $("btn-new-back").addEventListener("click", () => go("projects"));
  $("proj-list").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-act]");
    const id = (e.target as HTMLElement).closest<HTMLElement>(".proj-card")?.dataset.id;
    if (!b || !id) return;
    e.stopPropagation();
    if (b.dataset.act === "menu") openMenu(id, b);
    else void projectAction(id, b.dataset.act!);
  });
  $("proj-list").addEventListener("dblclick", (e) => {
    const id = (e.target as HTMLElement).closest<HTMLElement>(".proj-card")?.dataset.id;
    if (id && !(e.target as HTMLElement).closest("button")) void openProject(id);
  });
  $("proj-menu").addEventListener("click", (e) => {
    const b = (e.target as HTMLElement).closest<HTMLButtonElement>("button[data-act]");
    if (b && menuFor) void projectAction(menuFor, b.dataset.act!);
  });
  document.addEventListener("click", (e) => {
    if (!$("proj-menu").contains(e.target as Node)) $("proj-menu").classList.add("hidden");
  });
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") $("proj-menu").classList.add("hidden");
  });
  const nameInput = $<HTMLInputElement>("proj-name");
  nameInput.addEventListener("keydown", (e) => {
    if (e.key === "Enter") nameInput.blur();
    if (e.key === "Escape") {
      nameInput.value = state.project?.name ?? "";
      nameInput.blur();
    }
  });
  nameInput.addEventListener("change", () => {
    const v = nameInput.value.trim();
    if (state.project && v && v !== state.project.name) void renameProject(state.project.id, v);
    else nameInput.value = state.project?.name ?? "";
  });
  $("btn-open-folder").addEventListener("click", () => void pickFolder());
  $("btn-open-image").addEventListener("click", () => void pickImage());
  $("btn-export-mod").addEventListener("click", () => void exportMod());
  $("btn-done").addEventListener("click", () => void done());
  $("crumb-project").addEventListener("click", () => void done());
  $("btn-proj-save").addEventListener("click", () => void saveCharacter());
  $("btn-show").addEventListener("click", () => state.lastExport && void revealItemInDir(state.lastExport.path).catch((e) => toast(errText(e))));
  window.setInterval(() => void autosave(), 20000);
  window.addEventListener("beforeunload", () => void autosave());
  (window as unknown as { __mkss: unknown }).__mkss = { editor, state, composite, autosave, saveCharacter, dirty };
  await loadProjects();
  go("projects");
  document.documentElement.dataset.ready = "1";
  void discovery.scan();
  try {
    void listen("mkss-open", () => void takeOpenRequests()).catch(() => {});
  } catch {}
  await takeOpenRequests();
}

void init();
