import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { applyStatic, errText, t, type Key } from "../i18n";
import { CanvasView, isTyping } from "./canvasview";
import { ColorPicker } from "./colorpicker";
import { BLENDS, History, activeLayer, baseEdited, composite, identityAdjust, isEdited, newDoc, touch, type Blend, type Doc, type Layer } from "./doc";
import { icon } from "./icons";
import { applyStroke, fillMask, lineStroke, maskBBox, maskFromRuns, maskOutline, palette, rectStroke, regionMask, replaceColor, hex, Stroke, type BrushOpts, type PaintMode, type RGBA } from "./paint";
import { decodeDataUrl, pngDataUrl } from "./png";
import { Preview3D, i32arr, u32arr, type EyeData, type LinkData, type ModelBundle, type ModelData, type SurfaceHit, type TexParts } from "./preview3d";
import { SurfaceGeo, TexelOwners, partMaskOf, stampSphere, wrapTexel } from "./surface";

export interface TextureView { name: string; width: number; height: number; format: string; images: number; files: number; menu: boolean; eye: boolean; preview: string; sha256: string }
export interface CharacterView { code: string; name: string; files: number; menu: boolean; textures: TextureView[] }

type Tool = "brush" | "eraser" | "fill" | "picker" | "line" | "rect" | "select" | "wand" | "move" | "hand";
const TOOLS: { id: Tool; key: string; icon: string }[] = [
  { id: "brush", key: "B", icon: "brush" },
  { id: "eraser", key: "E", icon: "eraser" },
  { id: "fill", key: "G", icon: "fill" },
  { id: "picker", key: "I", icon: "picker" },
  { id: "line", key: "L", icon: "line" },
  { id: "rect", key: "U", icon: "rect" },
  { id: "select", key: "M", icon: "select" },
  { id: "wand", key: "W", icon: "wand" },
  { id: "move", key: "V", icon: "move" },
  { id: "hand", key: "H", icon: "hand" },
];

const $ = <T extends HTMLElement = HTMLElement>(id: string) => document.getElementById(id) as T;
const esc = (s: string) => s.replace(/[&<>"']/g, (c) => `&#${c.charCodeAt(0)};`);

function syncRanges(root: ParentNode) {
  root.querySelectorAll<HTMLInputElement>('input[type="range"]').forEach((el) => {
    const min = el.min === "" ? 0 : Number(el.min), max = el.max === "" ? 100 : Number(el.max);
    const percent = max > min ? Math.max(0, Math.min(100, (Number(el.value) - min) * 100 / (max - min))) : 0;
    el.style.setProperty("--range-progress", `${percent}%`);
  });
}

interface PartRef { texture: string; index: number }
type Mode = "3d" | "2d";
const TOOLS_3D: Tool[] = ["brush", "eraser", "fill", "picker", "wand", "hand"];
type Vehicle = "kart" | "bike";
type Detail = "full" | "low";
interface Target3D { doc: Doc; layer: Layer; snapshot: Uint8ClampedArray; stroke: Stroke; mode: PaintMode }

export class Editor {
  docs = new Map<string, Doc>();
  character: CharacterView | null = null;
  current: Doc | null = null;
  history = new History();
  tool: Tool = "brush";
  opts = {
    brush: { size: 4, hardness: 0.8, opacity: 1, pixelPerfect: false } as BrushOpts,
    eraser: { size: 8, hardness: 0.8, opacity: 1, pixelPerfect: false } as BrushOpts,
    eraseOriginal: false,
    line: { size: 2, hardness: 1, opacity: 1, pixelPerfect: true } as BrushOpts,
    rect: { size: 1, hardness: 1, opacity: 1, pixelPerfect: true } as BrushOpts,
    rectFill: false,
    fillTolerance: 24,
    fillContiguous: true,
    fillOpacity: 1,
    wandTolerance: 24,
    wandContiguous: true,
    size3d: { brush: 18, eraser: 28 } as Record<"brush" | "eraser", number>,
    showMirror: localStorage.getItem("mkss.showMirror") !== "0",
  };
  mode: Mode = "3d";
  eyeMode = "all";
  blink = false;
  private blinkTimer = 0;
  private blinkFrame: string | null = null;
  private geos = new Map<number, SurfaceGeo>();
  private owners = new Map<string, TexelOwners>();
  private stroke3d: null | { targets: Map<string, Target3D>; last: [number, number]; color: RGBA; tool: Tool; mirror: Map<number, Set<number>> } = null;
  private dirtyDocs = new Set<Doc>();
  private derivedCache = new Map<string, { key: string; px: Uint8ClampedArray }>();
  private unusedCache = new Map<string, Uint8Array | null>();
  private mirrorHintShown = false;
  private modelIndex = 0;
  vehicle: Vehicle = "kart";
  detail: Detail = "full";
  private outfitViews = new Map<string, ModelData>();
  private pendingClick: null | { hit: SurfaceHit; right: boolean } = null;
  lastStroke3d: Record<string, number> = {};
  view: CanvasView;
  color: ColorPicker;
  preview: Preview3D;
  bundle: ModelBundle | null = null;
  modelError: string | null = null;
  focus: PartRef | null = null;
  hover: PartRef | null = null;
  private maskCache = new Map<string, Uint8Array>();
  private outlineCache = new Map<string, number[]>();
  private gesture: null | ((x: number, y: number, phase: "move" | "up", e: PointerEvent) => void) = null;
  private frame = 0;
  private thumbs = new Map<string, string>();
  private thumbTimer = 0;
  replaceFrom: RGBA | null = null;
  private pickingReplace = false;
  onEdited: () => void = () => {};
  toast: (msg: string, kind?: "error" | "ok") => void = () => {};
  changes = 0;

  constructor() {
    this.view = new CanvasView($("ed-canvas") as HTMLCanvasElement, {
      down: (x, y, e) => this.down(x, y, e),
      move: (x, y, e) => this.gesture?.(x, y, "move", e),
      up: (x, y, e) => {
        const g = this.gesture;
        this.gesture = null;
        g?.(x, y, "up", e);
      },
      hover: (x, y) => this.hoverAt(x, y),
    });
    this.view.onZoom = (z) => {
      $("zoom-label").textContent = fmtZoom(z);
      $("btn-fit").setAttribute("aria-pressed", String(this.view.fit));
    };
    this.color = new ColorPicker($("color-picker"));
    this.color.onChange = () => this.renderStatusColor();
    this.preview = new Preview3D($("p3d-canvas") as HTMLCanvasElement);
    this.preview.source = (name) => {
      const d = this.docs.get(name);
      return d ? { w: d.w, h: d.h, data: this.pixelsOf(d) } : null;
    };
    this.preview.textureFor = (name) => this.texParts(name)?.aliasOf ?? name;
    this.preview.onPick = (p) => {
      if (!p) return;
      const cur = this.current;
      const target = cur && (this.texParts(cur.name)?.aliasOf ?? cur.name) === p.texture ? cur.name : p.texture;
      if (target !== cur?.name) this.select(target);
      this.selectPart({ texture: target, index: p.part });
    };
    this.preview.onHover = (p) => {
      const cur = this.current;
      this.hover = p && cur && (this.texParts(cur.name)?.aliasOf ?? cur.name) === p.texture ? { texture: cur.name, index: p.part } : null;
      this.renderHover();
    };
    this.preview.onNavStart = () => this.cancelStroke3d();
    if (!this.preview.ok) this.p3dMessage(t("p3d.failed", { why: t("p3d.noWebgl") }));
    this.history.onChange = (d) => this.changed(d, true);
    this.bind3d();
    this.buildStatic();
    $("view-edit").addEventListener("input", (e) => {
      if (e.target instanceof HTMLInputElement && e.target.type === "range") syncRanges($("view-edit"));
    });
    syncRanges($("view-edit"));
    this.bindKeys();
    this.syncTheme();
    new MutationObserver(() => this.syncTheme()).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  }

  private buildStatic() {
    $("ed-tools").innerHTML = TOOLS.map((x) => `<button type="button" class="tool" data-tool="${x.id}" data-i18n-title="tool.${x.id}" aria-pressed="false">${icon(x.icon)}</button>`).join("");
    $("ed-tools").querySelectorAll<HTMLButtonElement>(".tool").forEach((b) => b.addEventListener("click", () => this.setTool(b.dataset.tool as Tool)));
    const ib = (id: string, ic: string) => {
      const el = $(id);
      el.insertAdjacentHTML("afterbegin", icon(ic));
    };
    for (const [id, ic] of [
      ["btn-zoom-out", "minus"], ["btn-zoom-in", "plus"], ["btn-fit", "fit"], ["btn-grid", "grid"], ["btn-outlines", "parts"], ["btn-undo", "undo"], ["btn-redo", "redo"],
      ["btn-proj-save", "save"], ["btn-help", "help"], ["btn-export-png", "image"], ["btn-p3d-rotate", "rotate"], ["btn-p3d-reset", "center"], ["btn-p3d-pixels", "pixels"],
      ["lay-add", "plus"], ["lay-dup", "copy"], ["lay-import", "image"], ["lay-recolor", "recolor"], ["lay-del", "trash"], ["btn-replace-color", "replace"], ["btn-eyes-all", "frames"], ["rc-pick", "picker"],
    ] as const)
      ib(id, ic);
    $("tex-switch").insertAdjacentHTML("beforeend", icon("caret", "ic caret"));
    $("lay-blend").innerHTML = BLENDS.map((b) => `<option value="${b}" data-i18n="blend.${b}"></option>`).join("");
    $("btn-zoom-in").addEventListener("click", () => this.view.setZoom(nextZoom(this.view.zoom, 1)));
    $("btn-zoom-out").addEventListener("click", () => this.view.setZoom(nextZoom(this.view.zoom, -1)));
    $("btn-fit").addEventListener("click", () => this.view.setFit());
    $("btn-grid").addEventListener("click", () => {
      this.view.grid = !this.view.grid;
      this.syncToggles();
    });
    $("btn-outlines").addEventListener("click", () => {
      this.view.outlines = !this.view.outlines;
      this.syncToggles();
    });
    $("btn-undo").addEventListener("click", () => this.undo());
    $("btn-redo").addEventListener("click", () => this.redo());
    $("btn-help").addEventListener("click", (e) => {
      e.stopPropagation();
      togglePop("help-pop", $("btn-help"));
    });
    $("tex-switch").addEventListener("click", (e) => {
      e.stopPropagation();
      this.renderSwitcher();
      togglePop("tex-pop", $("tex-switch"));
    });
    document.addEventListener("click", (e) => {
      for (const id of ["tex-pop", "help-pop"]) if (!$(id).contains(e.target as Node)) $(id).classList.add("hidden");
    });
    $("btn-p3d-rotate").addEventListener("click", () => {
      this.preview.autoRotate = !this.preview.autoRotate;
      this.syncToggles();
    });
    $("btn-p3d-reset").addEventListener("click", () => this.preview.resetView());
    $("btn-p3d-pixels").addEventListener("click", () => {
      this.preview.setNearest(!this.preview.nearest);
      this.syncToggles();
    });
    $("lay-add").addEventListener("click", () => this.addLayer());
    $("lay-dup").addEventListener("click", () => this.duplicateLayer());
    $("lay-del").addEventListener("click", () => this.deleteLayer());
    $("lay-import").addEventListener("click", () => void this.importImage());
    $("lay-recolor").addEventListener("click", () => this.addAdjustment());
    const opacity = $("lay-opacity") as HTMLInputElement;
    let snap: ReturnType<History["snapLayers"]> | null = null;
    let snapActive = 0;
    const begin = () => {
      if (this.current && !snap) {
        snap = this.history.snapLayers(this.current);
        snapActive = this.current.active;
      }
    };
    const end = () => {
      if (this.current && snap) this.history.layers(this.current, snap, snapActive);
      snap = null;
    };
    opacity.addEventListener("pointerdown", begin);
    opacity.addEventListener("keydown", begin);
    opacity.addEventListener("input", () => {
      begin();
      const d = this.current;
      if (!d) return;
      activeLayer(d).opacity = Number(opacity.value) / 100;
      $("lay-opacity-out").textContent = `${opacity.value}%`;
      this.changed(d);
    });
    opacity.addEventListener("change", () => {
      end();
      this.renderLayers();
    });
    $("lay-blend").addEventListener("change", () => {
      const d = this.current;
      if (!d) return;
      begin();
      activeLayer(d).blend = ($("lay-blend") as HTMLSelectElement).value as Blend;
      end();
      this.changed(d);
      this.renderLayers();
    });
    for (const id of ["sl-hue", "sl-sat", "sl-bri"]) {
      const el = $(id) as HTMLInputElement;
      el.addEventListener("pointerdown", begin);
      el.addEventListener("keydown", begin);
      el.addEventListener("input", () => {
        begin();
        this.readAdjust();
      });
      el.addEventListener("change", end);
    }
    $("adj-colorize").addEventListener("change", () => {
      begin();
      this.readAdjust();
      end();
    });
    $("parts-clear").addEventListener("click", () => this.setSelection(null));
    $("btn-replace-color").addEventListener("click", () => this.toggleReplace());
    $("rc-pick").addEventListener("click", () => {
      this.pickingReplace = true;
      this.toast(t("rc.pickHint"), "ok");
    });
    $("rc-apply").addEventListener("click", () => this.applyReplace());
    $("rc-cancel").addEventListener("click", () => $("replace-pop").classList.add("hidden"));
    $("rc-tol").addEventListener("input", () => ($("rc-tol-out").textContent = ($("rc-tol") as HTMLInputElement).value));
    $("btn-eyes-all").addEventListener("click", () => void this.applyToAllEyes());
    $("btn-export-png").addEventListener("click", () => this.onExportPng());
    $("mode-3d").addEventListener("click", () => this.setMode("3d"));
    $("mode-2d").addEventListener("click", () => this.setMode("2d"));
    $("eye-frame").addEventListener("change", () => {
      this.eyeMode = ($("eye-frame") as HTMLSelectElement).value;
      this.renderEyePanel();
      this.refreshEyePreview();
    });
    $("eye-blink").addEventListener("change", () => this.setBlink(($("eye-blink") as HTMLInputElement).checked));
    $("parts-toggle").addEventListener("click", () => {
      const p = document.querySelector(".parts-panel")!;
      const collapsed = p.classList.toggle("collapsed");
      $("parts-toggle").setAttribute("aria-expanded", String(!collapsed));
    });
    for (const v of ["kart", "bike"] as Vehicle[]) $(`veh-${v}`).addEventListener("click", () => this.setOutfit(v, this.detail));
    for (const d of ["full", "low"] as Detail[]) $(`det-${d}`).addEventListener("click", () => this.setOutfit(this.vehicle, d));
    this.setTool("brush");
    this.syncToggles();
  }
  onExportPng: () => void = () => {};

  private syncTheme() {
    const dark = document.documentElement.dataset.theme === "dark";
    this.preview.setBackground(dark ? "#141417" : "#e9e9e3");
    this.view.request();
  }
  private syncToggles() {
    $("btn-grid").setAttribute("aria-pressed", String(this.view.grid));
    $("btn-outlines").setAttribute("aria-pressed", String(this.view.outlines));
    $("btn-p3d-rotate").setAttribute("aria-pressed", String(this.preview.autoRotate));
    $("btn-p3d-pixels").setAttribute("aria-pressed", String(this.preview.nearest));
    this.view.request();
  }

  async load(ch: CharacterView) {
    this.character = ch;
    this.setBlink(false);
    this.eyeMode = "all";
    this.geos.clear();
    this.owners.clear();
    this.derivedCache.clear();
    this.unusedCache.clear();
    this.modelIndex = 0;
    this.vehicle = "kart";
    this.detail = "full";
    this.outfitViews.clear();
    this.pendingClick = null;
    this.docs.clear();
    this.history.clear();
    this.maskCache.clear();
    this.outlineCache.clear();
    this.rgbCache.clear();
    this.thumbs.clear();
    this.bundle = null;
    this.modelError = null;
    this.focus = this.hover = null;
    this.preview.clear();
    for (const x of ch.textures) {
      const img = await decodeDataUrl(x.preview);
      const d = newDoc(x.name, img.width, img.height, img.data, x.sha256);
      for (const l of d.layers) l.name = l.base ? t("lay.base") : t("lay.layer", { n: 1 });
      this.docs.set(x.name, d);
    }
    this.select(ch.textures[0]?.name ?? "");
    this.setMode(this.preview.ok ? "3d" : "2d");
    this.modelReady = this.loadModel(ch.code);
  }
  modelReady: Promise<void> = Promise.resolve();

  applied() {
    for (const d of this.docs.values()) this.changed(d, true);
    this.history.clear();
    this.renderUndo();
    this.select(this.current?.name ?? "");
  }

  editTags(): string[] {
    const tags = new Set<string>();
    const bike = this.hasBikeOutfit() ? this.outfitTextures("bike") : null;
    const kart = bike ? this.outfitTextures("kart") : null;
    for (const d of this.docs.values()) {
      if (!isEdited(d)) continue;
      const x = this.character?.textures.find((tx) => tx.name === d.name);
      if (this.eyeGroupOf(d.name) || x?.eye) tags.add("eyes");
      else if (this.fact(d.name)?.role === "low" || this.linkOf(d.name)) tags.add("low");
      else if (bike && bike.has(d.name) && !kart!.has(d.name)) tags.add("bike");
      else tags.add("body");
    }
    return ["body", "eyes", "bike", "low"].filter((tg) => tags.has(tg));
  }

  async loadModel(code: string) {
    this.p3dMessage(this.preview.ok ? t("p3d.loading") : t("p3d.failed", { why: t("p3d.noWebgl") }));
    this.renderParts();
    try {
      const b = await invoke<ModelBundle>("model_load", { character: code });
      if (this.character?.code !== code) return;
      this.bundle = b;
      this.modelError = null;
      this.unusedCache.clear();
      this.derivedCache.clear();
      this.outfitViews.clear();
      this.setOutfit("kart", "full");
      this.renderEyePanel();
      this.view.setUnused(this.current ? this.unusedMask(this.current.name) : null);
    } catch (e) {
      this.modelError = errText(e);
      if (this.preview.ok) this.p3dMessage(t("p3d.failed", { why: this.modelError }));
      this.setMode("2d");
    }
    ($("mode-3d") as HTMLButtonElement).disabled = !this.preview.ok || !!this.modelError;
    this.renderOutfitBar();
    this.renderParts();
    this.refreshOutlines();
    this.renderSwitcherButton();
  }

  texParts(name: string): TexParts | undefined {
    return this.bundle?.textures.find((x) => x.texture === name);
  }

  private showModelFor(name: string) {
    if (!this.bundle || !this.preview.ok) return;
    if (this.mode === "3d" && this.preview.current) {
      this.refreshEyePreview();
      return;
    }
    const tp = this.texParts(name);
    const mi = tp?.model ?? null;
    if (mi === null) {
      if (!this.preview.current) {
        const first = this.bundle.models[0];
        if (first) this.preview.show(first);
      }
      this.p3dMessage(t("p3d.noModel"), true);
    } else {
      this.preview.show(this.bundle.models[mi]);
      this.p3dMessage(null);
    }
    const m = this.preview.current;
    $("p3d-label").textContent = m ? this.modelLabel(m) : "";
    if (this.bundle && m) {
      const mi = this.bundle.models.indexOf(m);
      if (mi !== this.modelIndex) {
        this.geos.clear();
        this.owners.clear();
      }
      this.modelIndex = Math.max(0, mi);
      if (mi >= 0 && m.kind !== "menu") {
        this.vehicle = this.vehicleOf(m);
        this.detail = this.detailOf(m);
        this.renderOutfitBar();
      }
    }
    for (const d of this.docs.values()) if (this.preview.hasTexture(d.name) && (this.texParts(d.name)?.aliasOf ?? d.name) === d.name) this.preview.update(d.name);
    if (this.current) this.preview.update(this.current.name);
    this.preview.setHighlight(this.focus && this.focus.texture === this.current?.name ? this.pickOf(this.focus) : null, true);
  }
  private pickOf(p: PartRef) {
    return { texture: this.texParts(p.texture)?.aliasOf ?? p.texture, part: p.index };
  }

  private p3dMessage(msg: string | null, soft = false) {
    const el = $("p3d-msg");
    el.classList.toggle("hidden", !msg);
    el.classList.toggle("soft", soft);
    el.textContent = msg ?? "";
  }

  select(name: string) {
    const d = this.docs.get(name);
    if (!d) return;
    const prev = this.current;
    this.current = d;
    this.focus = this.focus?.texture === name ? this.focus : null;
    this.hover = null;
    this.view.setImage(d.w, d.h);
    this.view.update(composite(d));
    const x = this.character?.textures.find((t) => t.name === name);
    $("eye-warn").classList.toggle("hidden", !x?.eye || this.mode === "3d");
    this.color.setPalette(palette(d.original));
    this.view.setUnused(this.unusedMask(name));
    if (prev && prev !== d && (this.texParts(prev.name)?.aliasOf ?? null) !== null && this.mode === "2d") {
      const target = this.texParts(prev.name)!.aliasOf!;
      this.preview.update(target);
    }
    this.showModelFor(name);
    this.refreshOutlines();
    this.refreshSelectionOutline();
    this.renderSwitcherButton();
    this.renderLayers();
    this.renderParts();
    this.renderStatus();
  }

  changed(d: Doc, structural = false) {
    touch(d);
    this.changes++;
    this.dirtyDocs.add(d);
    if (structural) this.renderLayers();
    if (this.frame) return;
    this.frame = requestAnimationFrame(() => {
      this.frame = 0;
      const cur = this.current;
      if (!cur) return;
      this.view.update(composite(cur));
      const dirty = [...this.dirtyDocs];
      this.dirtyDocs.clear();
      if (!dirty.includes(cur)) dirty.push(cur);
      for (const x of dirty) this.uploadDoc(x);
      this.renderUndo();
      clearTimeout(this.thumbTimer);
      this.thumbTimer = window.setTimeout(() => {
        this.thumbs.delete(cur.name);
        this.renderSwitcherButton();
        this.renderLayerThumbs();
        this.renderOutfitBar();
        this.refreshPartColors();
        this.onEdited();
      }, 250);
    });
  }

  setTool(tool: Tool) {
    if (this.mode === "3d" && !TOOLS_3D.includes(tool)) tool = "brush";
    this.tool = tool;
    $("ed-tools").querySelectorAll<HTMLButtonElement>(".tool").forEach((b) => b.setAttribute("aria-pressed", String(b.dataset.tool === tool)));
    this.renderToolOpts();
    this.view.brushSize = tool === "brush" ? this.opts.brush.size : tool === "eraser" ? this.opts.eraser.size : tool === "line" ? this.opts.line.size : 0;
    this.view.canvas.dataset.tool = tool;
    this.view.hand = tool === "hand";
    if (tool !== "hand") this.view.canvas.style.cursor = "";
    this.preview.setHand(tool === "hand");
    this.preview.rightPicks = tool === "picker";
    this.view.request();
  }

  private renderToolOpts() {
    const o = this.opts;
    const slider = (id: string, key: Key, min: number, max: number, val: number, unit = "") =>
      `<label class="opt"><span>${esc(t(key))}</span><input type="range" id="${id}" min="${min}" max="${max}" value="${val}" /><output>${val}${unit}</output></label>`;
    const check = (id: string, key: Key, on: boolean) => `<label class="opt check"><input type="checkbox" id="${id}" ${on ? "checked" : ""}/><span>${esc(t(key))}</span></label>`;
    let html = `<h3>${esc(this.mode === "3d" && this.tool === "wand" ? t("tool.part") : t(`tool.${this.tool}` as Key))}</h3>`;
    const tool = this.tool;
    if (this.mode === "3d" && (tool === "brush" || tool === "eraser")) {
      const b = o[tool];
      html += slider("o-size3d", "opt.size", 2, 120, o.size3d[tool], " px");
      html += slider("o-hard", "opt.hardness", 0, 100, Math.round(b.hardness * 100), "%");
      html += slider("o-opacity", "opt.opacity", 1, 100, Math.round(b.opacity * 100), "%");
      if (tool === "eraser") html += `<label class="opt"><span>${esc(t("opt.eraseMode"))}</span><select id="o-erase"><option value="t" ${o.eraseOriginal ? "" : "selected"}>${esc(t("opt.eraseTransparent"))}</option><option value="o" ${o.eraseOriginal ? "selected" : ""}>${esc(t("opt.eraseOriginal"))}</option></select></label>`;
      html += check("o-mirror", "opt.showMirror", o.showMirror);
      html += `<p class="hint">${esc(t("opt.paint3dHint"))}</p>`;
    } else if (this.mode === "3d" && tool === "fill") {
      html += slider("o-opacity", "opt.opacity", 1, 100, Math.round(o.fillOpacity * 100), "%");
      html += `<p class="hint">${esc(t("opt.fill3dHint"))}</p>`;
    } else if (this.mode === "3d" && tool === "wand") {
      html += `<p class="hint">${esc(t("opt.part3dHint"))}</p>`;
    } else if (tool === "hand") {
      html += `<p class="hint" id="hand-hint">${esc(t(this.mode === "3d" ? "opt.handHint" : "opt.hand2dHint"))}</p>`;
      if (this.mode === "3d") html += `<div class="opt-row"><button type="button" class="mini" id="o-reset">${icon("center")}<span>${esc(t("p3d.reset"))}</span></button></div>`;
    } else if (tool === "brush" || tool === "eraser" || tool === "line") {
      const b = o[tool];
      html += slider("o-size", "opt.size", 1, 64, b.size, " px");
      if (tool !== "line") html += slider("o-hard", "opt.hardness", 0, 100, Math.round(b.hardness * 100), "%");
      html += slider("o-opacity", "opt.opacity", 1, 100, Math.round(b.opacity * 100), "%");
      html += check("o-pp", "opt.pixelPerfect", b.pixelPerfect);
      if (tool === "eraser") html += `<label class="opt"><span>${esc(t("opt.eraseMode"))}</span><select id="o-erase"><option value="t" ${o.eraseOriginal ? "" : "selected"}>${esc(t("opt.eraseTransparent"))}</option><option value="o" ${o.eraseOriginal ? "selected" : ""}>${esc(t("opt.eraseOriginal"))}</option></select></label>`;
    } else if (tool === "rect") {
      html += slider("o-size", "opt.size", 1, 32, o.rect.size, " px");
      html += slider("o-opacity", "opt.opacity", 1, 100, Math.round(o.rect.opacity * 100), "%");
      html += check("o-rfill", "opt.filled", o.rectFill);
    } else if (tool === "fill") {
      html += slider("o-tol", "opt.tolerance", 0, 255, o.fillTolerance);
      html += slider("o-opacity", "opt.opacity", 1, 100, Math.round(o.fillOpacity * 100), "%");
      html += check("o-contig", "opt.contiguous", o.fillContiguous);
    } else if (tool === "wand") {
      html += slider("o-tol", "opt.tolerance", 0, 255, o.wandTolerance);
      html += check("o-contig", "opt.contiguous", o.wandContiguous);
      html += `<p class="hint">${esc(t("opt.wandHint"))}</p>`;
    } else if (tool === "select") html += `<p class="hint">${esc(t("opt.selectHint"))}</p>`;
    else if (tool === "move") html += `<p class="hint">${esc(t("opt.moveHint"))}</p>`;
    else if (tool === "picker") html += `<p class="hint">${esc(t("opt.pickerHint"))}</p>`;
    if (this.mode === "2d" && (tool === "select" || tool === "wand" || tool === "move" || tool === "fill")) html += `<div class="opt-row"><button type="button" class="mini" id="o-all">${esc(t("opt.selectAll"))}</button><button type="button" class="mini" id="o-none">${esc(t("opt.deselect"))}</button><button type="button" class="mini" id="o-inv">${esc(t("opt.invert"))}</button></div>`;
    const host = $("tool-opts");
    host.innerHTML = html;
    syncRanges(host);
    const bind = (id: string, fn: (v: number, el: HTMLInputElement) => void) => {
      const el = host.querySelector<HTMLInputElement>("#" + id);
      if (!el) return;
      el.addEventListener("input", () => {
        fn(Number(el.value), el);
        const out = el.parentElement!.querySelector("output");
        if (out) out.textContent = out.textContent!.replace(/^[\d.]+/, el.value);
      });
      el.addEventListener("change", () => fn(Number(el.value), el));
    };
    const brush = (tool === "brush" || tool === "eraser" || tool === "line" || tool === "rect" ? o[tool] : null) as BrushOpts | null;
    bind("o-size", (v) => {
      if (brush) brush.size = v;
      this.view.brushSize = tool === "rect" ? 0 : v;
    });
    bind("o-size3d", (v) => {
      if (tool === "brush" || tool === "eraser") o.size3d[tool] = v;
    });
    bind("o-hard", (v) => brush && (brush.hardness = v / 100));
    bind("o-opacity", (v) => (brush ? (brush.opacity = v / 100) : (o.fillOpacity = v / 100)));
    bind("o-tol", (v) => (tool === "fill" ? (o.fillTolerance = v) : (o.wandTolerance = v)));
    bind("o-pp", (_, el) => brush && (brush.pixelPerfect = el.checked));
    bind("o-mirror", (_, el) => {
      o.showMirror = el.checked;
      localStorage.setItem("mkss.showMirror", el.checked ? "1" : "0");
      if (!el.checked) this.preview.setMirror(null);
    });
    bind("o-rfill", (_, el) => (o.rectFill = el.checked));
    bind("o-contig", (_, el) => (tool === "fill" ? (o.fillContiguous = el.checked) : (o.wandContiguous = el.checked)));
    host.querySelector("#o-erase")?.addEventListener("change", (e) => (o.eraseOriginal = (e.target as HTMLSelectElement).value === "o"));
    host.querySelector("#o-reset")?.addEventListener("click", () => this.preview.resetView());
    host.querySelector("#o-all")?.addEventListener("click", () => this.selectAll());
    host.querySelector("#o-none")?.addEventListener("click", () => this.setSelection(null));
    host.querySelector("#o-inv")?.addEventListener("click", () => this.invertSelection());
  }

  private paintable(d: Doc | null = this.current, quiet = false): Layer | null {
    if (!d) return null;
    const ok = (L: Layer) => L.kind === "pixel" && !!L.pixels && !L.locked && L.visible;
    const L = activeLayer(d);
    if (ok(L)) return L;
    const i = d.layers.indexOf(L);
    const below = [...d.layers.slice(0, i)].reverse().find(ok) ?? [...d.layers].reverse().find(ok);
    if (below) {
      d.active = below.id;
      if (d === this.current) this.renderLayers();
      if (!quiet) this.toast(t("lay.switched", { name: below.name }), "ok");
      return below;
    }
    if (!quiet) this.toast(t(L.kind !== "pixel" ? "lay.notPixel" : L.locked ? "lay.locked" : "lay.hidden"));
    return null;
  }

  private sample(x: number, y: number): RGBA | null {
    const d = this.current;
    if (!d) return null;
    const ix = Math.floor(x), iy = Math.floor(y);
    if (ix < 0 || iy < 0 || ix >= d.w || iy >= d.h) return null;
    const c = composite(d);
    const o = (iy * d.w + ix) * 4;
    return [c[o], c[o + 1], c[o + 2], c[o + 3]];
  }

  private down(x: number, y: number, e: PointerEvent) {
    const d = this.current;
    if (!d) return;
    const right = e.button === 2;
    if (this.pickingReplace) {
      const c = this.sample(x, y);
      this.pickingReplace = false;
      if (c) {
        this.replaceFrom = c;
        this.renderReplace();
      }
      return;
    }
    const tool = e.altKey && (this.tool === "brush" || this.tool === "fill" || this.tool === "line" || this.tool === "rect") ? "picker" : this.tool;
    if (tool === "picker") {
      const pick = (px: number, py: number) => {
        const c = this.sample(px, py);
        if (c) this.color.set(c, right);
      };
      pick(x, y);
      this.gesture = (px, py) => pick(px, py);
      return;
    }
    if (tool === "brush" || tool === "eraser") {
      const L = this.paintable();
      if (!L) return;
      const opts = tool === "brush" ? this.opts.brush : this.opts.eraser;
      const color = right ? this.color.secondary : this.color.primary;
      const snapshot = L.pixels!.slice();
      const stroke = new Stroke(d.w, d.h, opts);
      const mode = tool === "brush" ? ({ kind: "paint", color } as const) : this.opts.eraseOriginal && L.base ? ({ kind: "restore", original: d.original } as const) : ({ kind: "erase" } as const);
      const unused = this.unusedMask(d.name);
      let warned = false;
      $("st-warn").textContent = "";
      const step = (px: number, py: number) => {
        stroke.to(px, py);
        applyStroke(L.pixels!, snapshot, stroke, mode, d.selection);
        const ix = Math.floor(px), iy = Math.floor(py);
        if (!warned && unused && ix >= 0 && iy >= 0 && ix < d.w && iy < d.h && unused[iy * d.w + ix]) {
          warned = true;
          $("st-warn").textContent = t("adv.paintUnused");
        }
        this.changed(d);
      };
      step(x, y);
      this.gesture = (px, py, phase) => {
        if (phase === "move") step(px, py);
        else {
          this.history.pixels(d, L, snapshot, stroke.rect());
          if (tool === "brush") this.color.use(color);
          this.changed(d);
        }
      };
      return;
    }
    if (tool === "fill") {
      const L = this.paintable();
      if (!L) return;
      const snapshot = L.pixels!.slice();
      const color = right ? this.color.secondary : this.color.primary;
      const mask = regionMask(composite(d), d.w, d.h, Math.floor(x), Math.floor(y), this.opts.fillTolerance, this.opts.fillContiguous, d.selection);
      const rect = fillMask(L.pixels!, d.w, d.h, mask, color, this.opts.fillOpacity, null);
      this.history.pixels(d, L, snapshot, rect);
      this.color.use(color);
      this.changed(d);
      return;
    }
    if (tool === "line" || tool === "rect") {
      const L = this.paintable();
      if (!L) return;
      const color = right ? this.color.secondary : this.color.primary;
      const sx = x, sy = y;
      const make = (ex: number, ey: number) =>
        tool === "line" ? lineStroke(d.w, d.h, this.opts.line, sx, sy, ex, ey) : rectStroke(d.w, d.h, this.opts.rect, sx, sy, ex, ey, this.opts.rectFill);
      const snapshot = L.pixels!.slice();
      const opts = tool === "line" ? this.opts.line : this.opts.rect;
      const show = (ex: number, ey: number) => {
        const s = make(ex, ey);
        L.pixels!.set(snapshot);
        applyStroke(L.pixels!, snapshot, s, { kind: "paint", color }, d.selection);
        this.changed(d);
        return s;
      };
      show(x, y);
      this.gesture = (px, py, phase) => {
        const s = show(px, py);
        if (phase === "up") {
          void opts;
          this.history.pixels(d, L, snapshot, s.rect());
          this.color.use(color);
        }
      };
      return;
    }
    if (tool === "select") {
      const sx = Math.max(0, Math.min(d.w, Math.round(x))), sy = Math.max(0, Math.min(d.h, Math.round(y)));
      const base = d.selection ? d.selection.slice() : null;
      const mode = e.shiftKey ? "add" : e.altKey ? "sub" : "set";
      const apply = (px: number, py: number, final: boolean) => {
        const ex = Math.max(0, Math.min(d.w, Math.round(px))), ey = Math.max(0, Math.min(d.h, Math.round(py)));
        const [x0, x1] = [Math.min(sx, ex), Math.max(sx, ex)], [y0, y1] = [Math.min(sy, ey), Math.max(sy, ey)];
        if (final && (x1 - x0 === 0 || y1 - y0 === 0)) {
          if (mode === "set") this.setSelection(null);
          else this.setSelection(base);
          return;
        }
        const m = mode === "set" || !base ? new Uint8Array(d.w * d.h) : base.slice();
        for (let yy = y0; yy < y1; yy++) m.fill(mode === "sub" ? 0 : 255, yy * d.w + x0, yy * d.w + x1);
        this.setSelection(m, !final);
      };
      this.gesture = (px, py, phase) => apply(px, py, phase === "up");
      return;
    }
    if (tool === "wand") {
      const m = regionMask(composite(d), d.w, d.h, Math.floor(x), Math.floor(y), this.opts.wandTolerance, this.opts.wandContiguous, null);
      if (e.shiftKey && d.selection) for (let i = 0; i < m.length; i++) m[i] = Math.max(m[i], d.selection[i]);
      else if (e.altKey && d.selection) for (let i = 0; i < m.length; i++) m[i] = m[i] ? 0 : d.selection[i];
      this.setSelection(m);
      return;
    }
    if (tool === "move") {
      const L = this.paintable();
      if (!L) return;
      const snapshot = L.pixels!.slice();
      const sel = d.selection ? d.selection.slice() : null;
      const sx = Math.round(x), sy = Math.round(y);
      let last = [0, 0];
      const shift = (dx: number, dy: number) => {
        if (dx === last[0] && dy === last[1]) return;
        last = [dx, dy];
        const out = L.pixels!;
        out.set(snapshot);
        const ns = sel ? new Uint8Array(d.w * d.h) : null;
        if (sel) {
          for (let i = 0; i < sel.length; i++) if (sel[i]) out.fill(0, i * 4, i * 4 + 4);
        } else out.fill(0);
        for (let yy = 0; yy < d.h; yy++)
          for (let xx = 0; xx < d.w; xx++) {
            const i = yy * d.w + xx;
            if (sel && !sel[i]) continue;
            const tx = xx + dx, ty = yy + dy;
            if (tx < 0 || ty < 0 || tx >= d.w || ty >= d.h) continue;
            const j = ty * d.w + tx;
            const k = sel ? sel[i] / 255 : 1;
            for (let c = 0; c < 4; c++) out[j * 4 + c] = snapshot[i * 4 + c] * k + out[j * 4 + c] * (1 - k);
            if (ns) ns[j] = sel![i];
          }
        if (ns) {
          d.selection = ns;
          this.refreshSelectionOutline();
        }
        this.changed(d);
      };
      this.gesture = (px, py, phase) => {
        shift(Math.round(px) - sx, Math.round(py) - sy);
        if (phase === "up") this.history.pixels(d, L, snapshot, undefined, sel);
      };
    }
  }

  setSelection(m: Uint8Array | null, transient = false) {
    const d = this.current;
    if (!d) return;
    d.selection = m && m.some((v) => v) ? m : null;
    if (!transient && this.focus && (!d.selection || !this.sameAsFocus(d.selection))) {
      this.focus = null;
      this.preview.setHighlight(null, true);
      this.renderParts();
      this.refreshOutlines();
    }
    this.refreshSelectionOutline();
    $("parts-clear").toggleAttribute("disabled", !d.selection);
  }
  private sameAsFocus(m: Uint8Array) {
    if (!this.focus || !this.current) return false;
    const pm = this.partMask(this.focus.texture, this.focus.index);
    if (!pm) return false;
    for (let i = 0; i < m.length; i++) if (m[i] !== pm[i]) return false;
    return true;
  }
  selectAll() {
    const d = this.current;
    if (d) this.setSelection(new Uint8Array(d.w * d.h).fill(255));
  }
  invertSelection() {
    const d = this.current;
    if (!d) return;
    const m = new Uint8Array(d.w * d.h);
    for (let i = 0; i < m.length; i++) m[i] = 255 - (d.selection ? d.selection[i] : 0);
    this.setSelection(m);
  }
  private refreshSelectionOutline() {
    const d = this.current;
    this.view.selectionOutline = d?.selection ? maskOutline(d.selection, d.w, d.h) : [];
    this.view.request();
  }

  partMask(texture: string, index: number): Uint8Array | null {
    const key = `${texture}#${index}`;
    let m = this.maskCache.get(key);
    if (m) return m;
    const tp = this.texParts(texture);
    const p = tp?.parts[index];
    if (!tp || !p) return null;
    m = maskFromRuns(u32arr(p.runs), tp.width * tp.height);
    this.maskCache.set(key, m);
    return m;
  }
  private partOutline(texture: string, index: number): number[] {
    const key = `${texture}#${index}`;
    let o = this.outlineCache.get(key);
    if (!o) {
      const tp = this.texParts(texture)!;
      const m = this.partMask(texture, index);
      o = m ? maskOutline(m, tp.width, tp.height) : [];
      this.outlineCache.set(key, o);
    }
    return o;
  }
  partLabel(texture: string, index: number) {
    const tp = this.texParts(texture);
    const d = this.docs.get(texture);
    const p = tp?.parts[index];
    if (!p || !d) return "";
    const c = p.key === "eyes" ? null : colorName(this.partRgb(d, index));
    const name = c ? `${t(`part.${p.key}` as Key)} · ${t(`color.${c}` as Key)}` : t(`part.${p.key}` as Key);
    return p.n > 0 ? `${name} ${p.n}` : name;
  }
  selectPart(p: PartRef | null) {
    const d = this.current;
    if (!d) return;
    if (!p) {
      this.setSelection(null);
      return;
    }
    const m = this.partMask(p.texture, p.index);
    if (!m) return;
    this.focus = p;
    this.setSelection(m.slice());
    this.preview.setHighlight(this.pickOf(p), true);
    this.refreshOutlines();
    this.renderParts();
    $("parts-list").querySelector<HTMLElement>(`li[data-i="${p.index}"]`)?.scrollIntoView({ block: "nearest" });
  }
  private refreshOutlines() {
    const d = this.current;
    const tp = d ? this.texParts(d.name) : undefined;
    const all: number[] = [];
    if (tp && d && tp.width === d.w && tp.height === d.h) tp.parts.forEach((_, i) => all.push(...this.partOutline(tp.texture, i)));
    this.view.partOutlines = all;
    this.view.focusOutline = this.focus && d && this.focus.texture === d.name ? this.partOutline(this.focus.texture, this.focus.index) : [];
    this.renderHover();
  }
  private renderHover() {
    const d = this.current;
    this.view.hoverOutline = this.hover && d && this.hover.texture === d.name ? this.partOutline(this.hover.texture, this.hover.index) : [];
    $("parts-list").querySelectorAll<HTMLElement>("li").forEach((li) => li.classList.toggle("hover", !!this.hover && Number(li.dataset.i) === this.hover.index));
    this.view.request();
  }
  private partAt(x: number, y: number): number | null {
    const d = this.current;
    const tp = d && this.texParts(d.name);
    if (!d || !tp) return null;
    const ix = Math.floor(x), iy = Math.floor(y);
    if (ix < 0 || iy < 0 || ix >= d.w || iy >= d.h) return null;
    for (let i = 0; i < tp.parts.length; i++) if (this.partMask(d.name, i)?.[iy * d.w + ix]) return i;
    return null;
  }

  renderParts() {
    const list = $("parts-list");
    const d = this.current;
    const tp = d ? this.texParts(d.name) : undefined;
    if (!this.bundle) {
      list.innerHTML = `<li class="empty">${esc(this.modelError ? t("parts.unavailable") : t("parts.loading"))}</li>`;
      return;
    }
    if (!d || !tp || !tp.parts.length) {
      list.innerHTML = `<li class="empty">${esc(t("parts.none"))}</li>`;
      return;
    }
    const total = d.w * d.h;
    list.innerHTML = tp.parts
      .map((p, i) => {
        const c = this.partColor(d, i);
        const on = this.focus?.texture === d.name && this.focus.index === i;
        return `<li data-i="${i}" class="${on ? "on" : ""}" tabindex="0" role="option" aria-selected="${on}"><i class="sw" style="background:${c}"></i><span>${esc(this.partLabel(d.name, i))}</span><small>${Math.max(1, Math.round((p.pixels / total) * 100))}%</small></li>`;
      })
      .join("");
    list.querySelectorAll<HTMLElement>("li[data-i]").forEach((li) => {
      const i = Number(li.dataset.i);
      li.addEventListener("click", () => this.selectPart(this.focus?.texture === d.name && this.focus.index === i ? null : { texture: d.name, index: i }));
      li.addEventListener("keydown", (e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          li.click();
        }
      });
      li.addEventListener("pointerenter", () => {
        this.hover = { texture: d.name, index: i };
        this.preview.setHighlight(this.pickOf(this.hover), false);
        this.renderHover();
      });
      li.addEventListener("pointerleave", () => {
        this.hover = null;
        this.preview.setHighlight(null, false);
        this.renderHover();
      });
    });
  }
  private rgbCache = new Map<string, { key: string; rgb: [number, number, number] | null }>();
  private partRgb(d: Doc, i: number): [number, number, number] | null {
    const id = `${d.name}#${i}`;
    const key = this.pxKey(d);
    const hit = this.rgbCache.get(id);
    if (hit && hit.key === key) return hit.rgb;
    const rgb = this.dominantRgb(this.pixelsOf(d), this.partMask(d.name, i));
    this.rgbCache.set(id, { key, rgb });
    return rgb;
  }
  private dominantRgb(px: Uint8ClampedArray, m: Uint8Array | null): [number, number, number] | null {
    if (!m) return null;
    const count = new Map<number, number[]>();
    let best: number[] | null = null;
    for (let k = 0; k < m.length; k++) {
      if (!m[k] || px[k * 4 + 3] === 0) continue;
      const r = px[k * 4], g = px[k * 4 + 1], b = px[k * 4 + 2];
      const q = ((r >> 3) << 10) | ((g >> 3) << 5) | (b >> 3);
      let e = count.get(q);
      if (!e) count.set(q, (e = [0, 0, 0, 0]));
      e[0]++;
      e[1] += r;
      e[2] += g;
      e[3] += b;
      if (!best || e[0] > best[0]) best = e;
    }
    return best ? [best[1] / best[0], best[2] / best[0], best[3] / best[0]] : null;
  }
  private partColor(d: Doc, i: number) {
    const c = this.partRgb(d, i);
    return c ? `rgb(${Math.round(c[0])},${Math.round(c[1])},${Math.round(c[2])})` : "transparent";
  }
  refreshPartColors() {
    const d = this.current;
    const tp = d ? this.texParts(d.name) : undefined;
    if (!d || !tp) return;
    $("parts-list").querySelectorAll<HTMLElement>("li[data-i]").forEach((li) => {
      const i = Number(li.dataset.i);
      if (!tp.parts[i]) return;
      li.querySelector<HTMLElement>(".sw")!.style.background = this.partColor(d, i);
      const label = this.partLabel(d.name, i);
      const span = li.querySelector("span")!;
      if (span.textContent !== label) span.textContent = label;
    });
    if (this.mode === "3d" && $("st-part").textContent) this.renderStatusHit();
  }

  private structural(fn: (d: Doc) => void) {
    const d = this.current;
    if (!d) return;
    const before = this.history.snapLayers(d);
    const active = d.active;
    fn(d);
    this.history.layers(d, before, active);
    this.changed(d, true);
  }
  private newLayer(d: Doc, name: string, pixels?: Uint8ClampedArray): Layer {
    return { id: d.nextId++, name, kind: "pixel", base: false, visible: true, locked: false, opacity: 1, blend: "normal", pixels: pixels ?? new Uint8ClampedArray(d.w * d.h * 4), adjust: null, mask: null };
  }
  private insertAbove(d: Doc, L: Layer) {
    const i = d.layers.findIndex((l) => l.id === d.active);
    d.layers.splice(i + 1, 0, L);
    d.active = L.id;
  }
  addLayer() {
    this.structural((d) => this.insertAbove(d, this.newLayer(d, t("lay.layer", { n: d.layers.filter((l) => !l.base).length + 1 }))));
  }
  duplicateLayer() {
    this.structural((d) => {
      const s = activeLayer(d);
      const L: Layer = { ...s, id: d.nextId++, base: false, locked: false, name: t("lay.copyOf", { name: s.name }), pixels: s.pixels ? s.pixels.slice() : null, adjust: s.adjust ? { ...s.adjust } : null, mask: s.mask ? s.mask.slice() : null };
      this.insertAbove(d, L);
    });
  }
  deleteLayer() {
    const d = this.current;
    if (!d) return;
    const L = activeLayer(d);
    if (L.base) {
      this.toast(t("lay.cantDeleteBase"));
      return;
    }
    this.structural((d) => {
      const i = d.layers.indexOf(L);
      d.layers.splice(i, 1);
      d.active = d.layers[Math.max(0, i - 1)].id;
    });
  }
  addAdjustment() {
    const d = this.current;
    if (!d) return;
    const scope = d.selection ? d.selection.slice() : null;
    const label = this.focus && this.focus.texture === d.name && d.selection ? this.partLabel(d.name, this.focus.index) : scope ? t("adj.scopeSel") : t("adj.scopeAll");
    this.structural((d) => {
      const L: Layer = { id: d.nextId++, name: t("lay.recolorOf", { part: label }), kind: "adjust", base: false, visible: true, locked: false, opacity: 1, blend: "normal", pixels: null, adjust: identityAdjust(), mask: scope };
      d.layers.push(L);
      d.active = L.id;
    });
  }
  private readAdjust() {
    const d = this.current;
    if (!d) return;
    const L = activeLayer(d);
    if (!L.adjust) return;
    L.adjust.hue = Number(($("sl-hue") as HTMLInputElement).value);
    L.adjust.saturation = Number(($("sl-sat") as HTMLInputElement).value);
    L.adjust.brightness = Number(($("sl-bri") as HTMLInputElement).value);
    L.adjust.colorize = ($("adj-colorize") as HTMLInputElement).checked;
    this.renderAdjustOutputs(L);
    this.changed(d);
  }
  private renderAdjustOutputs(L: Layer) {
    const a = L.adjust!;
    $("out-hue").textContent = `${Math.round(a.hue)}°`;
    $("out-sat").textContent = `${Math.round(a.saturation)}%`;
    $("out-bri").textContent = `${Math.round(a.brightness)}%`;
  }
  async importImage() {
    const d = this.current;
    if (!d) return;
    const path = await open({ title: t("edit.pickImage"), filters: [{ name: t("edit.imageFilter"), extensions: ["png", "jpg", "jpeg"] }] });
    if (typeof path !== "string") return;
    let file: { name: string; width: number; height: number; sha256: string; dataUrl: string };
    try {
      file = await invoke("read_image", { path });
    } catch (e) {
      this.toast(errText(e));
      return;
    }
    await this.addImageLayer(d, file);
  }
  async addImageLayer(d: Doc, file: { name: string; width: number; height: number; sha256: string; dataUrl: string }) {
    const img = await decodeDataUrl(file.dataUrl);
    const box = d.selection ? maskBBox(d.selection, d.w, d.h) : null;
    const [bx, by, bx1, by1] = box ?? [0, 0, d.w, d.h];
    const bw = bx1 - bx, bh = by1 - by;
    const px = new Uint8ClampedArray(d.w * d.h * 4);
    const sx = img.width / bw, sy = img.height / bh;
    for (let y = 0; y < bh; y++)
      for (let x = 0; x < bw; x++) {
        const x0 = Math.floor(x * sx), x1 = Math.max(x0 + 1, Math.floor((x + 1) * sx));
        const y0 = Math.floor(y * sy), y1 = Math.max(y0 + 1, Math.floor((y + 1) * sy));
        let r = 0, g = 0, b = 0, a = 0, n = 0;
        for (let yy = y0; yy < Math.min(y1, img.height); yy++)
          for (let xx = x0; xx < Math.min(x1, img.width); xx++) {
            const o = (yy * img.width + xx) * 4;
            const al = img.data[o + 3];
            r += img.data[o] * al;
            g += img.data[o + 1] * al;
            b += img.data[o + 2] * al;
            a += al;
            n++;
          }
        const o = ((by + y) * d.w + bx + x) * 4;
        if (a > 0) (px[o] = r / a), (px[o + 1] = g / a), (px[o + 2] = b / a);
        px[o + 3] = n ? a / n : 0;
        if (d.selection) px[o + 3] *= d.selection[(by + y) * d.w + bx + x] / 255;
      }
    this.structural((d) => {
      const L = this.newLayer(d, file.name, px);
      L.source = { name: file.name, sha256: file.sha256 };
      this.insertAbove(d, L);
    });
    const stretched = Math.abs(img.width / img.height - bw / bh) > 0.01;
    this.toast(t("lay.imported", { file: file.name }) + (stretched ? " " + t("lay.stretched", { w: bw, h: bh }) : ""), "ok");
  }
  private moveLayer(id: number, toIndex: number) {
    this.structural((d) => {
      const i = d.layers.findIndex((l) => l.id === id);
      if (i < 0 || d.layers[i].base) return;
      const [L] = d.layers.splice(i, 1);
      d.layers.splice(Math.max(1, Math.min(d.layers.length, toIndex)), 0, L);
    });
  }

  renderLayers() {
    const d = this.current;
    const list = $("layer-list");
    if (!d) {
      list.innerHTML = "";
      return;
    }
    const rows = [...d.layers].reverse();
    list.innerHTML = rows
      .map((L) => {
        const on = L.id === d.active;
        return `<li data-id="${L.id}" class="${on ? "on" : ""}${L.visible ? "" : " off"}" draggable="${!L.base}" tabindex="0" role="option" aria-selected="${on}">
          <button type="button" class="icon-btn vis" data-i18n-title="lay.visible" aria-pressed="${L.visible}">${icon(L.visible ? "eye" : "eyeOff")}</button>
          <button type="button" class="icon-btn lck" data-i18n-title="lay.lock" aria-pressed="${L.locked}">${icon(L.locked ? "lock" : "unlock")}</button>
          <span class="lthumb checker">${L.kind === "adjust" ? icon("recolor") : `<canvas width="28" height="28"></canvas>`}</span>
          <span class="lname" data-i18n-title="lay.rename">${esc(L.name)}</span>
          ${L.opacity < 1 ? `<small>${Math.round(L.opacity * 100)}%</small>` : ""}${L.blend !== "normal" ? `<small>${esc(t(`blend.${L.blend}` as Key))}</small>` : ""}</li>`;
      })
      .join("");
    applyStatic(list);
    list.querySelectorAll<HTMLElement>("li").forEach((li) => {
      const id = Number(li.dataset.id);
      const L = d.layers.find((l) => l.id === id)!;
      li.addEventListener("click", (e) => {
        if ((e.target as HTMLElement).closest("button,input")) return;
        if (d.active !== id) {
          d.active = id;
          this.renderLayers();
        }
      });
      li.querySelector(".vis")!.addEventListener("click", () => this.structural(() => (L.visible = !L.visible)));
      li.querySelector(".lck")!.addEventListener("click", () => this.structural(() => (L.locked = !L.locked)));
      li.querySelector(".lname")!.addEventListener("dblclick", () => this.rename(li, L));
      li.addEventListener("dragstart", (e) => {
        e.dataTransfer?.setData("text/plain", String(id));
        li.classList.add("dragging");
      });
      li.addEventListener("dragend", () => li.classList.remove("dragging"));
      li.addEventListener("dragover", (e) => {
        e.preventDefault();
        li.classList.add("drop");
      });
      li.addEventListener("dragleave", () => li.classList.remove("drop"));
      li.addEventListener("drop", (e) => {
        e.preventDefault();
        li.classList.remove("drop");
        const from = Number(e.dataTransfer?.getData("text/plain"));
        if (!from || from === id) return;
        this.moveLayer(from, d.layers.findIndex((l) => l.id === id));
      });
    });
    this.renderLayerThumbs();
    const L = activeLayer(d);
    const op = $("lay-opacity") as HTMLInputElement;
    op.value = String(Math.round(L.opacity * 100));
    $("lay-opacity-out").textContent = `${op.value}%`;
    ($("lay-blend") as HTMLSelectElement).value = L.blend;
    ($("lay-blend") as HTMLSelectElement).disabled = L.kind === "adjust";
    ($("lay-del") as HTMLButtonElement).disabled = L.base;
    $("adjust-panel").classList.toggle("hidden", L.kind !== "adjust");
    if (L.adjust) {
      ($("sl-hue") as HTMLInputElement).value = String(L.adjust.hue);
      ($("sl-sat") as HTMLInputElement).value = String(L.adjust.saturation);
      ($("sl-bri") as HTMLInputElement).value = String(L.adjust.brightness);
      ($("adj-colorize") as HTMLInputElement).checked = L.adjust.colorize;
      $("adj-scope").textContent = L.mask ? t("adj.scopeSel") : t("adj.scopeAll");
      this.renderAdjustOutputs(L);
    }
    syncRanges($("view-edit"));
    this.renderUndo();
  }
  private renderLayerThumbs() {
    const d = this.current;
    if (!d) return;
    $("layer-list").querySelectorAll<HTMLElement>("li").forEach((li) => {
      const L = d.layers.find((l) => l.id === Number(li.dataset.id));
      const c = li.querySelector("canvas");
      if (!L?.pixels || !c) return;
      drawThumb(c, L.pixels, d.w, d.h);
    });
  }
  private rename(li: HTMLElement, L: Layer) {
    const span = li.querySelector(".lname") as HTMLElement;
    const input = document.createElement("input");
    input.value = L.name;
    input.className = "lrename";
    input.maxLength = 40;
    span.replaceWith(input);
    input.focus();
    input.select();
    const done = (ok: boolean) => {
      const v = input.value.trim();
      if (ok && v && v !== L.name) this.structural(() => (L.name = v));
      else this.renderLayers();
    };
    input.addEventListener("keydown", (e) => {
      if (e.key === "Enter") done(true);
      if (e.key === "Escape") done(false);
    });
    input.addEventListener("blur", () => done(true));
  }

  private toggleReplace() {
    const pop = $("replace-pop");
    pop.classList.toggle("hidden");
    if (!pop.classList.contains("hidden")) this.renderReplace();
  }
  private renderReplace() {
    const from = this.replaceFrom ?? this.color.secondary;
    ($("rc-from") as HTMLElement).style.background = hex(from);
    $("rc-from").title = hex(from);
    ($("rc-to") as HTMLElement).style.background = hex(this.color.primary);
    $("rc-to").title = hex(this.color.primary);
  }
  applyReplace() {
    const d = this.current;
    if (!d) return;
    const from = this.replaceFrom ?? this.color.secondary;
    const to = this.color.primary;
    const tol = Number(($("rc-tol") as HTMLInputElement).value);
    const px = replaceColor(composite(d), d.w, d.h, from, to, tol, d.selection);
    if (!px) {
      this.toast(t("rc.none"));
      return;
    }
    this.structural((d) => this.insertAbove(d, this.newLayer(d, t("rc.layer", { from: hex(from), to: hex(to) }), px)));
    this.structural((d) => {
      const L = activeLayer(d);
      d.layers.splice(d.layers.indexOf(L), 1);
      d.layers.push(L);
    });
    this.color.use(to);
    $("replace-pop").classList.add("hidden");
  }

  async applyToAllEyes() {
    const d = this.current;
    if (!d) return;
    const stem = d.name.replace(/\.\d+$/, "");
    const targets = [...this.docs.values()].filter((x) => x !== d && x.name.replace(/\.\d+$/, "") === stem && /\.\d+$/.test(x.name));
    for (const tdoc of targets) {
      const before = this.history.snapLayers(tdoc);
      const act = tdoc.active;
      const scale = (src: Uint8ClampedArray | Uint8Array, ch: number) => {
        const out = new (src instanceof Uint8Array ? Uint8Array : Uint8ClampedArray)(tdoc.w * tdoc.h * ch);
        for (let y = 0; y < tdoc.h; y++)
          for (let x = 0; x < tdoc.w; x++) {
            const sx = Math.min(d.w - 1, Math.floor((x * d.w) / tdoc.w)), sy = Math.min(d.h - 1, Math.floor((y * d.h) / tdoc.h));
            for (let c = 0; c < ch; c++) out[(y * tdoc.w + x) * ch + c] = src[(sy * d.w + sx) * ch + c];
          }
        return out;
      };
      const base = tdoc.layers.find((l) => l.base)!;
      const srcBase = d.layers.find((l) => l.base)!;
      if (baseEdited(d) && srcBase.pixels && base.pixels) {
        const snap = base.pixels.slice();
        const diff = scale(srcBase.pixels, 4) as Uint8ClampedArray;
        const orig = scale(d.original, 4) as Uint8ClampedArray;
        for (let i = 0; i < diff.length; i += 4) if (diff[i] !== orig[i] || diff[i + 1] !== orig[i + 1] || diff[i + 2] !== orig[i + 2] || diff[i + 3] !== orig[i + 3]) base.pixels.set(diff.subarray(i, i + 4), i);
        this.history.pixels(tdoc, base, snap);
      }
      tdoc.layers = [base, ...d.layers.filter((l) => !l.base).map((l) => ({ ...l, id: tdoc.nextId++, pixels: l.pixels ? (scale(l.pixels, 4) as Uint8ClampedArray) : null, mask: l.mask ? (scale(l.mask, 1) as Uint8Array) : null, adjust: l.adjust ? { ...l.adjust } : null }))];
      tdoc.active = tdoc.layers[tdoc.layers.length - 1].id;
      this.history.layers(tdoc, before, act);
      touch(tdoc);
      this.thumbs.delete(tdoc.name);
    }
    this.changes++;
    this.toast(t("eye.applied", { n: targets.length }), "ok");
    this.renderSwitcherButton();
    this.onEdited();
  }

  fact(name: string) {
    return this.bundle?.facts?.find((f) => f.texture === name);
  }
  linkOf(name: string): LinkData | undefined {
    return this.bundle?.links?.find((l) => l.low === name);
  }
  shareOf(name: string): LinkData | undefined {
    return this.bundle?.shares?.find((l) => l.low === name && l.mapped > 0);
  }
  eyeGroupOf(name: string): EyeData | undefined {
    return this.bundle?.eyes?.find((e) => e.frames.includes(name));
  }
  surfaceName(name: string): string {
    const f = this.fact(name);
    const x = this.character?.textures.find((tx) => tx.name === name);
    const role = f?.role ?? (x?.eye ? "eye" : "other");
    const bodies = (this.bundle?.facts ?? []).filter((y) => y.role === "body" && !/braid|hair/i.test(y.texture));
    const bike = /bike/i.test(name);
    if (/braid|hair/i.test(name) && role !== "eye") return t("surf.hair");
    if (role === "body") return bodies.length > 1 ? t(bike ? "surf.bodyBike" : "surf.bodyKart") : t("surf.body");
    if (role === "low") return t(bike ? "surf.lowBike" : "surf.low");
    if (role === "eye") {
      const g = this.eyeGroupOf(name);
      return g ? t("surf.eyeFrame", { n: g.frames.indexOf(name) }) : t("surf.eyes");
    }
    return name;
  }
  unusedMask(name: string): Uint8Array | null {
    if (this.unusedCache.has(name)) return this.unusedCache.get(name)!;
    const f = this.fact(name);
    const d = this.docs.get(name);
    let m: Uint8Array | null = null;
    if (f && d && f.width === d.w && f.height === d.h && f.usedTexels > 0) {
      const used = maskFromRuns(u32arr(f.used), d.w * d.h);
      m = new Uint8Array(d.w * d.h);
      for (let i = 0; i < m.length; i++) m[i] = used[i] ? 0 : 1;
    }
    if (this.bundle) this.unusedCache.set(name, m);
    return m;
  }
  private pxKey(d: Doc): string {
    const parts = [String(d.rev)];
    const share = this.shareOf(d.name);
    if (share) parts.push(`s${this.docs.get(share.highs[0].texture)?.rev ?? ""}`);
    for (const h of this.linkOf(d.name)?.highs ?? []) {
      const x = this.docs.get(h.texture);
      parts.push(x ? `(${this.pxKey(x)})` : "");
    }
    return parts.join(",");
  }
  private applyLink(base: Uint8ClampedArray, d: Doc, link: LinkData, srcs: Doc[], edits: (Uint8ClampedArray | null)[]) {
    if (!edits.some((e) => e)) return base;
    const px = base.slice();
    const S = i32arr(link.samples);
    const n = d.w * d.h;
    for (let i = 0; i < n; i++) {
      if (S[i * 4] < 0) continue;
      let changed = false;
      const delta = [0, 0, 0, 0];
      for (let k = 0; k < 4; k++) {
        const v = S[i * 4 + k];
        const src = v >> 24, o = (v & 0xffffff) * 4;
        const e = edits[src];
        if (!e) continue;
        const orig = srcs[src].original;
        for (let ch = 0; ch < 4; ch++) {
          const dd = e[o + ch] - orig[o + ch];
          if (dd) changed = true;
          delta[ch] += dd / 4;
        }
      }
      const own = base[i * 4] !== d.original[i * 4] || base[i * 4 + 1] !== d.original[i * 4 + 1] || base[i * 4 + 2] !== d.original[i * 4 + 2] || base[i * 4 + 3] !== d.original[i * 4 + 3];
      if (changed && !own) for (let ch = 0; ch < 4; ch++) px[i * 4 + ch] = Math.round(base[i * 4 + ch] + delta[ch]);
    }
    return px;
  }
  private linkFits(d: Doc, link: LinkData, srcs: (Doc | undefined)[]): srcs is Doc[] {
    return d.w === link.width && d.h === link.height && srcs.every((h, i) => !!h && h.w === link.highs[i].width && h.h === link.highs[i].height);
  }
  pixelsOf(d: Doc): Uint8ClampedArray {
    const base = composite(d);
    const share = this.shareOf(d.name);
    const link = this.linkOf(d.name);
    if (!link && !share) return base;
    const key = this.pxKey(d);
    const c = this.derivedCache.get(d.name);
    if (c && c.key === key) return c.px;
    let px = base;
    if (share) {
      const srcs = [this.docs.get(share.highs[0].texture)];
      if (this.linkFits(d, share, srcs)) px = this.applyLink(px, d, share, srcs, srcs.map((s) => (isEdited(s) ? composite(s) : null)));
    }
    if (link) {
      const srcs = link.highs.map((h) => this.docs.get(h.texture));
      if (this.linkFits(d, link, srcs)) px = this.applyLink(px, d, link, srcs, srcs.map((s) => (this.effectiveEdited(s) ? this.pixelsOf(s) : null)));
    }
    this.derivedCache.set(d.name, { key, px });
    return px;
  }
  effectiveEdited(d: Doc) {
    if (!this.linkOf(d.name) && !this.shareOf(d.name)) return isEdited(d);
    const px = this.pixelsOf(d);
    for (let i = 0; i < px.length; i++) if (px[i] !== d.original[i]) return true;
    return false;
  }
  private uploadDoc(d: Doc) {
    const g = this.eyeGroupOf(d.name);
    if (!g || (this.mode === "2d" ? d === this.current || d.name === this.displayedFrame(g) : d.name === this.displayedFrame(g))) this.preview.update(d.name);
    const follow = new Set([d.name, ...(this.bundle?.shares ?? []).filter((l) => l.highs[0].texture === d.name).map((l) => l.low)]);
    for (const n of follow) if (n !== d.name && this.preview.hasTexture(n)) this.preview.update(n);
    for (const l of this.bundle?.links ?? []) if (l.highs.some((h) => follow.has(h.texture)) && this.preview.hasTexture(l.low)) this.preview.update(l.low);
  }
  private displayedFrame(g: EyeData) {
    if (this.blink && this.blinkFrame) return this.blinkFrame;
    if (this.mode === "2d" && this.current && g.frames.includes(this.current.name)) return this.current.name;
    return this.eyeMode !== "all" && g.frames.includes(this.eyeMode) ? this.eyeMode : g.defaultFrame;
  }
  refreshEyePreview() {
    for (const g of this.bundle?.eyes ?? []) this.preview.update(this.displayedFrame(g));
  }
  setBlink(on: boolean) {
    this.blink = on;
    ($("eye-blink") as HTMLInputElement).checked = on;
    clearTimeout(this.blinkTimer);
    this.blinkFrame = null;
    if (on) {
      let i = 0;
      const step = () => {
        const g = this.bundle?.eyes?.[0];
        if (!this.blink || !g) return;
        const k = i % g.frames.length;
        this.blinkFrame = g.frames[k];
        this.refreshEyePreview();
        $("eye-hint").textContent = t("eye.showing", { n: k, total: g.frames.length });
        i++;
        this.blinkTimer = window.setTimeout(step, 800);
      };
      step();
    } else {
      this.refreshEyePreview();
      this.renderEyePanel();
    }
  }
  renderEyePanel() {
    const g = this.bundle?.eyes?.[0];
    const show = !!g && this.mode === "3d" && this.detail === "full";
    $("eye-panel").classList.toggle("hidden", !show);
    if (!g) return;
    if (this.eyeMode !== "all" && !g.frames.includes(this.eyeMode)) this.eyeMode = "all";
    const sel = $("eye-frame") as HTMLSelectElement;
    sel.innerHTML = [`<option value="all">${esc(t("eye.allFrames", { n: g.frames.length }))}</option>`, ...g.frames.map((f, i) => `<option value="${esc(f)}">${esc(f === g.defaultFrame ? t("eye.frameDefault", { n: i }) : t("eye.frameN", { n: i }))}</option>`)].join("");
    sel.value = this.eyeMode;
    $("eye-hint").textContent = this.eyeMode === "all" ? t("eye.hintAll", { n: g.frames.length }) : t("eye.hintOne", { n: g.frames.indexOf(this.eyeMode) });
  }
  private modelLabel(m: ModelData) {
    if (m.kind === "menu") return t("p3d.menu");
    const detail = t(this.detailOf(m) === "low" ? "outfit.low" : "outfit.full");
    return this.hasBikeOutfit() ? `${t(this.vehicleOf(m) === "bike" ? "outfit.bike" : "outfit.kart")} · ${detail}` : detail;
  }

  vehicleOf(m: ModelData): Vehicle {
    return (m.vehicle ?? (/bike/i.test(m.file.split(/[\\/]/).pop()?.split("-")[0] ?? "") ? "bike" : "kart")) === "bike" ? "bike" : "kart";
  }
  detailOf(m: ModelData): Detail {
    return (m.detail ?? (/lod/i.test(m.name) ? "low" : "full")) === "low" ? "low" : "full";
  }
  hasBikeOutfit() {
    return (this.bundle?.models ?? []).some((m) => m.kind === "race" && this.vehicleOf(m) === "bike" && this.detailOf(m) === "full");
  }
  hasLowDetail() {
    return (this.bundle?.models ?? []).some((m) => m.kind === "race" && this.detailOf(m) === "low");
  }
  outfitModels(v: Vehicle, d: Detail): ModelData[] {
    const race = (this.bundle?.models ?? []).filter((m) => m.kind === "race");
    const pick = (vv: Vehicle) => race.filter((m) => this.vehicleOf(m) === vv && this.detailOf(m) === d);
    const ms = pick(v).length ? pick(v) : pick("kart");
    return ms.length ? ms : race.filter((m) => this.detailOf(m) === "full");
  }
  outfitView(v: Vehicle, d: Detail): ModelData | null {
    const key = `${v}/${d}`;
    const hit = this.outfitViews.get(key);
    if (hit) return hit;
    const ms = this.outfitModels(v, d);
    if (!ms.length) return null;
    const tris = (m: ModelData) => m.submeshes.reduce((a, s) => a + s.triangles, 0);
    const main = [...ms].sort((a, b) => tris(b) - tris(a))[0];
    const inside = (m: ModelData) => [0, 1, 2].every((a) => m.bboxMin[a] >= main.bboxMin[a] - (main.bboxMax[a] - main.bboxMin[a]) * 0.25 && m.bboxMax[a] <= main.bboxMax[a] + (main.bboxMax[a] - main.bboxMin[a]) * 0.25);
    const extra = ms.filter((m) => m !== main && inside(m));
    const view: ModelData = extra.length
      ? {
          ...main,
          bboxMin: [0, 1, 2].map((a) => Math.min(main.bboxMin[a], ...extra.map((m) => m.bboxMin[a]))) as [number, number, number],
          bboxMax: [0, 1, 2].map((a) => Math.max(main.bboxMax[a], ...extra.map((m) => m.bboxMax[a]))) as [number, number, number],
          submeshes: [...main.submeshes, ...extra.flatMap((m) => m.submeshes)],
        }
      : main;
    this.outfitViews.set(key, view);
    return view;
  }
  mainTexture(m: ModelData): string | null {
    let best: string | null = null, n = -1;
    for (const s of m.submeshes) if (s.texture && this.docs.has(s.texture) && s.triangles > n && !this.eyeGroupOf(s.texture)) (best = s.texture), (n = s.triangles);
    return best ?? m.submeshes.find((s) => s.texture && this.docs.has(s.texture))?.texture ?? null;
  }
  private outfitTextures(v: Vehicle, d?: Detail): Set<string> {
    const out = new Set<string>();
    for (const dd of d ? [d] : (["full", "low"] as Detail[]))
      for (const m of this.outfitModels(v, dd))
        for (const s of m.submeshes) {
          if (!s.texture) continue;
          out.add(s.texture);
          for (const f of this.bundle?.eyes?.find((e) => e.frames[0] === s.texture)?.frames ?? []) out.add(f);
        }
    return out;
  }
  setOutfit(v: Vehicle, d: Detail) {
    if (!this.hasBikeOutfit()) v = "kart";
    if (!this.hasLowDetail()) d = "full";
    const changed = v !== this.vehicle || d !== this.detail;
    this.vehicle = v;
    this.detail = d;
    const m = this.outfitView(v, d);
    if (changed) {
      this.focus = this.hover = null;
      if (this.current?.selection) this.setSelection(null);
      $("st-part").textContent = "";
    }
    if (m && this.preview.ok && this.mode === "3d") this.showModel(m);
    const main = m && this.mainTexture(m);
    if (changed) this.preview.setHighlight(null, true);
    if (main && (changed || !this.current || !this.shownByModel(this.current.name, m))) this.select(main);
    else {
      if (this.mode === "2d" && this.current) this.showModelFor(this.current.name);
      this.renderParts();
      this.refreshOutlines();
    }
    this.renderOutfitBar();
    this.renderEyePanel();
  }
  private shownByModel(name: string, m: ModelData) {
    const target = this.texParts(name)?.aliasOf ?? this.eyeGroupOf(name)?.frames[0] ?? name;
    return m.submeshes.some((s) => s.texture === name || s.texture === target);
  }
  renderOutfitBar() {
    const bike = this.hasBikeOutfit(), low = this.hasLowDetail();
    $("ed-outfit").classList.toggle("hidden", !this.bundle || (!bike && !low));
    $("vehicle-group").classList.toggle("hidden", !bike);
    $("detail-group").classList.toggle("hidden", !low);
    for (const v of ["kart", "bike"] as Vehicle[]) {
      const b = $(`veh-${v}`);
      b.setAttribute("aria-checked", String(this.vehicle === v));
      b.querySelector(".seg-dot")!.classList.toggle("hidden", !bike || ![...this.outfitTextures(v)].some((n) => this.edited(n)));
    }
    for (const d of ["full", "low"] as Detail[]) {
      const b = $(`det-${d}`);
      b.setAttribute("aria-checked", String(this.detail === d));
      const names = [...this.outfitTextures(this.vehicle, d)];
      const dot = d === "full" ? names.some((n) => this.edited(n)) : names.some((n) => { const x = this.docs.get(n); return !!x && isEdited(x); });
      b.querySelector(".seg-dot")!.classList.toggle("hidden", !dot);
    }
    $("low-hint").classList.toggle("hidden", !this.bundle || this.detail !== "low");
    $("outfit-shared").classList.toggle("hidden", !bike || !(this.bundle?.shares ?? []).some((l) => l.mapped > 0));
  }
  private showModel(m: ModelData) {
    if (!this.preview.ok) return;
    if (this.preview.current !== m) {
      this.geos.clear();
      this.owners.clear();
    }
    const i = this.bundle?.models.indexOf(m) ?? -1;
    this.modelIndex = i >= 0 ? i : this.bundle?.models.indexOf(this.outfitModels(this.vehicle, this.detail)[0]) ?? 0;
    this.preview.show(m);
    this.p3dMessage(null);
    $("p3d-label").textContent = this.modelLabel(m);
    for (const d of this.docs.values()) if (!this.eyeGroupOf(d.name) && this.preview.hasTexture(d.name)) this.preview.update(d.name);
    this.refreshEyePreview();
  }
  showModelIndex(i: number) {
    const m = this.bundle?.models[i];
    if (!m) return;
    if (m.kind === "race") this.setOutfit(this.vehicleOf(m), this.detailOf(m));
    else this.showModel(m);
  }

  setMode(m: Mode) {
    if (m === "3d" && (!this.preview.ok || this.modelError)) m = "2d";
    this.mode = m;
    const p3d = document.querySelector<HTMLElement>(".p3d")!;
    if (m === "3d") $("ed-stage").prepend(p3d);
    else document.querySelector(".ed-right")!.prepend(p3d);
    this.preview.setPaintMode(m === "3d");
    this.preview.reparented();
    this.syncModeUi();
    if (!TOOLS_3D.includes(this.tool) && m === "3d") this.setTool("brush");
    else this.setTool(this.tool);
    this.renderEyePanel();
    if (m === "2d") {
      this.setBlink(false);
      requestAnimationFrame(() => this.view.layout());
      if (this.current) this.select(this.current.name);
    } else if (this.bundle) {
      this.setOutfit(this.vehicle, this.detail);
      this.refreshEyePreview();
    }
  }
  private syncModeUi() {
    const m = this.mode;
    $("view-edit").dataset.mode = m;
    $("mode-3d").setAttribute("aria-selected", String(m === "3d"));
    $("mode-2d").setAttribute("aria-selected", String(m === "2d"));
    document.querySelector(".mode-switch")?.setAttribute("aria-label", t("mode.label"));
    $("ed-tools").querySelectorAll<HTMLButtonElement>(".tool").forEach((b) => {
      const id = b.dataset.tool as Tool;
      b.classList.toggle("hidden", m === "3d" && !TOOLS_3D.includes(id));
      b.title = m === "3d" && id === "wand" ? t("tool.part") : t(`tool.${id}` as Key);
    });
    const cur = this.current;
    const x = cur && this.character?.textures.find((tx) => tx.name === cur.name);
    $("eye-warn").classList.toggle("hidden", !x?.eye || m === "3d");
    $("st-warn").textContent = "";
  }

  private geoOf(si: number): SurfaceGeo | null {
    let g = this.geos.get(si);
    const s = this.preview.subMeshes[si];
    if (!s) return null;
    if (!g || g.sub !== s) {
      g = new SurfaceGeo(s);
      this.geos.set(si, g);
    }
    return g;
  }
  private ownersOf(si: number, w: number, h: number) {
    const key = `${si}:${w}x${h}`;
    let o = this.owners.get(key);
    if (!o) {
      const g = this.geoOf(si);
      if (!g) return null;
      o = new TexelOwners(g, w, h);
      this.owners.set(key, o);
    }
    return o;
  }
  docsForTexture(texture: string): Doc[] {
    const g = this.eyeGroupOf(texture) ?? this.bundle?.eyes?.find((e) => e.frames[0] === texture);
    const names = g ? (this.eyeMode === "all" ? g.frames : [this.eyeMode]) : [texture];
    return names.map((n) => this.docs.get(n)).filter((d): d is Doc => !!d);
  }
  private shownDoc(texture: string) {
    const g = this.bundle?.eyes?.find((e) => e.frames[0] === texture || e.frames.includes(texture));
    return this.docs.get(g ? this.displayedFrame(g) : texture) ?? null;
  }
  private brushRadius(hit: SurfaceHit) {
    const px = this.tool === "eraser" ? this.opts.size3d.eraser : this.opts.size3d.brush;
    return (px / 2) * this.preview.worldPerPixel(hit.distance);
  }

  private bind3d() {
    const cv = this.preview.renderer?.domElement;
    if (!cv) return;
    cv.addEventListener("pointerdown", (e) => this.down3d(e));
    cv.addEventListener("pointermove", (e) => this.move3d(e));
    cv.addEventListener("pointerup", (e) => this.up3d(e));
    cv.addEventListener("pointercancel", (e) => this.up3d(e));
    cv.addEventListener("pointerleave", () => {
      if (this.mode === "3d") $("st-part").textContent = "";
    });
  }
  private down3d(e: PointerEvent) {
    if (this.mode !== "3d" || e.altKey || this.tool === "hand" || this.preview.navigating || (e.button !== 0 && e.button !== 2) || !this.bundle) return;
    if (e.button === 2 && this.tool !== "picker") return;
    const hit = this.preview.hitAt(e.clientX, e.clientY);
    if (!hit) return;
    const right = e.button === 2;
    if (this.tool === "picker" || this.tool === "wand" || this.tool === "fill") {
      this.pendingClick = { hit, right };
      return;
    }
    if (this.tool !== "brush" && this.tool !== "eraser") return;
    this.stroke3d = { targets: new Map(), last: [e.clientX, e.clientY], color: this.color.primary, tool: this.tool, mirror: new Map() };
    this.lastStroke3d = {};
    const first = this.docsForTexture(hit.texture)[0];
    if (first && first !== this.current) this.select(first.name);
    this.preview.setMirror(null);
    this.stamp3d(hit);
  }
  private move3d(e: PointerEvent) {
    if (this.mode !== "3d") return;
    const st = this.stroke3d;
    if (st && e.buttons & 1) {
      const [lx, ly] = st.last;
      const size = st.tool === "eraser" ? this.opts.size3d.eraser : this.opts.size3d.brush;
      const step = Math.max(1, size * 0.25);
      const n = Math.max(1, Math.ceil(Math.hypot(e.clientX - lx, e.clientY - ly) / step));
      let lastHit: SurfaceHit | null = null;
      for (let i = 1; i <= n; i++) {
        const hit = this.preview.hitAt(lx + ((e.clientX - lx) * i) / n, ly + ((e.clientY - ly) * i) / n);
        if (hit) {
          this.stamp3d(hit);
          lastHit = hit;
        }
      }
      st.last = [e.clientX, e.clientY];
      if (lastHit) this.preview.setRing(lastHit, this.brushRadius(lastHit));
      return;
    }
    if (e.buttons) return;
    const hit = this.preview.hitAt(e.clientX, e.clientY);
    if (!hit) {
      this.preview.setRing(null);
      $("st-part").textContent = "";
      return;
    }
    const paints = this.tool === "brush" || this.tool === "eraser";
    this.preview.setRing(hit, paints ? this.brushRadius(hit) : this.preview.worldPerPixel(hit.distance) * 6);
    const d = this.shownDoc(hit.texture);
    this.statusHit = d ? { name: d.name, tex: this.texParts(d.name) ? d.name : hit.texture, part: hit.part } : null;
    this.renderStatusHit();
  }
  private statusHit: { name: string; tex: string; part: number } | null = null;
  private renderStatusHit() {
    const h = this.statusHit;
    if (!h) return;
    const label = h.part !== 0xffff ? this.partLabel(h.tex, h.part) : "";
    $("st-part").textContent = [this.surfaceName(h.name), label].filter(Boolean).join(" · ");
  }
  private up3d(_e: PointerEvent) {
    const pc = this.pendingClick;
    this.pendingClick = null;
    if (pc && this.mode === "3d") {
      if (this.tool === "picker") this.pick3d(pc.hit, pc.right);
      else if (this.tool === "wand") this.part3d(pc.hit);
      else if (this.tool === "fill") this.fill3d(pc.hit);
    }
    const st = this.stroke3d;
    if (!st) return;
    this.stroke3d = null;
    this.history.beginGroup();
    for (const tg of st.targets.values()) {
      this.history.pixels(tg.doc, tg.layer, tg.snapshot, tg.stroke.rect());
      this.lastStroke3d[tg.doc.name] = Math.round(tg.stroke.cov.reduce((a, v) => a + (v > 0 ? 1 : 0), 0));
    }
    this.history.endGroup();
    if (st.tool === "brush") this.color.use(st.color);
    this.renderUndo();
    for (const tg of st.targets.values()) this.changed(tg.doc);
    if (st.mirror.size) window.setTimeout(() => !this.stroke3d && this.preview.setMirror(null), 1600);
  }
  private cancelStroke3d() {
    this.pendingClick = null;
    const st = this.stroke3d;
    if (!st) return;
    this.stroke3d = null;
    for (const tg of st.targets.values()) {
      tg.layer.pixels!.set(tg.snapshot);
      this.changed(tg.doc);
    }
    this.preview.setMirror(null);
  }
  stamp3d(hit: SurfaceHit) {
    const st = this.stroke3d;
    const geo = this.geoOf(hit.sub);
    if (!st || !geo) return;
    const radius = this.brushRadius(hit);
    const opts = st.tool === "eraser" ? this.opts.eraser : this.opts.brush;
    const docs = this.docsForTexture(hit.texture);
    docs.forEach((d, di) => {
      let tg = st.targets.get(d.name);
      if (!tg) {
        const L = this.paintable(d, d !== this.current);
        if (!L) return;
        const mode: PaintMode = st.tool === "brush" ? { kind: "paint", color: st.color } : this.opts.eraseOriginal && L.base ? { kind: "restore", original: d.original } : { kind: "erase" };
        tg = { doc: d, layer: L, snapshot: L.pixels!.slice(), stroke: new Stroke(d.w, d.h, opts), mode };
        st.targets.set(d.name, tg);
      }
      const painted: number[] = [];
      const touched = stampSphere(geo, hit.point, hit.normal, d.w, d.h, { radius, hardness: opts.hardness }, (i, a) => {
        tg!.stroke.cover(i, a);
        if (di === 0) painted.push(i);
      });
      applyStroke(tg.layer.pixels!, tg.snapshot, tg.stroke, tg.mode, d.selection);
      this.changed(d);
      if (di === 0 && painted.length) this.trackMirror(hit, geo, d, painted, touched, radius);
    });
  }
  private trackMirror(hit: SurfaceHit, geo: SurfaceGeo, d: Doc, painted: number[], touched: number[], radius: number) {
    const st = this.stroke3d!;
    if (!this.opts.showMirror) return;
    const own = this.ownersOf(hit.sub, d.w, d.h);
    if (!own) return;
    const near = new Set(touched);
    let set = st.mirror.get(hit.sub);
    if (!set) st.mirror.set(hit.sub, (set = new Set()));
    const before = set.size;
    const seen = new Set<number>();
    for (const i of painted) {
      if (seen.has(i)) continue;
      seen.add(i);
      own.forEach(i, (tri) => {
        if (near.has(tri) || set!.has(tri)) return;
        const cx = geo.centroids[tri * 3] - hit.point.x, cy = geo.centroids[tri * 3 + 1] - hit.point.y, cz = geo.centroids[tri * 3 + 2] - hit.point.z;
        if (Math.hypot(cx, cy, cz) > 2 * radius + geo.radii[tri]) set!.add(tri);
      });
    }
    if (set.size !== before) {
      this.preview.setMirror([...st.mirror].map(([sub, tris]) => ({ sub, tris: [...tris] })));
      if (!this.mirrorHintShown) {
        this.mirrorHintShown = true;
        $("mirror-hint").classList.remove("hidden");
        window.setTimeout(() => $("mirror-hint").classList.add("hidden"), 4500);
      }
    }
  }
  private fill3d(hit: SurfaceHit) {
    const geo = this.geoOf(hit.sub);
    if (!geo) return;
    const color = this.color.primary;
    this.history.beginGroup();
    for (const d of this.docsForTexture(hit.texture)) {
      const L = this.paintable(d, d !== this.current);
      if (!L) continue;
      const mask = hit.part !== 0xffff ? partMaskOf(geo, hit.part, d.w, d.h) : null;
      if (!mask) continue;
      const snap = L.pixels!.slice();
      const rect = fillMask(L.pixels!, d.w, d.h, mask, color, this.opts.fillOpacity, d.selection);
      this.history.pixels(d, L, snap, rect);
      this.changed(d);
    }
    this.history.endGroup();
    this.color.use(color);
    this.renderUndo();
  }
  private pick3d(hit: SurfaceHit, secondary: boolean) {
    const d = this.shownDoc(hit.texture);
    const s = this.preview.subMeshes[hit.sub];
    if (!d || !s) return;
    const x = wrapTexel(Math.floor(hit.uv[0] * d.w), d.w, s.data.wrapS), y = wrapTexel(Math.floor(hit.uv[1] * d.h), d.h, s.data.wrapT);
    const c = composite(d);
    const o = (y * d.w + x) * 4;
    this.color.set([c[o], c[o + 1], c[o + 2], c[o + 3]], secondary);
  }
  private part3d(hit: SurfaceHit) {
    const d = this.shownDoc(hit.texture);
    if (!d || hit.part === 0xffff) return;
    if (d !== this.current) this.select(d.name);
    this.selectPart({ texture: d.name, index: hit.part });
  }
  paintScreen(points: [number, number][]) {
    if (!points.length) return false;
    const h0 = this.preview.hitAt(points[0][0], points[0][1]);
    if (!h0) return false;
    this.down3d(new PointerEvent("pointerdown", { clientX: points[0][0], clientY: points[0][1], button: 0, buttons: 1 }));
    for (const p of points.slice(1)) this.move3d(new PointerEvent("pointermove", { clientX: p[0], clientY: p[1], buttons: 1 }));
    this.up3d(new PointerEvent("pointerup", { clientX: points[points.length - 1][0], clientY: points[points.length - 1][1] }));
    return true;
  }

  undo() {
    const d = this.history.undo();
    if (d && d !== this.current) this.select(d.name);
    this.afterHistory();
  }
  redo() {
    const d = this.history.redo();
    if (d && d !== this.current) this.select(d.name);
    this.afterHistory();
  }
  private afterHistory() {
    this.refreshSelectionOutline();
    this.renderLayers();
  }
  private renderUndo() {
    ($("btn-undo") as HTMLButtonElement).disabled = !this.history.canUndo;
    ($("btn-redo") as HTMLButtonElement).disabled = !this.history.canRedo;
  }

  thumb(name: string): string {
    let u = this.thumbs.get(name);
    if (u) return u;
    const d = this.docs.get(name);
    if (!d) return "";
    const c = document.createElement("canvas");
    c.width = d.w;
    c.height = d.h;
    c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(this.pixelsOf(d)), d.w, d.h), 0, 0);
    u = c.toDataURL();
    this.thumbs.set(name, u);
    return u;
  }
  edited(name: string) {
    const d = this.docs.get(name);
    return !!d && this.effectiveEdited(d);
  }
  private renderSwitcherButton() {
    const d = this.current;
    const x = d && this.character?.textures.find((t) => t.name === d.name);
    if (!d || !x || this.docs.get(d.name) !== d) return;
    $("tex-switch-thumb").innerHTML = `<img src="${this.thumb(d.name)}" alt="" />`;
    $("tex-switch-name").textContent = this.surfaceName(d.name);
    $("tex-switch-meta").textContent = `${d.name} · ${x.width}×${x.height} · ${x.format}${this.edited(d.name) ? " · " + t("edit.edited") : ""}`;
    $("lay-surface").textContent = `· ${this.surfaceName(d.name)}`;
  }
  private renderSwitcher() {
    const pop = $("tex-pop");
    const ch = this.character;
    if (!ch) return;
    pop.innerHTML = `<ul class="tex-grid" role="listbox">${ch.textures
      .map((x) => {
        const ed = this.edited(x.name);
        const xd = this.docs.get(x.name);
        const derived = !!this.linkOf(x.name) || (!!this.shareOf(x.name) && !!xd && !isEdited(xd) && ed);
        const badges = [ed ? `<i class="b ed">${esc(t("edit.edited"))}</i>` : "", derived ? `<i class="b">${esc(t("edit.auto"))}</i>` : "", x.menu ? `<i class="b">${esc(t("edit.menu"))}</i>` : ""].join("");
        return `<li role="option" tabindex="0" data-name="${esc(x.name)}" class="${x.name === this.current?.name ? "on" : ""}" aria-selected="${x.name === this.current?.name}"><span class="thumb checker"><img src="${this.thumb(x.name)}" alt="" /></span><b>${esc(this.surfaceName(x.name))}</b><small class="tech">${esc(x.name)} · ${x.width}×${x.height}</small><span class="badges">${badges}</span></li>`;
      })
      .join("")}</ul>`;
    pop.querySelectorAll<HTMLElement>("li[data-name]").forEach((li) => {
      const go = () => {
        pop.classList.add("hidden");
        this.select(li.dataset.name!);
      };
      li.addEventListener("click", go);
      li.addEventListener("keydown", (e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), go()));
    });
  }

  private hoverAt(x: number, y: number | null) {
    const d = this.current;
    if (!d || y === null) {
      $("st-pos").textContent = "";
      if (this.hover) {
        this.hover = null;
        this.preview.setHighlight(null, false);
        this.renderHover();
      }
      return;
    }
    const ix = Math.floor(x), iy = Math.floor(y);
    const inside = ix >= 0 && iy >= 0 && ix < d.w && iy < d.h;
    $("st-pos").textContent = inside ? `${ix}, ${iy}` : "";
    const c = inside ? this.sample(x, y) : null;
    $("st-pixel").textContent = c ? hex(c) : "";
    const part = inside ? this.partAt(x, y) : null;
    const unused = inside && !!this.unusedMask(d.name)?.[iy * d.w + ix];
    $("st-part").textContent = unused ? t("adv.unused") : part !== null ? this.partLabel(d.name, part) : "";
    this.view.canvas.title = unused ? t("adv.unused") : "";
    const h = part !== null ? { texture: d.name, index: part } : null;
    if (h?.index !== this.hover?.index || h?.texture !== this.hover?.texture) {
      this.hover = h;
      this.preview.setHighlight(h ? this.pickOf(h) : null, false);
      this.renderHover();
    }
  }
  private renderStatusColor() {
    this.renderReplace();
  }
  renderStatus() {
    const d = this.current;
    if (!d) return;
    $("st-size").textContent = `${d.w}×${d.h}`;
  }

  private bindKeys() {
    const spaceUp = () => this.preview.tempHand && this.preview.setHand(false, true);
    window.addEventListener("keyup", (e) => e.code === "Space" && spaceUp());
    window.addEventListener("blur", spaceUp);
    window.addEventListener("keydown", (e) => {
      if (!document.body.classList.contains("editing") || isTyping(e)) return;
      if (e.code === "Space") {
        if ((e.target as HTMLElement | null)?.closest?.("button,select,summary,[role=option],[role=radio],[role=tab]")) return;
        e.preventDefault();
        if (this.mode === "3d" && !this.preview.tempHand) this.preview.setHand(true, true);
        return;
      }
      const k = e.key.toLowerCase();
      const ctrl = e.ctrlKey || e.metaKey;
      if (ctrl && k === "z") {
        e.preventDefault();
        if (e.shiftKey) this.redo();
        else this.undo();
      } else if (ctrl && k === "y") {
        e.preventDefault();
        this.redo();
      } else if (ctrl && k === "a") {
        e.preventDefault();
        this.selectAll();
      } else if (ctrl && k === "d") {
        e.preventDefault();
        this.setSelection(null);
      } else if (ctrl && k === "s") {
        e.preventDefault();
        this.onSaveShortcut();
      } else if (ctrl && k === "o") {
        e.preventDefault();
        this.onOpenShortcut();
      } else if (ctrl && (k === "=" || k === "+")) {
        e.preventDefault();
        this.view.setZoom(nextZoom(this.view.zoom, 1));
      } else if (ctrl && k === "-") {
        e.preventDefault();
        this.view.setZoom(nextZoom(this.view.zoom, -1));
      } else if (ctrl && k === "0") {
        e.preventDefault();
        this.view.setFit();
      } else if (!ctrl && !e.altKey) {
        const tool = TOOLS.find((x) => x.key.toLowerCase() === k);
        if (tool) {
          if (this.mode === "2d" || TOOLS_3D.includes(tool.id)) this.setTool(tool.id);
        } else if (k === "x") this.color.swap();
        else if ((k === "[" || k === "]") && this.mode === "3d") {
          const key = this.tool === "eraser" ? "eraser" : "brush";
          const v = this.opts.size3d[key];
          this.opts.size3d[key] = Math.max(2, Math.min(120, v + (k === "]" ? Math.max(1, Math.round(v * 0.15)) : -Math.max(1, Math.round(v * 0.15)))));
          this.setTool(this.tool);
        } else if (k === "[" || k === "]") {
          const b = this.tool === "eraser" ? this.opts.eraser : this.tool === "line" ? this.opts.line : this.opts.brush;
          b.size = Math.max(1, Math.min(64, b.size + (k === "]" ? (b.size >= 10 ? 2 : 1) : b.size > 10 ? -2 : -1)));
          this.setTool(this.tool);
        } else if (k === "delete" || k === "backspace") this.clearSelectionPixels();
        else if (k === "?" || k === "f1") togglePop("help-pop", $("btn-help"));
        else if (k === "escape") {
          $("tex-pop").classList.add("hidden");
          $("help-pop").classList.add("hidden");
          $("replace-pop").classList.add("hidden");
        }
      }
    });
  }
  onSaveShortcut: () => void = () => {};
  onOpenShortcut: () => void = () => {};

  private clearSelectionPixels() {
    const d = this.current;
    if (!d?.selection) return;
    const L = this.paintable();
    if (!L) return;
    const snap = L.pixels!.slice();
    for (let i = 0; i < d.selection.length; i++) if (d.selection[i]) L.pixels![i * 4 + 3] *= 1 - d.selection[i] / 255;
    this.history.pixels(d, L, snap);
    this.changed(d);
  }

  summary(d: Doc) {
    const user = d.layers.filter((l) => !l.base && l.visible);
    const link = this.linkOf(d.name);
    const share = this.shareOf(d.name);
    const from = link?.highs[0].texture ?? share?.highs[0].texture;
    return {
      layers: user.length,
      adjustments: user.filter((l) => l.kind === "adjust").length,
      baseEdited: baseEdited(d),
      sources: user.filter((l) => l.source).map((l) => l.source!),
      ...(from && !isEdited(d) ? { derivedFrom: from } : {}),
      ...(this.eyeGroupOf(d.name) ? { eyeFrame: true } : {}),
    };
  }
  async commitAll() {
    for (const d of this.docs.values()) {
      const ed = this.effectiveEdited(d);
      const png = ed ? await pngDataUrl(this.pixelsOf(d), d.w, d.h) : null;
      await invoke<boolean>("commit_texture", { name: d.name, png, summary: this.summary(d) });
    }
  }
  editedDocs() {
    return [...this.docs.values()].filter((d) => this.effectiveEdited(d));
  }
  relabel() {
    applyStatic($("view-edit"));
    this.syncModeUi();
    this.renderOutfitBar();
    this.renderEyePanel();
    this.renderToolOpts();
    this.renderLayers();
    this.renderParts();
    this.renderSwitcherButton();
    this.view.onZoom(this.view.zoom);
    if (this.bundle) this.showModelFor(this.current?.name ?? "");
  }
}

function drawThumb(c: HTMLCanvasElement, px: Uint8ClampedArray, w: number, h: number) {
  const g = c.getContext("2d")!;
  g.clearRect(0, 0, c.width, c.height);
  const tmp = document.createElement("canvas");
  tmp.width = w;
  tmp.height = h;
  tmp.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(px), w, h), 0, 0);
  const s = Math.min(c.width / w, c.height / h);
  g.imageSmoothingEnabled = false;
  g.drawImage(tmp, (c.width - w * s) / 2, (c.height - h * s) / 2, w * s, h * s);
}
function colorName(c: [number, number, number] | null): string | null {
  if (!c) return null;
  const [r, g, b] = c.map((v) => v / 255);
  const max = Math.max(r, g, b), min = Math.min(r, g, b), l = (max + min) / 2, d = max - min;
  const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
  if (l < 0.12) return "black";
  if (s < 0.18) return l > 0.82 ? "white" : l < 0.3 ? "black" : "gray";
  let h = max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  h *= 60;
  if (h < 15 || h >= 340) return l < 0.3 ? "brown" : "red";
  if (h < 45) return l < 0.45 ? "brown" : l > 0.62 ? "beige" : "orange";
  if (h < 68) return l < 0.35 ? "brown" : "yellow";
  if (h < 165) return "green";
  if (h < 200) return "cyan";
  if (h < 255) return "blue";
  if (h < 290) return "purple";
  return "pink";
}
const fmtZoom = (z: number) => (z >= 1 ? `${Math.round(z)}×` : `${Math.round(z * 100)}%`);
function nextZoom(z: number, dir: number) {
  const steps = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32];
  return dir > 0 ? steps.find((s) => s > z + 0.01) ?? 32 : [...steps].reverse().find((s) => s < z - 0.01) ?? 1;
}
function togglePop(id: string, anchor: HTMLElement) {
  const pop = $(id);
  const willOpen = pop.classList.contains("hidden");
  document.querySelectorAll(".popover").forEach((p) => p.id !== "replace-pop" && p.classList.add("hidden"));
  if (!willOpen) return;
  pop.classList.remove("hidden");
  const r = anchor.getBoundingClientRect();
  const host = pop.offsetParent?.getBoundingClientRect() ?? { left: 0, top: 0 };
  const top = Math.max(4, Math.min(r.bottom + 4, window.innerHeight - pop.offsetHeight - 8));
  pop.style.top = `${top - host.top}px`;
  pop.style.left = `${Math.max(4, Math.min(r.left - host.left, window.innerWidth - pop.offsetWidth - 8 - host.left))}px`;
}
