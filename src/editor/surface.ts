import { vertexNormal, type Sub } from "./preview3d";

export interface Vec3 {
  x: number;
  y: number;
  z: number;
}

export function wrapTexel(i: number, n: number, mode: number) {
  if (mode === 1) return ((i % n) + n) % n;
  if (mode === 2) {
    const m = ((i % (2 * n)) + 2 * n) % (2 * n);
    return m < n ? m : 2 * n - 1 - m;
  }
  return Math.max(0, Math.min(n - 1, i));
}

export class SurfaceGeo {
  normals: Float32Array;
  centroids: Float32Array;
  radii: Float32Array;
  constructor(public sub: Sub) {
    const n = sub.index.length / 3;
    this.normals = new Float32Array(n * 3);
    this.centroids = new Float32Array(n * 3);
    this.radii = new Float32Array(n);
    const P = sub.pos;
    for (let t = 0; t < n; t++) {
      const a = sub.index[t * 3] * 3, b = sub.index[t * 3 + 1] * 3, c = sub.index[t * 3 + 2] * 3;
      const ux = P[b] - P[a], uy = P[b + 1] - P[a + 1], uz = P[b + 2] - P[a + 2];
      const vx = P[c] - P[a], vy = P[c + 1] - P[a + 1], vz = P[c + 2] - P[a + 2];
      let nx = uy * vz - uz * vy, ny = uz * vx - ux * vz, nz = ux * vy - uy * vx;
      const l = Math.hypot(nx, ny, nz) || 1;
      nx /= l;
      ny /= l;
      nz /= l;
      const vn = vertexNormal(sub, sub.index[t * 3], sub.index[t * 3 + 1], sub.index[t * 3 + 2]);
      if (vn && vn.x * nx + vn.y * ny + vn.z * nz < 0) {
        nx = -nx;
        ny = -ny;
        nz = -nz;
      }
      this.normals.set([nx, ny, nz], t * 3);
      const cx = (P[a] + P[b] + P[c]) / 3, cy = (P[a + 1] + P[b + 1] + P[c + 1]) / 3, cz = (P[a + 2] + P[b + 2] + P[c + 2]) / 3;
      this.centroids.set([cx, cy, cz], t * 3);
      this.radii[t] = Math.max(Math.hypot(P[a] - cx, P[a + 1] - cy, P[a + 2] - cz), Math.hypot(P[b] - cx, P[b + 1] - cy, P[b + 2] - cz), Math.hypot(P[c] - cx, P[c + 1] - cy, P[c + 2] - cz));
    }
  }
  get triangles() {
    return this.radii.length;
  }
}

export interface StampOpts {
  radius: number;
  hardness: number;
  part?: number;
}

export function stampSphere(geo: SurfaceGeo, p: Vec3, n: Vec3, w: number, h: number, o: StampOpts, cover: (i: number, a: number) => void): number[] {
  const s = geo.sub;
  const P = s.pos, U = s.uv;
  const ws = s.data.wrapS, wt = s.data.wrapT;
  const r = Math.max(1e-6, o.radius);
  const inner = r * Math.min(0.999, Math.max(0, o.hardness));
  const touched: number[] = [];
  for (let t = 0; t < geo.triangles; t++) {
    if (o.part !== undefined && o.part !== 0xffff && s.parts[t] !== o.part) continue;
    const cx = geo.centroids[t * 3], cy = geo.centroids[t * 3 + 1], cz = geo.centroids[t * 3 + 2];
    if (Math.hypot(cx - p.x, cy - p.y, cz - p.z) > r + geo.radii[t]) continue;
    const d = geo.normals[t * 3] * n.x + geo.normals[t * 3 + 1] * n.y + geo.normals[t * 3 + 2] * n.z;
    if (s.nrm ? d < -0.35 : Math.abs(d) < 0.2 && d < 0) continue;
    const ia = s.index[t * 3], ib = s.index[t * 3 + 1], ic = s.index[t * 3 + 2];
    const ax = U[ia * 2] * w, ay = U[ia * 2 + 1] * h, bx = U[ib * 2] * w, by = U[ib * 2 + 1] * h, qx = U[ic * 2] * w, qy = U[ic * 2 + 1] * h;
    const area = (bx - ax) * (qy - ay) - (by - ay) * (qx - ax);
    const x0 = Math.floor(Math.min(ax, bx, qx)), x1 = Math.ceil(Math.max(ax, bx, qx));
    const y0 = Math.floor(Math.min(ay, by, qy)), y1 = Math.ceil(Math.max(ay, by, qy));
    if (x1 - x0 > 4 * w || y1 - y0 > 4 * h) continue;
    let hit = false;
    const put = (x: number, y: number, b0: number, b1: number, b2: number) => {
      const px = P[ia * 3] * b0 + P[ib * 3] * b1 + P[ic * 3] * b2;
      const py = P[ia * 3 + 1] * b0 + P[ib * 3 + 1] * b1 + P[ic * 3 + 1] * b2;
      const pz = P[ia * 3 + 2] * b0 + P[ib * 3 + 2] * b1 + P[ic * 3 + 2] * b2;
      const dist = Math.hypot(px - p.x, py - p.y, pz - p.z);
      if (dist >= r) return;
      const a = dist <= inner ? 1 : 1 - (dist - inner) / Math.max(1e-6, r - inner);
      if (a <= 0) return;
      hit = true;
      cover(wrapTexel(y, h, wt) * w + wrapTexel(x, w, ws), a);
    };
    if (Math.abs(area) > 1e-9) {
      for (let y = y0; y < y1; y++)
        for (let x = x0; x < x1; x++) {
          const px = x + 0.5, py = y + 0.5;
          const b0 = ((bx - px) * (qy - py) - (by - py) * (qx - px)) / area;
          const b1 = ((qx - px) * (ay - py) - (qy - py) * (ax - px)) / area;
          const b2 = 1 - b0 - b1;
          if (b0 < -1e-4 || b1 < -1e-4 || b2 < -1e-4) continue;
          put(x, y, b0, b1, b2);
        }
    }
    if (!hit) put(Math.floor((ax + bx + qx) / 3), Math.floor((ay + by + qy) / 3), 1 / 3, 1 / 3, 1 / 3);
    if (hit) touched.push(t);
  }
  return touched;
}

export function partMaskOf(geo: SurfaceGeo, part: number, w: number, h: number): Uint8Array {
  const m = new Uint8Array(w * h);
  const s = geo.sub;
  const U = s.uv;
  for (let t = 0; t < geo.triangles; t++) {
    if (s.parts[t] !== part) continue;
    rasterUV(U, s.index[t * 3], s.index[t * 3 + 1], s.index[t * 3 + 2], w, h, s.data.wrapS, s.data.wrapT, (i) => (m[i] = 255));
  }
  return m;
}

export function rasterUV(U: Float32Array, ia: number, ib: number, ic: number, w: number, h: number, ws: number, wt: number, put: (i: number) => void) {
  const ax = U[ia * 2] * w, ay = U[ia * 2 + 1] * h, bx = U[ib * 2] * w, by = U[ib * 2 + 1] * h, qx = U[ic * 2] * w, qy = U[ic * 2 + 1] * h;
  const area = (bx - ax) * (qy - ay) - (by - ay) * (qx - ax);
  const x0 = Math.floor(Math.min(ax, bx, qx) - 0.5), x1 = Math.ceil(Math.max(ax, bx, qx) + 0.5);
  const y0 = Math.floor(Math.min(ay, by, qy) - 0.5), y1 = Math.ceil(Math.max(ay, by, qy) + 0.5);
  if (x1 - x0 > 4 * w || y1 - y0 > 4 * h) return;
  if (Math.abs(area) > 1e-9) {
    const sign = area > 0 ? 1 : -1;
    const edges: [number, number, number, number][] = [[ax, ay, bx, by], [bx, by, qx, qy], [qx, qy, ax, ay]];
    for (let y = y0; y < y1; y++)
      for (let x = x0; x < x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        let inside = true;
        for (const [ex, ey, fx, fy] of edges) {
          const len = Math.hypot(fx - ex, fy - ey) || 1e-6;
          if ((((fx - ex) * (py - ey) - (fy - ey) * (px - ex)) * sign) / len <= -0.49) {
            inside = false;
            break;
          }
        }
        if (inside) put(wrapTexel(y, h, wt) * w + wrapTexel(x, w, ws));
      }
  }
  put(wrapTexel(Math.floor((ay + by + qy) / 3), h, wt) * w + wrapTexel(Math.floor((ax + bx + qx) / 3), w, ws));
}

export class TexelOwners {
  head: Int32Array;
  next: number[] = [];
  tri: number[] = [];
  constructor(geo: SurfaceGeo, public w: number, public h: number) {
    this.head = new Int32Array(w * h).fill(-1);
    const s = geo.sub;
    for (let t = 0; t < geo.triangles; t++) {
      const seen = new Set<number>();
      rasterUV(s.uv, s.index[t * 3], s.index[t * 3 + 1], s.index[t * 3 + 2], w, h, s.data.wrapS, s.data.wrapT, (i) => {
        if (seen.has(i)) return;
        seen.add(i);
        this.tri.push(t);
        this.next.push(this.head[i]);
        this.head[i] = this.tri.length - 1;
      });
    }
  }
  forEach(i: number, fn: (t: number) => void) {
    for (let k = this.head[i]; k >= 0; k = this.next[k]) fn(this.tri[k]);
  }
}
