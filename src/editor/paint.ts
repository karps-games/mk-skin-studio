export type RGBA = [number, number, number, number];

export interface BrushOpts {
  size: number;
  hardness: number;
  opacity: number;
  pixelPerfect: boolean;
}

export class Stroke {
  cov: Float32Array;
  x0 = Infinity;
  y0 = Infinity;
  x1 = -Infinity;
  y1 = -Infinity;
  last: [number, number] | null = null;
  constructor(public w: number, public h: number, public opts: BrushOpts) {
    this.cov = new Float32Array(w * h);
  }
  stamp(cx: number, cy: number) {
    const { size, hardness, pixelPerfect } = this.opts;
    const r = Math.max(0.5, size / 2);
    const ccx = pixelPerfect ? (size % 2 ? Math.floor(cx) + 0.5 : Math.round(cx)) : cx;
    const ccy = pixelPerfect ? (size % 2 ? Math.floor(cy) + 0.5 : Math.round(cy)) : cy;
    const xa = Math.max(0, Math.floor(ccx - r - 1)), xb = Math.min(this.w - 1, Math.ceil(ccx + r + 1));
    const ya = Math.max(0, Math.floor(ccy - r - 1)), yb = Math.min(this.h - 1, Math.ceil(ccy + r + 1));
    for (let y = ya; y <= yb; y++)
      for (let x = xa; x <= xb; x++) {
        const d = Math.hypot(x + 0.5 - ccx, y + 0.5 - ccy);
        let a: number;
        if (pixelPerfect || size <= 1.01) a = d < r || (size <= 1.01 && Math.floor(ccx) === x && Math.floor(ccy) === y) ? 1 : 0;
        else {
          const inner = r * hardness;
          a = d <= inner ? 1 : d >= r ? 0 : 1 - (d - inner) / Math.max(1e-3, r - inner);
          if (hardness >= 0.999) a = Math.max(0, Math.min(1, r + 0.5 - d));
        }
        if (a <= 0) continue;
        const i = y * this.w + x;
        if (a > this.cov[i]) this.cov[i] = a;
        if (x < this.x0) this.x0 = x;
        if (y < this.y0) this.y0 = y;
        if (x + 1 > this.x1) this.x1 = x + 1;
        if (y + 1 > this.y1) this.y1 = y + 1;
      }
  }
  cover(i: number, a: number) {
    if (a <= this.cov[i]) return;
    this.cov[i] = a;
    const x = i % this.w, y = (i / this.w) | 0;
    if (x < this.x0) this.x0 = x;
    if (y < this.y0) this.y0 = y;
    if (x + 1 > this.x1) this.x1 = x + 1;
    if (y + 1 > this.y1) this.y1 = y + 1;
  }
  to(x: number, y: number) {
    if (!this.last) {
      this.stamp(x, y);
      this.last = [x, y];
      return;
    }
    const [lx, ly] = this.last;
    const dist = Math.hypot(x - lx, y - ly);
    const step = this.opts.pixelPerfect || this.opts.size <= 2 ? 0.5 : Math.max(0.5, this.opts.size * 0.25);
    const n = Math.max(1, Math.ceil(dist / step));
    for (let i = 1; i <= n; i++) this.stamp(lx + ((x - lx) * i) / n, ly + ((y - ly) * i) / n);
    this.last = [x, y];
  }
  rect(): [number, number, number, number] {
    return [this.x0, this.y0, this.x1, this.y1];
  }
}

export type PaintMode = { kind: "paint"; color: RGBA } | { kind: "erase" } | { kind: "restore"; original: Uint8ClampedArray };

export function applyStroke(dst: Uint8ClampedArray, snapshot: Uint8ClampedArray, s: Stroke, mode: PaintMode, sel: Uint8Array | null) {
  const [x0, y0, x1, y1] = s.rect();
  for (let y = Math.max(0, y0); y < Math.min(s.h, y1); y++)
    for (let x = Math.max(0, x0); x < Math.min(s.w, x1); x++) {
      const i = y * s.w + x;
      let k = s.cov[i] * s.opts.opacity;
      if (sel) k *= sel[i] / 255;
      const o = i * 4;
      if (k <= 0) {
        dst[o] = snapshot[o];
        dst[o + 1] = snapshot[o + 1];
        dst[o + 2] = snapshot[o + 2];
        dst[o + 3] = snapshot[o + 3];
        continue;
      }
      paintPixel(dst, snapshot, o, k, mode);
    }
}

export function paintPixel(dst: Uint8ClampedArray, src: Uint8ClampedArray, o: number, k: number, mode: PaintMode) {
  if (mode.kind === "erase") {
    dst[o] = src[o];
    dst[o + 1] = src[o + 1];
    dst[o + 2] = src[o + 2];
    dst[o + 3] = src[o + 3] * (1 - k);
    return;
  }
  if (mode.kind === "restore") {
    for (let c = 0; c < 4; c++) dst[o + c] = src[o + c] + (mode.original[o + c] - src[o + c]) * k;
    return;
  }
  const c = mode.color;
  const ca = (c[3] / 255) * k;
  const ba = src[o + 3] / 255;
  const ao = ca + ba * (1 - ca);
  if (ao <= 0) {
    dst[o + 3] = 0;
    return;
  }
  for (let ch = 0; ch < 3; ch++) dst[o + ch] = (c[ch] * ca + src[o + ch] * ba * (1 - ca)) / ao;
  dst[o + 3] = ao * 255;
}

export function rectStroke(w: number, h: number, opts: BrushOpts, ax: number, ay: number, bx: number, by: number, fill: boolean): Stroke {
  const s = new Stroke(w, h, { ...opts, hardness: 1 });
  const x0 = Math.max(0, Math.floor(Math.min(ax, bx))), x1 = Math.min(w - 1, Math.floor(Math.max(ax, bx)));
  const y0 = Math.max(0, Math.floor(Math.min(ay, by))), y1 = Math.min(h - 1, Math.floor(Math.max(ay, by)));
  const t = Math.max(1, Math.round(opts.size));
  for (let y = y0; y <= y1; y++)
    for (let x = x0; x <= x1; x++) {
      if (!fill && x - x0 >= t && x1 - x >= t && y - y0 >= t && y1 - y >= t) continue;
      s.cov[y * w + x] = 1;
    }
  s.x0 = x0;
  s.y0 = y0;
  s.x1 = x1 + 1;
  s.y1 = y1 + 1;
  return s;
}

export function lineStroke(w: number, h: number, opts: BrushOpts, ax: number, ay: number, bx: number, by: number): Stroke {
  const s = new Stroke(w, h, opts);
  s.to(ax, ay);
  s.to(bx, by);
  return s;
}

const dist = (p: Uint8ClampedArray, o: number, c: RGBA) => Math.max(Math.abs(p[o] - c[0]), Math.abs(p[o + 1] - c[1]), Math.abs(p[o + 2] - c[2]), Math.abs(p[o + 3] - c[3]));

export function regionMask(px: Uint8ClampedArray, w: number, h: number, sx: number, sy: number, tolerance: number, contiguous: boolean, limit: Uint8Array | null): Uint8Array {
  const out = new Uint8Array(w * h);
  if (sx < 0 || sy < 0 || sx >= w || sy >= h) return out;
  const so = (sy * w + sx) * 4;
  const seed: RGBA = [px[so], px[so + 1], px[so + 2], px[so + 3]];
  const ok = (i: number) => (!limit || limit[i] > 0) && dist(px, i * 4, seed) <= tolerance;
  if (!contiguous) {
    for (let i = 0; i < w * h; i++) if (ok(i)) out[i] = 255;
    return out;
  }
  const start = sy * w + sx;
  if (!ok(start)) return out;
  const stack = [start];
  out[start] = 255;
  while (stack.length) {
    const i = stack.pop()!;
    const x = i % w, y = (i / w) | 0;
    const nb = [x > 0 ? i - 1 : -1, x < w - 1 ? i + 1 : -1, y > 0 ? i - w : -1, y < h - 1 ? i + w : -1];
    for (const j of nb)
      if (j >= 0 && !out[j] && ok(j)) {
        out[j] = 255;
        stack.push(j);
      }
  }
  return out;
}

export function fillMask(dst: Uint8ClampedArray, w: number, h: number, mask: Uint8Array, color: RGBA, opacity: number, sel: Uint8Array | null): [number, number, number, number] {
  let x0 = w, y0 = h, x1 = 0, y1 = 0;
  const src = dst.slice();
  for (let i = 0; i < w * h; i++) {
    let k = (mask[i] / 255) * opacity;
    if (sel) k *= sel[i] / 255;
    if (k <= 0) continue;
    paintPixel(dst, src, i * 4, k, { kind: "paint", color });
    const x = i % w, y = (i / w) | 0;
    if (x < x0) x0 = x;
    if (y < y0) y0 = y;
    if (x + 1 > x1) x1 = x + 1;
    if (y + 1 > y1) y1 = y + 1;
  }
  return [x0, y0, x1, y1];
}

const luma = (r: number, g: number, b: number) => 0.299 * r + 0.587 * g + 0.114 * b;

export function replaceColor(composite: Uint8ClampedArray, w: number, h: number, from: RGBA, to: RGBA, tolerance: number, limit: Uint8Array | null): Uint8ClampedArray | null {
  const out = new Uint8ClampedArray(w * h * 4);
  const fl = luma(from[0], from[1], from[2]);
  let any = false;
  for (let i = 0; i < w * h; i++) {
    const o = i * 4;
    if (limit && !limit[i]) continue;
    if (composite[o + 3] === 0) continue;
    const d = Math.max(Math.abs(composite[o] - from[0]), Math.abs(composite[o + 1] - from[1]), Math.abs(composite[o + 2] - from[2]));
    if (d > tolerance) continue;
    const soft = tolerance > 0 ? Math.min(1, ((tolerance - d) / Math.max(1, tolerance * 0.25))) : 1;
    const shade = luma(composite[o], composite[o + 1], composite[o + 2]) - fl;
    out[o] = to[0] + shade;
    out[o + 1] = to[1] + shade;
    out[o + 2] = to[2] + shade;
    out[o + 3] = composite[o + 3] * soft * (limit ? limit[i] / 255 : 1);
    any = true;
  }
  return any ? out : null;
}

export function palette(px: Uint8ClampedArray, k = 12): RGBA[] {
  const pts: number[][] = [];
  const n = px.length / 4;
  const step = Math.max(1, Math.floor(n / 6000));
  for (let i = 0; i < n; i += step) if (px[i * 4 + 3] > 127) pts.push([px[i * 4], px[i * 4 + 1], px[i * 4 + 2]]);
  if (!pts.length) return [];
  const d2 = (a: number[], b: number[]) => (a[0] - b[0]) ** 2 + (a[1] - b[1]) ** 2 + (a[2] - b[2]) ** 2;
  const centers = [pts[0].slice()];
  while (centers.length < Math.min(k, pts.length)) {
    let best = 0, bi = 0;
    for (let i = 0; i < pts.length; i++) {
      const m = Math.min(...centers.map((c) => d2(c, pts[i])));
      if (m > best) (best = m), (bi = i);
    }
    if (best === 0) break;
    centers.push(pts[bi].slice());
  }
  const assign = new Int32Array(pts.length);
  for (let it = 0; it < 8; it++) {
    for (let i = 0; i < pts.length; i++) {
      let bi = 0, bd = Infinity;
      for (let c = 0; c < centers.length; c++) {
        const d = d2(centers[c], pts[i]);
        if (d < bd) (bd = d), (bi = c);
      }
      assign[i] = bi;
    }
    const sum = centers.map(() => [0, 0, 0, 0]);
    for (let i = 0; i < pts.length; i++) {
      const s = sum[assign[i]];
      s[0] += pts[i][0];
      s[1] += pts[i][1];
      s[2] += pts[i][2];
      s[3]++;
    }
    sum.forEach((s, c) => s[3] && (centers[c] = [s[0] / s[3], s[1] / s[3], s[2] / s[3]]));
  }
  const counts = centers.map((_, c) => assign.filter((a) => a === c).length);
  return centers
    .map((c, i) => ({ c, n: counts[i] }))
    .filter((x) => x.n > 0)
    .sort((a, b) => b.n - a.n)
    .map(({ c }) => [Math.round(c[0]), Math.round(c[1]), Math.round(c[2]), 255] as RGBA);
}

export function maskFromRuns(runs: Uint32Array, n: number): Uint8Array {
  const m = new Uint8Array(n);
  for (let i = 0; i + 1 < runs.length; i += 2) m.fill(255, runs[i], Math.min(n, runs[i] + runs[i + 1]));
  return m;
}

export function maskOutline(m: Uint8Array, w: number, h: number): number[] {
  const at = (x: number, y: number) => x >= 0 && y >= 0 && x < w && y < h && m[y * w + x] > 127;
  const out: number[] = [];
  for (let y = 0; y <= h; y++) {
    let start = -1, kind = 0;
    for (let x = 0; x <= w; x++) {
      const k = x < w ? Number(at(x, y - 1)) - Number(at(x, y)) : 0;
      if (k !== kind) {
        if (start >= 0) out.push(start, y, x, y);
        start = k ? x : -1;
        kind = k;
      }
    }
  }
  for (let x = 0; x <= w; x++) {
    let start = -1, kind = 0;
    for (let y = 0; y <= h; y++) {
      const k = y < h ? Number(at(x - 1, y)) - Number(at(x, y)) : 0;
      if (k !== kind) {
        if (start >= 0) out.push(x, start, x, y);
        start = k ? y : -1;
        kind = k;
      }
    }
  }
  return out;
}

export function maskBBox(m: Uint8Array, w: number, h: number): [number, number, number, number] | null {
  let x0 = w, y0 = h, x1 = -1, y1 = -1;
  for (let i = 0; i < w * h; i++)
    if (m[i]) {
      const x = i % w, y = (i / w) | 0;
      if (x < x0) x0 = x;
      if (x > x1) x1 = x;
      if (y < y0) y0 = y;
      if (y > y1) y1 = y;
    }
  return x1 < 0 ? null : [x0, y0, x1 + 1, y1 + 1];
}

export const hex = (c: RGBA) => "#" + [c[0], c[1], c[2]].map((v) => v.toString(16).padStart(2, "0")).join("") + (c[3] < 255 ? c[3].toString(16).padStart(2, "0") : "");
export function parseHex(s: string): RGBA | null {
  const m = /^#?([0-9a-f]{6})([0-9a-f]{2})?$/i.exec(s.trim()) ?? /^#?([0-9a-f]{3})$/i.exec(s.trim());
  if (!m) return null;
  let h = m[1];
  if (h.length === 3) h = h.replace(/./g, (c) => c + c);
  return [parseInt(h.slice(0, 2), 16), parseInt(h.slice(2, 4), 16), parseInt(h.slice(4, 6), 16), m[2] ? parseInt(m[2], 16) : 255];
}
