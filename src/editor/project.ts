import type { Adjust, Blend, Doc, Layer } from "./doc";
import { touch } from "./doc";
import { b64ToBytes, bytesToB64, decodePng, encodePng } from "./png";

export const FORMAT = "mkss-project/1.1";
export const FORMATS = ["mkss-project/1", FORMAT];

export interface ProjectFile { name: string; data: string }
export interface Project { manifest: string; files: ProjectFile[] }

interface LayerMeta {
  id: number;
  name: string;
  kind: "pixel" | "adjust";
  base: boolean;
  visible: boolean;
  locked: boolean;
  opacity: number;
  blend: Blend;
  adjust?: Adjust;
  data?: string;
  keep?: string;
  mask?: string;
  source?: { name: string; sha256: string };
}
interface TextureMeta { name: string; sha256: string; width: number; height: number; active: number; layers: LayerMeta[] }
export interface Manifest { format: string; tool: string; toolVersion: string; character: { code: string; name: string }; savedAt: string; textures: TextureMeta[] }

const safe = (s: string) => s.replace(/[^A-Za-z0-9_.-]/g, "_");

export function hasWork(d: Doc) {
  for (const L of d.layers) {
    if (L.base) {
      if (!L.visible || L.opacity !== 1 || L.blend !== "normal") return true;
      for (let i = 0; i < L.pixels!.length; i++) if (L.pixels![i] !== d.original[i]) return true;
      continue;
    }
    if (L.kind === "adjust") return true;
    if (L.pixels!.some((v, i) => i % 4 === 3 && v !== 0)) return true;
  }
  return d.layers.length > 2;
}

export async function serialize(docs: Iterable<Doc>, character: { code: string; name: string }, version: string): Promise<Project> {
  const files: ProjectFile[] = [];
  const textures: TextureMeta[] = [];
  for (const d of docs) {
    if (!hasWork(d)) continue;
    const dir = `layers/${safe(d.name)}`;
    const layers: LayerMeta[] = [];
    for (const L of d.layers) {
      const m: LayerMeta = { id: L.id, name: L.name, kind: L.kind, base: L.base, visible: L.visible, locked: L.locked, opacity: L.opacity, blend: L.blend };
      if (L.adjust) m.adjust = { ...L.adjust };
      if (L.source) m.source = { ...L.source };
      if (L.pixels) {
        const px = L.pixels.slice();
        const keep = new Uint8ClampedArray(d.w * d.h * 4);
        let kept = 0, any = false;
        for (let i = 0; i < d.w * d.h; i++) {
          const o = i * 4;
          if (px[o] === d.original[o] && px[o + 1] === d.original[o + 1] && px[o + 2] === d.original[o + 2] && px[o + 3] === d.original[o + 3] && (L.base || px[o + 3] !== 0)) {
            px[o] = px[o + 1] = px[o + 2] = px[o + 3] = 0;
            keep[o + 3] = 255;
            kept++;
          } else if (px[o + 3] || px[o] || px[o + 1] || px[o + 2]) any = true;
        }
        if (any) {
          m.data = `${dir}/${L.id}.png`;
          files.push({ name: m.data, data: bytesToB64(await encodePng(px, d.w, d.h)) });
        }
        if (kept) {
          m.keep = `${dir}/${L.id}-keep.png`;
          files.push({ name: m.keep, data: bytesToB64(await encodePng(keep, d.w, d.h)) });
        }
      }
      if (L.mask) {
        const mk = new Uint8ClampedArray(d.w * d.h * 4);
        for (let i = 0; i < L.mask.length; i++) mk[i * 4 + 3] = L.mask[i];
        m.mask = `${dir}/${L.id}-mask.png`;
        files.push({ name: m.mask, data: bytesToB64(await encodePng(mk, d.w, d.h)) });
      }
      layers.push(m);
    }
    textures.push({ name: d.name, sha256: d.sha256, width: d.w, height: d.h, active: d.active, layers });
  }
  const manifest: Manifest = { format: FORMAT, tool: "MK Skin Studio", toolVersion: version, character, savedAt: new Date().toISOString(), textures };
  return { manifest: JSON.stringify(manifest), files };
}

export function parseManifest(p: Project): Manifest {
  const m = JSON.parse(p.manifest) as Manifest;
  if (!FORMATS.includes(m.format) || !m.character?.code || !Array.isArray(m.textures)) throw new Error("E:project.format");
  return m;
}

export async function apply(p: Project, docs: Map<string, Doc>): Promise<{ mismatched: string[]; missing: string[] }> {
  const m = parseManifest(p);
  const files = new Map(p.files.map((f) => [f.name, f.data]));
  const png = async (name: string | undefined, d: Doc) => {
    if (!name) return null;
    const b64 = files.get(name);
    if (!b64) throw new Error("E:project.format");
    const img = await decodePng(b64ToBytes(b64));
    if (img.width !== d.w || img.height !== d.h) throw new Error("E:project.format");
    return img.data;
  };
  const mismatched: string[] = [], missing: string[] = [];
  for (const tm of m.textures) {
    const d = docs.get(tm.name);
    if (!d) {
      missing.push(tm.name);
      continue;
    }
    if (tm.sha256 !== d.sha256) mismatched.push(tm.name);
    if (tm.width !== d.w || tm.height !== d.h) {
      missing.push(tm.name);
      continue;
    }
    const layers: Layer[] = [];
    let next = 1;
    for (const lm of tm.layers) {
      let pixels: Uint8ClampedArray | null = null;
      if (lm.kind === "pixel") {
        pixels = (await png(lm.data, d)) ?? new Uint8ClampedArray(d.w * d.h * 4);
        const keep = await png(lm.keep, d);
        if (keep) for (let i = 0; i < d.w * d.h; i++) if (keep[i * 4 + 3]) pixels.set(d.original.subarray(i * 4, i * 4 + 4), i * 4);
        if (lm.base && !lm.data && !lm.keep) pixels = d.original.slice();
      }
      let mask: Uint8Array | null = null;
      const mk = await png(lm.mask, d);
      if (mk) {
        mask = new Uint8Array(d.w * d.h);
        for (let i = 0; i < mask.length; i++) mask[i] = mk[i * 4 + 3];
      }
      layers.push({ id: lm.id, name: lm.name, kind: lm.kind, base: !!lm.base, visible: lm.visible, locked: lm.locked, opacity: lm.opacity, blend: lm.blend, pixels, adjust: lm.adjust ? { ...lm.adjust } : null, mask, ...(lm.source ? { source: lm.source } : {}) });
      next = Math.max(next, lm.id + 1);
    }
    if (!layers.some((l) => l.base)) continue;
    d.layers = layers;
    d.active = layers.some((l) => l.id === tm.active) ? tm.active : layers[layers.length - 1].id;
    d.nextId = next;
    d.selection = null;
    touch(d);
  }
  return { mismatched, missing };
}
