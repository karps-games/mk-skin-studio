import * as THREE from "three";
import { TransformControls } from "three/examples/jsm/controls/TransformControls.js";
import type { ModelData, Sub, SubData } from "./preview3d";

export const SHAPES = ["sphere", "box", "cylinder", "cone", "ring"] as const;
export type Shape = (typeof SHAPES)[number];
export type Vec3 = [number, number, number];
export type GizmoMode = "translate" | "rotate" | "scale";
export const LIMIT = 16;
export const TEX = 64;
export const accDoc = (id: string) => `acc:${id}`;
export const isAccDoc = (name: string) => name.startsWith("acc:");
export const accIdOf = (name: string) => name.slice(4);
export const newAccId = () => Math.random().toString(36).slice(2, 10).padEnd(8, "0");
export const BONES = ["face_1", "spin", "skl_root", "wrist_l1", "wrist_r1", "arm_l1", "arm_l2", "arm_r1", "arm_r2", "leg_l1", "leg_l2", "leg_r1", "leg_r2", "ankle_l1", "ankle_r1"] as const;

export interface Accessory {
  id: string;
  name: string;
  shape: Shape;
  bone: string;
  position: Vec3;
  rotation: Vec3;
  scale: Vec3;
  color: Vec3;
  hidden?: boolean;
}

export interface ShapeView {
  name: string;
  positions: number[];
  normals: number[];
  uvs: number[];
  indices: number[];
}

export const baseBone = (name: string) => name.replace(/^pcd_/, "");

const num = (v: unknown, lo: number, hi: number) => (typeof v === "number" && Number.isFinite(v) ? Math.min(hi, Math.max(lo, v)) : null);
const away = (v: number) => (Math.abs(v) < 0.01 ? (v < 0 ? -0.01 : 0.01) : v);
const vec = (v: unknown, lo: number, hi: number): Vec3 | null => {
  if (!Array.isArray(v) || v.length !== 3) return null;
  const out = v.map((x) => num(x, lo, hi));
  return out.every((x) => x !== null) ? (out as Vec3) : null;
};

export function clean(list: unknown): Accessory[] {
  if (!Array.isArray(list)) return [];
  const out: Accessory[] = [];
  const ids = new Set<string>();
  for (const a of list.slice(0, LIMIT)) {
    if (!a || typeof a !== "object") continue;
    const x = a as Record<string, unknown>;
    const shape = SHAPES.find((s) => s === x.shape);
    const position = vec(x.position, -1e4, 1e4), rotation = vec(x.rotation, -360, 360), scale = vec(x.scale, -1e4, 1e4), color = vec(x.color, 0, 255);
    if (!shape || typeof x.bone !== "string" || !position || !rotation || !scale || !color) continue;
    let id = typeof x.id === "string" && /^[a-z0-9]{1,16}$/.test(x.id) ? x.id : newAccId();
    while (ids.has(id)) id = newAccId();
    ids.add(id);
    out.push({ id, name: typeof x.name === "string" ? x.name.slice(0, 40) : "", shape, bone: x.bone.slice(0, 40), position, rotation, scale: scale.map(away) as Vec3, color: color.map(Math.round) as Vec3, ...(x.hidden === true ? { hidden: true } : {}) });
  }
  return out;
}

const m4 = (a: ArrayLike<number>, o = 0) => new THREE.Matrix4().set(a[o], a[o + 1], a[o + 2], a[o + 3], a[o + 4], a[o + 5], a[o + 6], a[o + 7], a[o + 8], a[o + 9], a[o + 10], a[o + 11], 0, 0, 0, 1);
const round = (v: number, k = 1000) => Math.round(v * k) / k;

export function placeAt(bind: number[], world: Vec3, size: number): Pick<Accessory, "position" | "rotation" | "scale"> {
  const b = m4(bind);
  const t = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
  b.decompose(t, q, s);
  const p = new THREE.Vector3(...world).applyMatrix4(b.clone().invert());
  const e = new THREE.Euler().setFromQuaternion(q.invert(), "ZYX");
  return {
    position: p.toArray().map((v) => round(v)) as Vec3,
    rotation: [e.x, e.y, e.z].map((v) => round(THREE.MathUtils.radToDeg(v), 100)) as Vec3,
    scale: s.toArray().map((v) => round(away(size / v))) as Vec3,
  };
}

export function rebind(a: Accessory, from: number[] | null, to: number[], at: Vec3): Pick<Accessory, "position" | "rotation" | "scale"> {
  const local = new THREE.Matrix4().compose(new THREE.Vector3(...a.position), new THREE.Quaternion().setFromEuler(new THREE.Euler(...(a.rotation.map((d) => THREE.MathUtils.degToRad(d)) as Vec3), "ZYX")), new THREE.Vector3(...a.scale));
  const world = from ? m4(from).multiply(local) : local;
  world.setPosition(...at);
  const t = new THREE.Vector3(), q = new THREE.Quaternion(), s = new THREE.Vector3();
  m4(to).invert().multiply(world).decompose(t, q, s);
  const e = new THREE.Euler().setFromQuaternion(q, "ZYX");
  return {
    position: t.toArray().map((v) => round(v)) as Vec3,
    rotation: [e.x, e.y, e.z].map((v) => round(THREE.MathUtils.radToDeg(v), 100)) as Vec3,
    scale: s.toArray().map((v) => round(away(v))) as Vec3,
  };
}

export function boneOrigin(bind: number[]): Vec3 {
  return [bind[3], bind[7], bind[11]];
}

interface Item {
  host: THREE.Group;
  mesh: THREE.Mesh;
  bone: number;
}

export class AccessoryRig {
  readonly root = new THREE.Group();
  private geos = new Map<string, THREE.BufferGeometry>();
  private items: Item[] = [];
  private list: Accessory[] = [];
  private model: ModelData | null = null;
  private mats: Float32Array | null = null;
  private gizmo: TransformControls | null = null;
  private before: Accessory | null = null;
  selected = -1;
  editable = false;
  onEdit: (index: number, before: Accessory, after: Accessory) => void = () => {};
  onLive: (index: number, a: Accessory) => void = () => {};
  source: (id: string) => { w: number; h: number; data: Uint8ClampedArray } | null = () => null;
  nearest = true;
  private textures = new Map<string, THREE.DataTexture>();
  private paint: Sub[] | null = null;

  constructor(private camera: THREE.Camera, private canvas: HTMLCanvasElement, scene: THREE.Scene, private request: () => void) {
    scene.add(this.root);
    try {
      const g = new TransformControls(camera, canvas);
      g.setSize(0.8);
      g.addEventListener("change", () => this.request());
      g.addEventListener("dragging-changed", (e) => this.dragging(!!(e as unknown as { value: boolean }).value));
      g.addEventListener("objectChange", () => {
        this.paint = null;
        const i = this.selected;
        if (i >= 0 && this.items[i]) this.onLive(i, this.read(i));
      });
      scene.add(g.getHelper());
      this.gizmo = g;
    } catch {
      this.gizmo = null;
    }
  }

  setShapes(list: ShapeView[]) {
    for (const s of list) {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(s.positions, 3));
      g.setAttribute("normal", new THREE.Float32BufferAttribute(s.normals, 3));
      g.setAttribute("uv", new THREE.Float32BufferAttribute(s.uvs, 2));
      g.setIndex(s.indices);
      this.geos.get(s.name)?.dispose();
      this.geos.set(s.name, g);
    }
    this.rebuild();
  }
  get ready() {
    return this.geos.size > 0;
  }

  boneIndex(name: string, model = this.model): number {
    const names = model?.boneNames ?? [];
    const base = baseBone(name);
    const i = names.indexOf(base);
    return i >= 0 ? i : names.indexOf(`pcd_${base}`);
  }

  setModel(model: ModelData | null) {
    this.model = model;
    this.mats = null;
    this.rebuild();
  }

  set(list: Accessory[], selected: number) {
    this.list = list;
    this.selected = selected;
    this.rebuild();
  }

  private textureFor(id: string): THREE.DataTexture | null {
    const src = this.source(id);
    if (!src) return null;
    let t = this.textures.get(id);
    if (!t || (t.image as { width: number }).width !== src.w || (t.image as { height: number }).height !== src.h) {
      t?.dispose();
      t = new THREE.DataTexture(new Uint8Array(src.data), src.w, src.h, THREE.RGBAFormat);
      t.colorSpace = THREE.SRGBColorSpace;
      t.flipY = false;
      t.wrapS = t.wrapT = THREE.ClampToEdgeWrapping;
      this.filter(t);
      this.textures.set(id, t);
    } else (t.image as { data: Uint8Array }).data.set(src.data);
    t.needsUpdate = true;
    return t;
  }
  private filter(t: THREE.DataTexture) {
    t.magFilter = this.nearest ? THREE.NearestFilter : THREE.LinearFilter;
    t.minFilter = this.nearest ? THREE.NearestFilter : THREE.LinearMipmapLinearFilter;
    t.generateMipmaps = !this.nearest;
  }
  setNearest(v: boolean) {
    this.nearest = v;
    for (const t of this.textures.values()) {
      this.filter(t);
      t.needsUpdate = true;
    }
    this.request();
  }
  updateTexture(id: string) {
    const i = this.list.findIndex((a) => a.id === id);
    const t = this.textureFor(id);
    const it = this.items[i];
    if (it && t) {
      const m = it.mesh.material as THREE.MeshLambertMaterial;
      if (m.map !== t) (m.map = t), m.color.set(0xffffff), (m.needsUpdate = true);
    }
    this.request();
  }

  surfaces(): Sub[] {
    if (this.paint) return this.paint;
    this.root.updateMatrixWorld(true);
    this.paint = this.items.flatMap((it, i) => {
      const a = this.list[i];
      const g = it.mesh.geometry;
      const p = g.getAttribute("position"), n = g.getAttribute("normal"), uv = g.getAttribute("uv"), ix = g.getIndex();
      if (!it.host.visible || !p || !n || !uv || !ix) return [];
      const m = it.mesh.matrixWorld, nm = new THREE.Matrix3().getNormalMatrix(m);
      const pos = new Float32Array(p.count * 3), nrm = new Float32Array(p.count * 3);
      const v = new THREE.Vector3();
      for (let k = 0; k < p.count; k++) {
        v.fromBufferAttribute(p, k).applyMatrix4(m).toArray(pos, k * 3);
        v.fromBufferAttribute(n, k).applyMatrix3(nm).normalize().toArray(nrm, k * 3);
      }
      const index = new Uint32Array(ix.array);
      const tris = index.length / 3;
      const data: SubData = { material: "", shape: a.shape, texture: accDoc(a.id), wrapS: 0, wrapT: 0, xlu: false, vertices: p.count, triangles: tris, positions: "", normals: "", uvs: "", indices: "", parts: "" };
      return [{ mesh: it.mesh, data, parts: new Uint16Array(tris).fill(0xffff), index, texName: accDoc(a.id), pos, uv: new Float32Array(uv.array), nrm }];
    });
    return this.paint;
  }

  private rebuild() {
    this.paint = null;
    this.gizmo?.detach();
    for (const it of this.items) {
      (it.mesh.material as THREE.Material).dispose();
      this.root.remove(it.host);
    }
    this.items = [];
    for (const a of this.list) {
      const host = new THREE.Group();
      host.matrixAutoUpdate = false;
      const map = this.textureFor(a.id);
      const mesh = new THREE.Mesh(this.geos.get(a.shape) ?? new THREE.BufferGeometry(), new THREE.MeshLambertMaterial({ side: THREE.DoubleSide, map }));
      mesh.rotation.order = "ZYX";
      host.add(mesh);
      this.root.add(host);
      const bone = this.boneIndex(a.bone);
      this.items.push({ host, mesh, bone });
      this.apply(this.items.length - 1, a);
    }
    this.place();
    this.syncGizmo();
  }

  apply(i: number, a: Accessory) {
    const it = this.items[i];
    if (!it) return;
    it.mesh.position.set(...a.position);
    it.mesh.rotation.set(...(a.rotation.map((d) => THREE.MathUtils.degToRad(d)) as Vec3));
    it.mesh.scale.set(...a.scale);
    const mat = it.mesh.material as THREE.MeshLambertMaterial;
    if (mat.map) mat.color.set(0xffffff);
    else mat.color.setRGB(a.color[0] / 255, a.color[1] / 255, a.color[2] / 255, THREE.SRGBColorSpace);
    this.paint = null;
    this.request();
  }

  read(i: number): Accessory {
    const it = this.items[i], a = this.list[i];
    const r = it.mesh.rotation;
    return {
      ...a,
      position: it.mesh.position.toArray().map((v) => round(v)) as Vec3,
      rotation: [r.x, r.y, r.z].map((v) => round(THREE.MathUtils.radToDeg(v), 100)) as Vec3,
      scale: it.mesh.scale.toArray().map((v) => round(away(v))) as Vec3,
    };
  }

  pose(mats: Float32Array | null) {
    this.mats = mats;
    this.place();
    this.syncGizmo();
  }

  hostMatrix(bone: number): THREE.Matrix4 | null {
    const bind = this.model?.bind?.[bone];
    if (!bind) return null;
    const m = m4(bind);
    return this.mats ? m4(this.mats, bone * 12).multiply(m) : m;
  }

  private place() {
    this.paint = null;
    this.list.forEach((a, i) => {
      const it = this.items[i];
      const m = it.bone >= 0 ? this.hostMatrix(it.bone) : null;
      it.host.visible = !!m && !a.hidden;
      if (m) {
        it.host.matrix.copy(m);
        it.host.matrixWorldNeedsUpdate = true;
      }
    });
    this.request();
  }

  private syncGizmo() {
    const g = this.gizmo;
    if (!g) return;
    const it = this.items[this.selected];
    if (this.editable && !this.mats && it && it.host.visible) {
      if (g.object !== it.mesh) g.attach(it.mesh);
    } else if (g.object) g.detach();
    this.request();
  }

  select(i: number) {
    this.selected = i;
    this.syncGizmo();
  }

  setEditable(on: boolean) {
    this.editable = on;
    this.syncGizmo();
  }

  setMode(mode: GizmoMode) {
    this.gizmo?.setMode(mode);
    this.gizmo?.setSpace(mode === "translate" ? "world" : "local");
    this.request();
  }
  get mode(): GizmoMode {
    return (this.gizmo?.getMode() as GizmoMode) ?? "translate";
  }

  get busy() {
    const g = this.gizmo;
    return !!g && !!g.object && (g.dragging || g.axis !== null);
  }
  grab(clientX: number, clientY: number) {
    const g = this.gizmo;
    if (!g || !g.object || !g.enabled) return false;
    if (g.dragging) return true;
    const r = this.canvas.getBoundingClientRect();
    g.pointerHover({ x: ((clientX - r.left) / r.width) * 2 - 1, y: -((clientY - r.top) / r.height) * 2 + 1, button: 0 } as unknown as PointerEvent);
    return g.axis !== null;
  }
  get attached() {
    return !!this.gizmo?.object;
  }

  private dragging(on: boolean) {
    const i = this.selected;
    if (i < 0 || !this.items[i]) return;
    if (on) this.before = structuredClone(this.list[i]);
    else if (this.before) {
      const after = this.read(i);
      const before = this.before;
      this.before = null;
      this.onEdit(i, before, after);
    }
  }

  pick(clientX: number, clientY: number): { index: number; distance: number } | null {
    const r = this.canvas.getBoundingClientRect();
    const ray = new THREE.Raycaster();
    ray.setFromCamera(new THREE.Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1), this.camera);
    this.root.updateMatrixWorld(true);
    const meshes = this.items.filter((it) => it.host.visible).map((it) => it.mesh);
    const hit = ray.intersectObjects(meshes, false)[0];
    if (!hit) return null;
    return { index: this.items.findIndex((it) => it.mesh === hit.object), distance: hit.distance };
  }

  screenOf(i: number): [number, number] | null {
    const it = this.items[i];
    if (!it || !it.host.visible) return null;
    this.root.updateMatrixWorld(true);
    const v = it.mesh.getWorldPosition(new THREE.Vector3()).project(this.camera);
    const r = this.canvas.getBoundingClientRect();
    return [r.left + ((v.x + 1) / 2) * r.width, r.top + ((1 - v.y) / 2) * r.height];
  }

  worldOf(i: number): Vec3 | null {
    const it = this.items[i];
    if (!it || !it.host.visible) return null;
    this.root.updateMatrixWorld(true);
    return it.mesh.getWorldPosition(new THREE.Vector3()).toArray() as Vec3;
  }

  shown(i: number) {
    return !!this.items[i]?.host.visible;
  }

  clear() {
    this.set([], -1);
  }
}
