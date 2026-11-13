export type Blend = "normal" | "multiply" | "screen" | "overlay" | "color" | "hue";
export const BLENDS: Blend[] = ["normal", "multiply", "screen", "overlay", "color", "hue"];

export interface Adjust {
  hue: number;
  saturation: number;
  brightness: number;
  colorize: boolean;
}
export const identityAdjust = (): Adjust => ({ hue: 0, saturation: 0, brightness: 0, colorize: false });

export interface Layer {
  id: number;
  name: string;
  kind: "pixel" | "adjust";
  base: boolean;
  visible: boolean;
  locked: boolean;
  opacity: number;
  blend: Blend;
  pixels: Uint8ClampedArray | null;
  adjust: Adjust | null;
  mask: Uint8Array | null;
  source?: { name: string; sha256: string };
}

export interface Doc {
  name: string;
  w: number;
  h: number;
  original: Uint8ClampedArray;
  layers: Layer[];
  active: number;
  selection: Uint8Array | null;
  composite: Uint8ClampedArray;
  nextId: number;
  rev: number;
  compositeRev: number;
  sha256: string;
}

export function newDoc(name: string, w: number, h: number, original: Uint8ClampedArray, sha256: string): Doc {
  const base: Layer = { id: 1, name: "", kind: "pixel", base: true, visible: true, locked: true, opacity: 1, blend: "normal", pixels: original.slice(), adjust: null, mask: null };
  const paint: Layer = { id: 2, name: "", kind: "pixel", base: false, visible: true, locked: false, opacity: 1, blend: "normal", pixels: new Uint8ClampedArray(w * h * 4), adjust: null, mask: null };
  const d: Doc = { name, w, h, original, layers: [base, paint], active: 2, selection: null, composite: original.slice(), nextId: 3, rev: 0, compositeRev: 0, sha256 };
  return d;
}

export const activeLayer = (d: Doc) => d.layers.find((l) => l.id === d.active) ?? d.layers[d.layers.length - 1];

function hue2rgb(p: number, q: number, t: number) {
  if (t < 0) t += 1;
  if (t > 1) t -= 1;
  if (t < 1 / 6) return p + (q - p) * 6 * t;
  if (t < 1 / 2) return q;
  if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
  return p;
}
function hslToRgb(h: number, s: number, l: number): [number, number, number] {
  if (s === 0) return [l, l, l];
  const q = l < 0.5 ? l * (1 + s) : l + s - l * s;
  const p = 2 * l - q;
  const hh = (((h % 360) + 360) % 360) / 360;
  return [hue2rgb(p, q, hh + 1 / 3), hue2rgb(p, q, hh), hue2rgb(p, q, hh - 1 / 3)];
}
export function rgbToHsv(r: number, g: number, b: number): [number, number, number] {
  const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
  let h = 0;
  if (d) h = max === r ? 60 * (((g - b) / d + 6) % 6) : max === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
  return [h, max ? d / max : 0, max];
}
export function hsvToRgb(h: number, s: number, v: number): [number, number, number] {
  const c = v * s;
  const hp = (((h % 360) + 360) % 360) / 60;
  const x = c * (1 - Math.abs((hp % 2) - 1));
  const [r, g, b] = hp < 1 ? [c, x, 0] : hp < 2 ? [x, c, 0] : hp < 3 ? [0, c, x] : hp < 4 ? [0, x, c] : hp < 5 ? [x, 0, c] : [c, 0, x];
  const m = v - c;
  return [r + m, g + m, b + m];
}
const lum = (r: number, g: number, b: number) => 0.3 * r + 0.59 * g + 0.11 * b;
function setLum(r: number, g: number, b: number, l: number): [number, number, number] {
  const d = l - lum(r, g, b);
  r += d;
  g += d;
  b += d;
  const L = lum(r, g, b), n = Math.min(r, g, b), x = Math.max(r, g, b);
  if (n < 0) (r = L + ((r - L) * L) / (L - n)), (g = L + ((g - L) * L) / (L - n)), (b = L + ((b - L) * L) / (L - n));
  if (x > 1) (r = L + ((r - L) * (1 - L)) / (x - L)), (g = L + ((g - L) * (1 - L)) / (x - L)), (b = L + ((b - L) * (1 - L)) / (x - L));
  return [r, g, b];
}
const sat = (r: number, g: number, b: number) => Math.max(r, g, b) - Math.min(r, g, b);
function setSat(c: [number, number, number], s: number): [number, number, number] {
  const idx = [0, 1, 2].sort((a, b) => c[a] - c[b]);
  const out: [number, number, number] = [0, 0, 0];
  const [mn, md, mx] = idx;
  if (c[mx] > c[mn]) {
    out[md] = ((c[md] - c[mn]) * s) / (c[mx] - c[mn]);
    out[mx] = s;
  }
  out[mn] = 0;
  return out;
}

export function adjustRgb(r: number, g: number, b: number, a: Adjust): [number, number, number] {
  let R: number, G: number, B: number;
  const br = a.brightness / 100;
  if (a.colorize) {
    const l = lum(r, g, b);
    const s = Math.max(0, Math.min(1, 0.5 + a.saturation / 200));
    [R, G, B] = hslToRgb(a.hue + 180, s, l);
  } else {
    const [h, s, v] = rgbToHsv(r, g, b);
    [R, G, B] = hsvToRgb(h + a.hue, s, v);
    const k = 1 + a.saturation / 100;
    const L = 0.299 * R + 0.587 * G + 0.114 * B;
    R = L + (R - L) * k;
    G = L + (G - L) * k;
    B = L + (B - L) * k;
  }
  const f = (c: number) => {
    c = br >= 0 ? c + (1 - c) * br : c * (1 + br);
    return Math.max(0, Math.min(1, c));
  };
  return [f(R), f(G), f(B)];
}

function blendRgb(mode: Blend, cb: [number, number, number], cs: [number, number, number]): [number, number, number] {
  switch (mode) {
    case "multiply":
      return [cb[0] * cs[0], cb[1] * cs[1], cb[2] * cs[2]];
    case "screen":
      return [cb[0] + cs[0] - cb[0] * cs[0], cb[1] + cs[1] - cb[1] * cs[1], cb[2] + cs[2] - cb[2] * cs[2]];
    case "overlay":
      return cb.map((b, i) => (b <= 0.5 ? 2 * b * cs[i] : 1 - 2 * (1 - b) * (1 - cs[i]))) as [number, number, number];
    case "color":
      return setLum(cs[0], cs[1], cs[2], lum(cb[0], cb[1], cb[2]));
    case "hue": {
      const t = setSat(cs, sat(cb[0], cb[1], cb[2]));
      return setLum(t[0], t[1], t[2], lum(cb[0], cb[1], cb[2]));
    }
    default:
      return cs;
  }
}

export function composite(d: Doc): Uint8ClampedArray {
  if (d.compositeRev === d.rev && d.rev !== 0) return d.composite;
  const out = d.composite;
  out.fill(0);
  const n = d.w * d.h;
  for (const L of d.layers) {
    if (!L.visible || L.opacity <= 0) continue;
    if (L.kind === "adjust" && L.adjust) {
      const A = L.adjust;
      for (let i = 0; i < n; i++) {
        const m = (L.mask ? L.mask[i] / 255 : 1) * L.opacity;
        if (m <= 0) continue;
        const o = i * 4;
        if (out[o + 3] === 0) continue;
        const [r, g, b] = adjustRgb(out[o] / 255, out[o + 1] / 255, out[o + 2] / 255, A);
        out[o] += (r * 255 - out[o]) * m;
        out[o + 1] += (g * 255 - out[o + 1]) * m;
        out[o + 2] += (b * 255 - out[o + 2]) * m;
      }
      continue;
    }
    const P = L.pixels;
    if (!P) continue;
    const fast = L.blend === "normal";
    for (let i = 0; i < n; i++) {
      const o = i * 4;
      const sa = (P[o + 3] / 255) * L.opacity;
      if (sa <= 0) {
        if (L.base && out[o + 3] === 0) (out[o] = P[o]), (out[o + 1] = P[o + 1]), (out[o + 2] = P[o + 2]);
        continue;
      }
      const ba = out[o + 3] / 255;
      if (fast && sa >= 1) {
        out[o] = P[o];
        out[o + 1] = P[o + 1];
        out[o + 2] = P[o + 2];
        out[o + 3] = 255;
        continue;
      }
      let cs: [number, number, number] = [P[o] / 255, P[o + 1] / 255, P[o + 2] / 255];
      const cb: [number, number, number] = [out[o] / 255, out[o + 1] / 255, out[o + 2] / 255];
      if (!fast && ba > 0) {
        const bl = blendRgb(L.blend, cb, cs);
        cs = [(1 - ba) * cs[0] + ba * bl[0], (1 - ba) * cs[1] + ba * bl[1], (1 - ba) * cs[2] + ba * bl[2]];
      }
      const ao = sa + ba * (1 - sa);
      for (let c = 0; c < 3; c++) out[o + c] = ((cs[c] * sa + cb[c] * ba * (1 - sa)) / ao) * 255;
      out[o + 3] = ao * 255;
    }
  }
  d.compositeRev = d.rev;
  return out;
}

export function touch(d: Doc) {
  d.rev++;
  if (d.rev === d.compositeRev) d.rev++;
}

export const isEdited = (d: Doc) => {
  const c = composite(d);
  const o = d.original;
  for (let i = 0; i < c.length; i++) if (c[i] !== o[i]) return true;
  return false;
};

export function baseEdited(d: Doc) {
  const b = d.layers.find((l) => l.base)?.pixels;
  if (!b) return false;
  for (let i = 0; i < b.length; i++) if (b[i] !== d.original[i]) return true;
  return false;
}

interface Tile {
  x: number;
  y: number;
  w: number;
  h: number;
  before: Uint8ClampedArray;
  after: Uint8ClampedArray;
}
interface LayerSnap {
  layer: Layer;
  name: string;
  visible: boolean;
  locked: boolean;
  opacity: number;
  blend: Blend;
  adjust: Adjust | null;
  mask: Uint8Array | null;
  pixels: Uint8ClampedArray | null;
}
type Entry =
  | { kind: "pixels"; doc: Doc; layer: Layer; tiles: Tile[]; bytes: number; selBefore?: Uint8Array | null; selAfter?: Uint8Array | null }
  | { kind: "layers"; doc: Doc; before: LayerSnap[]; after: LayerSnap[]; activeBefore: number; activeAfter: number; bytes: number }
  | { kind: "group"; doc: Doc; entries: Entry[]; bytes: number };

const TILE = 32;
const MAX_BYTES = 128 << 20;
const MAX_ENTRIES = 300;

export class History {
  private undoStack: Entry[] = [];
  private redoStack: Entry[] = [];
  private bytes = 0;
  onChange: (doc: Doc) => void = () => {};

  get canUndo() {
    return this.undoStack.length > 0;
  }
  get canRedo() {
    return this.redoStack.length > 0;
  }
  get size() {
    return this.bytes;
  }
  clear() {
    this.undoStack = [];
    this.redoStack = [];
    this.bytes = 0;
  }
  forDoc(doc: Doc) {
    return this.undoStack.filter((e) => e.doc === doc).length;
  }

  private batch: Entry[] | null = null;
  beginGroup() {
    this.batch = [];
  }
  endGroup() {
    const b = this.batch;
    this.batch = null;
    if (!b || !b.length) return;
    if (b.length === 1) this.push(b[0]);
    else this.push({ kind: "group", doc: b[0].doc, entries: b, bytes: b.reduce((n, e) => n + e.bytes, 0) });
  }

  private push(e: Entry) {
    if (this.batch) {
      this.batch.push(e);
      return;
    }
    this.undoStack.push(e);
    this.bytes += e.bytes;
    for (const r of this.redoStack) this.bytes -= r.bytes;
    this.redoStack = [];
    while ((this.bytes > MAX_BYTES || this.undoStack.length > MAX_ENTRIES) && this.undoStack.length > 1) this.bytes -= this.undoStack.shift()!.bytes;
  }

  pixels(doc: Doc, layer: Layer, before: Uint8ClampedArray, rect?: [number, number, number, number], selBefore?: Uint8Array | null) {
    const after = layer.pixels!;
    const [rx0, ry0, rx1, ry1] = rect ?? [0, 0, doc.w, doc.h];
    const tiles: Tile[] = [];
    let bytes = 0;
    for (let ty = Math.floor(Math.max(0, ry0) / TILE) * TILE; ty < Math.min(doc.h, ry1); ty += TILE) {
      for (let tx = Math.floor(Math.max(0, rx0) / TILE) * TILE; tx < Math.min(doc.w, rx1); tx += TILE) {
        const w = Math.min(TILE, doc.w - tx), h = Math.min(TILE, doc.h - ty);
        let changed = false;
        for (let y = ty; y < ty + h && !changed; y++) {
          const s = (y * doc.w + tx) * 4;
          for (let i = s; i < s + w * 4; i++)
            if (before[i] !== after[i]) {
              changed = true;
              break;
            }
        }
        if (!changed) continue;
        const b = new Uint8ClampedArray(w * h * 4), a = new Uint8ClampedArray(w * h * 4);
        for (let y = 0; y < h; y++) {
          const s = ((ty + y) * doc.w + tx) * 4;
          b.set(before.subarray(s, s + w * 4), y * w * 4);
          a.set(after.subarray(s, s + w * 4), y * w * 4);
        }
        tiles.push({ x: tx, y: ty, w, h, before: b, after: a });
        bytes += b.length * 2;
      }
    }
    const selAfter = doc.selection ? doc.selection.slice() : null;
    const selChanged = selBefore !== undefined && !sameMask(selBefore, doc.selection);
    if (!tiles.length && !selChanged) return false;
    this.push({ kind: "pixels", doc, layer, tiles, bytes: bytes + (selChanged ? doc.w * doc.h * 2 : 0), ...(selChanged ? { selBefore, selAfter } : {}) });
    return true;
  }

  snapLayers(doc: Doc): LayerSnap[] {
    return doc.layers.map((l) => ({ layer: l, name: l.name, visible: l.visible, locked: l.locked, opacity: l.opacity, blend: l.blend, adjust: l.adjust ? { ...l.adjust } : null, mask: l.mask, pixels: l.pixels }));
  }
  layers(doc: Doc, before: LayerSnap[], activeBefore: number) {
    const after = this.snapLayers(doc);
    const same = before.length === after.length && before.every((b, i) => JSON.stringify({ ...b, layer: b.layer.id, mask: !!b.mask, pixels: !!b.pixels }) === JSON.stringify({ ...after[i], layer: after[i].layer.id, mask: !!after[i].mask, pixels: !!after[i].pixels }) && b.mask === after[i].mask && b.pixels === after[i].pixels);
    if (same && activeBefore === doc.active) return;
    this.push({ kind: "layers", doc, before, after, activeBefore, activeAfter: doc.active, bytes: 256 * after.length });
  }

  private apply(e: Entry, undo: boolean) {
    const d = e.doc;
    if (e.kind === "group") {
      const list = undo ? [...e.entries].reverse() : e.entries;
      for (const x of list) this.apply(x, undo);
      return;
    }
    if (e.kind === "pixels") {
      const px = e.layer.pixels!;
      for (const t of e.tiles) {
        const src = undo ? t.before : t.after;
        for (let y = 0; y < t.h; y++) px.set(src.subarray(y * t.w * 4, (y + 1) * t.w * 4), ((t.y + y) * d.w + t.x) * 4);
      }
      if ("selBefore" in e) d.selection = (undo ? e.selBefore : e.selAfter)?.slice() ?? null;
    } else {
      const snaps = undo ? e.before : e.after;
      d.layers = snaps.map((s) => {
        Object.assign(s.layer, { name: s.name, visible: s.visible, locked: s.locked, opacity: s.opacity, blend: s.blend, adjust: s.adjust ? { ...s.adjust } : null, mask: s.mask, pixels: s.pixels });
        return s.layer;
      });
      d.active = undo ? e.activeBefore : e.activeAfter;
      if (!d.layers.some((l) => l.id === d.active)) d.active = d.layers[d.layers.length - 1].id;
    }
    touch(d);
    this.onChange(d);
  }
  undo(): Doc | null {
    const e = this.undoStack.pop();
    if (!e) return null;
    this.redoStack.push(e);
    this.apply(e, true);
    return e.doc;
  }
  redo(): Doc | null {
    const e = this.redoStack.pop();
    if (!e) return null;
    this.undoStack.push(e);
    this.apply(e, false);
    return e.doc;
  }
}

function sameMask(a: Uint8Array | null | undefined, b: Uint8Array | null | undefined) {
  if (!a || !b) return !a && !b;
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}
