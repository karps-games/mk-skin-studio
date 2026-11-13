import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { b64ToBytes } from "./png";

export interface SubData {
  material: string;
  shape: string;
  texture: string | null;
  wrapS: number;
  wrapT: number;
  xlu: boolean;
  vertices: number;
  triangles: number;
  positions: string;
  normals: string;
  uvs: string;
  indices: string;
  parts: string;
}
export interface ModelData {
  kind: string;
  file: string;
  name: string;
  vehicle?: string | null;
  detail?: string;
  bboxMin: [number, number, number];
  bboxMax: [number, number, number];
  submeshes: SubData[];
}
export interface PartData {
  key: string;
  n: number;
  material: string;
  pixels: number;
  bbox: [number, number, number, number];
  runs: string;
  outline: string;
}
export interface TexParts {
  texture: string;
  width: number;
  height: number;
  model: number | null;
  aliasOf: string | null;
  parts: PartData[];
}
export interface FactData {
  texture: string;
  width: number;
  height: number;
  role: string;
  usedTexels: number;
  mirroredTexels: number;
  smearTriangles: number;
  lods: string[];
  derivedFrom: string | null;
  used: string;
}
export interface LinkData {
  low: string;
  width: number;
  height: number;
  highs: { texture: string; width: number; height: number }[];
  samples: string;
  mapped: number;
  distRel: number;
}
export interface EyeData {
  frames: string[];
  defaultFrame: string;
  blink: string[];
}
export interface ModelBundle {
  character: string;
  models: ModelData[];
  textures: TexParts[];
  unreferenced: string[];
  facts?: FactData[];
  links?: LinkData[];
  shares?: LinkData[];
  eyes?: EyeData[];
}

const f32 = (b64: string) => {
  const b = b64ToBytes(b64);
  return new Float32Array(b.buffer, b.byteOffset, b.byteLength / 4);
};
const u32 = (b64: string) => {
  const b = b64ToBytes(b64);
  return new Uint32Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
};
export const u16 = (b64: string) => {
  const b = b64ToBytes(b64);
  return new Uint16Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
};
export const u32arr = u32;
export function vertexNormal(s: { nrm: Float32Array | null }, a: number, b: number, c: number): THREE.Vector3 | null {
  const N = s.nrm;
  if (!N || N.length < (Math.max(a, b, c) + 1) * 3) return null;
  const v = new THREE.Vector3(N[a * 3] + N[b * 3] + N[c * 3], N[a * 3 + 1] + N[b * 3 + 1] + N[c * 3 + 1], N[a * 3 + 2] + N[b * 3 + 2] + N[c * 3 + 2]);
  return v.lengthSq() > 1e-8 ? v : null;
}
export const i32arr = (b64: string) => {
  const b = b64ToBytes(b64);
  return new Int32Array(b.buffer.slice(b.byteOffset, b.byteOffset + b.byteLength));
};

const WRAP = [THREE.ClampToEdgeWrapping, THREE.RepeatWrapping, THREE.MirroredRepeatWrapping] as const;

export interface Sub {
  mesh: THREE.Mesh;
  data: SubData;
  parts: Uint16Array;
  index: Uint32Array;
  texName: string | null;
  pos: Float32Array;
  uv: Float32Array;
  nrm: Float32Array | null;
}

export interface SurfaceHit {
  sub: number;
  face: number;
  texture: string;
  part: number;
  uv: [number, number];
  point: THREE.Vector3;
  normal: THREE.Vector3;
  distance: number;
}

export interface PickResult {
  texture: string;
  part: number;
}

export class Preview3D {
  readonly renderer: THREE.WebGLRenderer | null = null;
  readonly ok: boolean;
  private scene = new THREE.Scene();
  private camera = new THREE.PerspectiveCamera(35, 1, 1, 10000);
  private controls: OrbitControls | null = null;
  private group = new THREE.Group();
  private subs: Sub[] = [];
  private textures = new Map<string, THREE.DataTexture>();
  private highlight: THREE.Mesh | null = null;
  private selectedHL: THREE.Mesh | null = null;
  private model: ModelData | null = null;
  private needs = true;
  private raf = 0;
  private center = new THREE.Vector3();
  private radius = 100;
  nearest = true;
  paintMode = false;
  hand = false;
  tempHand = false;
  rightPicks = false;
  private nav: { id: number; mode: "orbit" | "pan"; x: number; y: number; moved: number } | null = null;
  private altDown = false;
  onNavStart: () => void = () => {};
  navigations = 0;
  private ring: THREE.Mesh | null = null;
  private mirrorHL: THREE.LineSegments | null = null;
  private mirrorTris = 0;
  textureFor: (name: string) => string = (n) => n;
  source: (name: string) => { w: number; h: number; data: Uint8ClampedArray } | null = () => null;
  onPick: (r: PickResult | null) => void = () => {};
  onHover: (r: PickResult | null) => void = () => {};
  uploads = 0;

  constructor(private canvas: HTMLCanvasElement) {
    let r: THREE.WebGLRenderer | null = null;
    try {
      r = new THREE.WebGLRenderer({ canvas, antialias: true, alpha: false, preserveDrawingBuffer: false });
    } catch {
      r = null;
    }
    this.renderer = r;
    this.ok = !!r;
    if (!r) return;
    r.outputColorSpace = THREE.SRGBColorSpace;
    r.setPixelRatio(Math.min(2, window.devicePixelRatio || 1));
    this.scene.add(new THREE.HemisphereLight(0xffffff, 0x8a8a96, 2.0));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(0.6, 1, 1.2);
    this.camera.add(sun);
    this.scene.add(this.camera);
    this.scene.add(this.group);
    this.controls = new OrbitControls(this.camera, canvas);
    this.controls.enableDamping = false;
    this.controls.enableRotate = false;
    this.controls.enablePan = false;
    Object.assign(this.controls.mouseButtons, { LEFT: -1, MIDDLE: -1, RIGHT: -1 });
    this.controls.addEventListener("change", () => this.request());
    let down: [number, number] | null = null;
    canvas.addEventListener("pointerdown", (e) => (down = [e.clientX, e.clientY]));
    canvas.addEventListener("pointerup", (e) => {
      if (!this.paintMode && down && Math.hypot(e.clientX - down[0], e.clientY - down[1]) < 4) this.onPick(this.pickAt(e.clientX, e.clientY));
      down = null;
    });
    canvas.addEventListener("pointermove", (e) => {
      if (e.buttons || this.paintMode) return;
      const p = this.pickAt(e.clientX, e.clientY);
      this.setHighlight(p, false);
      this.onHover(p);
    });
    canvas.addEventListener("pointerleave", () => {
      this.setHighlight(null, false);
      this.onHover(null);
      this.setRing(null);
    });
    const host = canvas.parentElement!;
    host.addEventListener("pointerdown", (e) => this.navDown(e), true);
    host.addEventListener("pointermove", (e) => this.navMove(e), true);
    for (const t of ["pointerup", "pointercancel", "lostpointercapture"]) host.addEventListener(t, (e) => this.navUp(e as PointerEvent), true);
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    canvas.addEventListener("auxclick", (e) => e.preventDefault());
    window.addEventListener("keydown", (e) => e.key === "Alt" && (this.altDown = true));
    window.addEventListener("keyup", (e) => e.key === "Alt" && (this.altDown = false));
    window.addEventListener("blur", () => (this.altDown = false));
    this.resizeObs = new ResizeObserver(() => this.resize());
    this.resizeObs.observe(canvas.parentElement ?? canvas);
    const loop = () => {
      this.raf = requestAnimationFrame(loop);
      if (this.controls?.autoRotate) {
        this.controls.update();
        this.needs = true;
      }
      if (this.needs) this.render();
    };
    loop();
  }

  private resizeObs: ResizeObserver | null = null;
  navFor(e: { button: number; shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey?: boolean }): "orbit" | "pan" | null {
    const mod = e.shiftKey || e.ctrlKey || !!e.metaKey;
    if (this.hand || this.tempHand) return e.button === 0 ? (mod ? "pan" : "orbit") : e.button === 1 || e.button === 2 ? "pan" : null;
    if (e.button === 2 && this.rightPicks && this.paintMode) return null;
    if (e.button === 1 || e.button === 2) return mod ? "pan" : !this.paintMode && e.button === 2 ? "pan" : "orbit";
    if (e.button === 0 && (e.altKey || this.altDown || !this.paintMode)) return mod ? "pan" : "orbit";
    return null;
  }
  private navDown(e: PointerEvent) {
    if (!this.controls || !this.subs.length) return;
    const mode = this.navFor(e);
    if (!mode) return;
    if (this.nav && this.nav.id === e.pointerId) {
      if (this.paintMode) e.stopPropagation();
      return;
    }
    this.nav = { id: e.pointerId, mode, x: e.clientX, y: e.clientY, moved: 0 };
    this.navigations++;
    try {
      this.canvas.setPointerCapture(e.pointerId);
    } catch {}
    if (e.button !== 0) e.preventDefault();
    if (this.paintMode) {
      e.stopPropagation();
      this.onNavStart();
      this.setRing(null);
    }
    this.syncCursor();
  }
  private navMove(e: PointerEvent) {
    const n = this.nav;
    if (!n || e.pointerId !== n.id) return;
    const dx = e.clientX - n.x, dy = e.clientY - n.y;
    n.x = e.clientX;
    n.y = e.clientY;
    n.moved += Math.abs(dx) + Math.abs(dy);
    const mode = e.shiftKey || e.ctrlKey || e.metaKey ? "pan" : n.mode;
    if (mode === "pan") this.pan(dx, dy);
    else this.orbit(dx, dy);
    if (this.paintMode) e.stopPropagation();
  }
  private navUp(e: PointerEvent) {
    const n = this.nav;
    if (!n || e.pointerId !== n.id) return;
    this.nav = null;
    try {
      if (this.canvas.hasPointerCapture(e.pointerId)) this.canvas.releasePointerCapture(e.pointerId);
    } catch {}
    this.syncCursor();
  }
  get navigating() {
    return !!this.nav;
  }
  orbit(dx: number, dy: number) {
    if (!this.controls) return;
    const target = this.controls.target;
    const off = this.camera.position.clone().sub(target);
    const sph = new THREE.Spherical().setFromVector3(off);
    const h = this.viewHeight;
    sph.theta -= (2 * Math.PI * dx) / h;
    sph.phi = Math.min(Math.PI - 0.05, Math.max(0.05, sph.phi - (2 * Math.PI * dy) / h));
    off.setFromSpherical(sph);
    this.camera.position.copy(target).add(off);
    this.camera.lookAt(target);
    this.controls.update();
    this.request();
  }
  pan(dx: number, dy: number) {
    if (!this.controls) return;
    const target = this.controls.target;
    this.camera.updateMatrixWorld();
    const k = this.worldPerPixel(this.camera.position.distanceTo(target));
    const right = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 0).multiplyScalar(-dx * k);
    const up = new THREE.Vector3().setFromMatrixColumn(this.camera.matrixWorld, 1).multiplyScalar(dy * k);
    const move = right.add(up);
    this.camera.position.add(move);
    target.add(move);
    this.controls.update();
    this.request();
  }
  get target(): THREE.Vector3 {
    return this.controls ? this.controls.target.clone() : this.center.clone();
  }
  syncCursor() {
    const hand = this.hand || this.tempHand;
    this.canvas.style.cursor = this.nav ? (this.nav.mode === "pan" && !hand ? "move" : "grabbing") : hand ? "grab" : this.paintMode ? "crosshair" : "";
  }
  setHand(on: boolean, temporary = false) {
    if (temporary) this.tempHand = on;
    else this.hand = on;
    if (on) this.setRing(null);
    this.syncCursor();
  }
  setPaintMode(on: boolean) {
    this.paintMode = on;
    if (!on) {
      this.setRing(null);
      this.setMirror(null);
    }
    this.setHighlight(null, false);
    this.syncCursor();
  }
  reparented() {
    this.resizeObs?.disconnect();
    this.resizeObs?.observe(this.canvas.parentElement ?? this.canvas);
    this.resize();
  }
  get subMeshes(): readonly Sub[] {
    return this.subs;
  }
  get cam() {
    return this.camera;
  }
  get viewHeight() {
    return Math.max(1, this.canvas.getBoundingClientRect().height);
  }
  worldPerPixel(distance: number) {
    return (2 * distance * Math.tan(THREE.MathUtils.degToRad(this.camera.fov / 2))) / this.viewHeight;
  }

  hitAt(clientX: number, clientY: number): SurfaceHit | null {
    if (!this.renderer || !this.subs.length) return null;
    const r = this.canvas.getBoundingClientRect();
    const ndc = new THREE.Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const hits = ray.intersectObjects(this.subs.map((s) => s.mesh), false);
    for (const h of hits) {
      const si = this.subs.findIndex((x) => x.mesh === h.object);
      const s = this.subs[si];
      if (!s || !s.texName || h.faceIndex == null) continue;
      const f = h.faceIndex;
      const a = s.index[f * 3], b = s.index[f * 3 + 1], c = s.index[f * 3 + 2];
      const pa = new THREE.Vector3(s.pos[a * 3], s.pos[a * 3 + 1], s.pos[a * 3 + 2]);
      const pb = new THREE.Vector3(s.pos[b * 3], s.pos[b * 3 + 1], s.pos[b * 3 + 2]);
      const pc = new THREE.Vector3(s.pos[c * 3], s.pos[c * 3 + 1], s.pos[c * 3 + 2]);
      const bary = new THREE.Vector3();
      THREE.Triangle.getBarycoord(h.point, pa, pb, pc, bary);
      const u = s.uv[a * 2] * bary.x + s.uv[b * 2] * bary.y + s.uv[c * 2] * bary.z;
      const v = s.uv[a * 2 + 1] * bary.x + s.uv[b * 2 + 1] * bary.y + s.uv[c * 2 + 1] * bary.z;
      const n = new THREE.Triangle(pa, pb, pc).getNormal(new THREE.Vector3());
      const vn = vertexNormal(s, a, b, c);
      if (vn ? n.dot(vn) < 0 : n.dot(ray.ray.direction) > 0) n.negate();
      return { sub: si, face: f, texture: s.texName, part: s.parts[f], uv: [u, v], point: h.point.clone(), normal: n, distance: h.distance };
    }
    return null;
  }

  setRing(hit: { point: THREE.Vector3; normal: THREE.Vector3 } | null, radius = 1) {
    if (!hit) {
      if (this.ring) this.ring.visible = false;
      this.request();
      return;
    }
    if (!this.ring) {
      const g = new THREE.RingGeometry(0.86, 1, 48);
      const m = new THREE.MeshBasicMaterial({ color: 0x00d4ff, transparent: true, opacity: 0.95, depthTest: false, side: THREE.DoubleSide });
      this.ring = new THREE.Mesh(g, m);
      this.ring.renderOrder = 5;
      this.scene.add(this.ring);
    }
    this.ring.visible = true;
    this.ring.position.copy(hit.point).addScaledVector(hit.normal, radius * 0.02);
    this.ring.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), hit.normal);
    this.ring.scale.setScalar(Math.max(1e-3, radius));
    this.request();
  }
  get ringVisible() {
    return !!this.ring?.visible;
  }

  setMirror(list: { sub: number; tris: number[] }[] | null) {
    if (this.mirrorHL) {
      this.group.remove(this.mirrorHL);
      this.mirrorHL.geometry.dispose();
      this.mirrorHL = null;
    }
    const pos: number[] = [];
    let tris = 0;
    const lift = this.radius * 0.004;
    for (const it of list ?? []) {
      const s = this.subs[it.sub];
      if (!s) continue;
      tris += it.tris.length;
      const key = (a: number, b: number) => (a < b ? a * 1e7 + b : b * 1e7 + a);
      const edges = new Map<number, [number, number, number]>();
      for (const t of it.tris) for (let k = 0; k < 3; k++) {
        const a = s.index[t * 3 + k], b = s.index[t * 3 + ((k + 1) % 3)];
        const e = edges.get(key(a, b));
        if (e) e[2]++;
        else edges.set(key(a, b), [a, b, 1]);
      }
      const at = (v: number) => {
        const n = s.nrm && s.nrm.length >= v * 3 + 3 ? [s.nrm[v * 3], s.nrm[v * 3 + 1], s.nrm[v * 3 + 2]] : [0, 0, 0];
        const l = Math.hypot(n[0], n[1], n[2]) || 1;
        return [s.pos[v * 3] + (n[0] / l) * lift, s.pos[v * 3 + 1] + (n[1] / l) * lift, s.pos[v * 3 + 2] + (n[2] / l) * lift];
      };
      for (const [a, b, n] of edges.values()) if (n === 1) pos.push(...at(a), ...at(b));
    }
    this.mirrorTris = pos.length ? tris : 0;
    if (pos.length) {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(pos, 3));
      const line = new THREE.LineSegments(g, new THREE.LineDashedMaterial({ color: 0xff3df0, dashSize: lift * 3, gapSize: lift * 2, transparent: true, opacity: 0.95, depthTest: true }));
      line.computeLineDistances();
      line.renderOrder = 3;
      this.mirrorHL = line;
      this.group.add(line);
    }
    this.request();
  }
  get mirrorTriangles() {
    return this.mirrorTris;
  }
  get mirrorIsOutline() {
    return !this.mirrorHL || this.mirrorHL instanceof THREE.LineSegments;
  }

  screenOf(p: [number, number, number]): [number, number] | null {
    const r = this.canvas.getBoundingClientRect();
    const v = new THREE.Vector3(...p).project(this.camera);
    if (Math.abs(v.x) > 1 || Math.abs(v.y) > 1) return null;
    return [r.left + ((v.x + 1) / 2) * r.width, r.top + ((1 - v.y) / 2) * r.height];
  }

  setBackground(color: string) {
    this.scene.background = new THREE.Color(color);
    this.request();
  }
  request() {
    this.needs = true;
  }
  render() {
    if (!this.renderer) return;
    this.needs = false;
    this.renderer.render(this.scene, this.camera);
  }
  resize() {
    if (!this.renderer) return;
    const host = this.canvas.parentElement!;
    const w = Math.max(1, host.clientWidth), h = Math.max(1, host.clientHeight);
    this.renderer.setSize(w, h, false);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.request();
  }
  get autoRotate() {
    return !!this.controls?.autoRotate;
  }
  set autoRotate(v: boolean) {
    if (this.controls) {
      this.controls.autoRotate = v;
      this.controls.autoRotateSpeed = 2.5;
    }
    this.request();
  }
  setNearest(v: boolean) {
    this.nearest = v;
    for (const t of this.textures.values()) {
      t.magFilter = v ? THREE.NearestFilter : THREE.LinearFilter;
      t.minFilter = v ? THREE.NearestFilter : THREE.LinearMipmapLinearFilter;
      t.generateMipmaps = !v;
      t.needsUpdate = true;
    }
    this.request();
  }

  get current() {
    return this.model;
  }

  clear() {
    for (const s of this.subs) {
      s.mesh.geometry.dispose();
      (s.mesh.material as THREE.Material).dispose();
    }
    this.group.clear();
    this.subs = [];
    this.highlight = this.selectedHL = this.mirrorHL = null;
    this.mirrorTris = 0;
    for (const t of this.textures.values()) t.dispose();
    this.textures.clear();
    this.model = null;
    this.request();
  }

  show(model: ModelData) {
    if (this.model === model) return;
    this.model = model;
    for (const s of this.subs) {
      s.mesh.geometry.dispose();
      (s.mesh.material as THREE.Material).dispose();
    }
    this.group.clear();
    this.subs = [];
    this.clearHL();
    this.mirrorHL = null;
    for (const d of model.submeshes) {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.BufferAttribute(f32(d.positions), 3));
      g.setAttribute("normal", new THREE.BufferAttribute(f32(d.normals), 3));
      g.setAttribute("uv", new THREE.BufferAttribute(f32(d.uvs), 2));
      const index = u32(d.indices);
      g.setIndex(new THREE.BufferAttribute(index, 1));
      const texName = d.texture;
      const mat = new THREE.MeshLambertMaterial({ color: 0xffffff, side: THREE.DoubleSide, alphaTest: 0.25, transparent: d.xlu });
      if (texName) mat.map = this.texture(texName, d);
      const mesh = new THREE.Mesh(g, mat);
      this.group.add(mesh);
      this.subs.push({ mesh, data: d, parts: u16(d.parts), index, texName, pos: g.getAttribute("position").array as Float32Array, uv: g.getAttribute("uv").array as Float32Array, nrm: (g.getAttribute("normal")?.array as Float32Array | undefined) ?? null });
    }
    const lo = new THREE.Vector3(...model.bboxMin), hi = new THREE.Vector3(...model.bboxMax);
    this.center = lo.clone().add(hi).multiplyScalar(0.5);
    this.radius = Math.max(1, hi.distanceTo(lo) / 2);
    this.camera.near = this.radius / 100;
    this.camera.far = this.radius * 100;
    this.resetView();
  }

  resetView() {
    if (!this.controls) return;
    const d = (this.radius / Math.sin(THREE.MathUtils.degToRad(this.camera.fov / 2))) * (this.paintMode ? 0.95 : 0.72);
    this.camera.position.copy(this.center).add(new THREE.Vector3(0.45, 0.25, 1).normalize().multiplyScalar(d));
    this.camera.updateProjectionMatrix();
    this.controls.target.copy(this.center);
    this.controls.update();
    this.request();
  }

  private texture(name: string, d: SubData): THREE.DataTexture {
    let t = this.textures.get(name);
    if (!t) {
      const src = this.source(name);
      const data = src ? new Uint8Array(src.data) : new Uint8Array([200, 200, 200, 255]);
      t = new THREE.DataTexture(data, src?.w ?? 1, src?.h ?? 1, THREE.RGBAFormat);
      t.colorSpace = THREE.SRGBColorSpace;
      t.flipY = false;
      t.wrapS = WRAP[d.wrapS] ?? THREE.RepeatWrapping;
      t.wrapT = WRAP[d.wrapT] ?? THREE.RepeatWrapping;
      t.magFilter = this.nearest ? THREE.NearestFilter : THREE.LinearFilter;
      t.minFilter = this.nearest ? THREE.NearestFilter : THREE.LinearMipmapLinearFilter;
      t.generateMipmaps = !this.nearest;
      t.needsUpdate = true;
      this.textures.set(name, t);
    }
    return t;
  }

  update(name: string) {
    const target = this.textureFor(name);
    const t = this.textures.get(target);
    const src = this.source(name);
    if (!t || !src) return;
    const img = t.image as { data: Uint8Array; width: number; height: number };
    if (img.width !== src.w || img.height !== src.h) {
      t.image = { data: new Uint8Array(src.data), width: src.w, height: src.h } as unknown as typeof t.image;
    } else img.data.set(src.data);
    t.needsUpdate = true;
    this.uploads++;
    this.request();
  }
  hasTexture(name: string) {
    return this.textures.has(this.textureFor(name));
  }

  pickAt(clientX: number, clientY: number): PickResult | null {
    if (!this.renderer || !this.subs.length) return null;
    const r = this.canvas.getBoundingClientRect();
    const ndc = new THREE.Vector2(((clientX - r.left) / r.width) * 2 - 1, -((clientY - r.top) / r.height) * 2 + 1);
    const ray = new THREE.Raycaster();
    ray.setFromCamera(ndc, this.camera);
    const hits = ray.intersectObjects(this.subs.map((s) => s.mesh), false);
    for (const h of hits) {
      const s = this.subs.find((x) => x.mesh === h.object);
      if (!s || !s.texName || h.faceIndex == null) continue;
      const part = s.parts[h.faceIndex];
      if (part === 0xffff) continue;
      return { texture: s.texName, part };
    }
    return null;
  }

  screenOfPart(texture: string, part: number): [number, number] | null {
    const r = this.canvas.getBoundingClientRect();
    for (const s of this.subs) {
      if (s.texName !== texture) continue;
      const pos = s.mesh.geometry.getAttribute("position");
      for (let t = 0; t < s.parts.length; t++) {
        if (s.parts[t] !== part) continue;
        const c = new THREE.Vector3();
        for (let k = 0; k < 3; k++) c.add(new THREE.Vector3().fromBufferAttribute(pos, s.index[t * 3 + k]));
        c.multiplyScalar(1 / 3).project(this.camera);
        if (Math.abs(c.x) > 1 || Math.abs(c.y) > 1) continue;
        const p: [number, number] = [r.left + ((c.x + 1) / 2) * r.width, r.top + ((1 - c.y) / 2) * r.height];
        const hit = this.pickAt(p[0], p[1]);
        if (hit && hit.texture === texture && hit.part === part) return p;
      }
    }
    return null;
  }

  private partMesh(p: PickResult, color: number, opacity: number): THREE.Mesh | null {
    const idx: number[] = [];
    let geo: THREE.BufferGeometry | null = null;
    for (const s of this.subs) {
      if (s.texName !== p.texture) continue;
      for (let t = 0; t < s.parts.length; t++) if (s.parts[t] === p.part) idx.push(s.index[t * 3], s.index[t * 3 + 1], s.index[t * 3 + 2]);
      if (idx.length) {
        geo = new THREE.BufferGeometry();
        geo.setAttribute("position", s.mesh.geometry.getAttribute("position"));
        geo.setIndex(idx);
        break;
      }
    }
    if (!geo) return null;
    const m = new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color, transparent: true, opacity, depthTest: true, side: THREE.DoubleSide, polygonOffset: true, polygonOffsetFactor: -2, polygonOffsetUnits: -2 }));
    m.renderOrder = 2;
    return m;
  }
  private clearHL() {
    for (const m of [this.highlight, this.selectedHL]) if (m) this.group.remove(m);
    this.highlight = this.selectedHL = null;
  }
  setHighlight(p: PickResult | null, selected: boolean) {
    const cur = selected ? this.selectedHL : this.highlight;
    if (cur) {
      this.group.remove(cur);
      cur.geometry.dispose();
    }
    const m = p ? this.partMesh(p, selected ? 0x00d4ff : 0xffffff, selected ? 0.38 : 0.28) : null;
    if (m) this.group.add(m);
    if (selected) this.selectedHL = m;
    else this.highlight = m;
    this.request();
  }

  readCenter(): number[] {
    if (!this.renderer) return [];
    this.render();
    const gl = this.renderer.getContext();
    const w = gl.drawingBufferWidth, h = gl.drawingBufferHeight;
    const s = Math.max(4, Math.floor(Math.min(w, h) / 3));
    const buf = new Uint8Array(s * s * 4);
    gl.readPixels(Math.floor((w - s) / 2), Math.floor((h - s) / 2), s, s, gl.RGBA, gl.UNSIGNED_BYTE, buf);
    const sum = [0, 0, 0];
    for (let i = 0; i < buf.length; i += 4) for (let c = 0; c < 3; c++) sum[c] += buf[i + c];
    return sum.map((v) => v / (s * s));
  }

  dispose() {
    cancelAnimationFrame(this.raf);
    this.renderer?.dispose();
  }
}
