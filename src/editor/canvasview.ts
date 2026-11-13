export interface ViewHooks {
  down(x: number, y: number, e: PointerEvent): void;
  move(x: number, y: number, e: PointerEvent): void;
  up(x: number, y: number, e: PointerEvent): void;
  hover(x: number, y: number | null): void;
}

export class CanvasView {
  zoom = 1;
  fit = true;
  ox = 0;
  oy = 0;
  grid = true;
  outlines = true;
  private img = document.createElement("canvas");
  private g: CanvasRenderingContext2D;
  private dpr = 1;
  private panning: { x: number; y: number; ox: number; oy: number } | null = null;
  private space = false;
  hand = false;
  private ants = 0;
  private cursor: [number, number] | null = null;
  w = 1;
  h = 1;
  partOutlines: number[] = [];
  focusOutline: number[] = [];
  hoverOutline: number[] = [];
  selectionOutline: number[] = [];
  private unusedImg: HTMLCanvasElement | null = null;
  unusedCount = 0;
  setUnused(mask: Uint8Array | null) {
    this.unusedImg = null;
    this.unusedCount = 0;
    if (mask && mask.length === this.w * this.h) {
      const c = document.createElement("canvas");
      c.width = this.w;
      c.height = this.h;
      const id = new ImageData(this.w, this.h);
      for (let i = 0; i < mask.length; i++) {
        if (!mask[i]) continue;
        this.unusedCount++;
        const x = i % this.w, y = (i / this.w) | 0;
        const stripe = (x + y) % 4 === 0;
        id.data.set(stripe ? [255, 61, 240, 150] : [0, 0, 0, 120], i * 4);
      }
      c.getContext("2d")!.putImageData(id, 0, 0);
      this.unusedImg = c;
    }
    this.request();
  }
  brushSize = 0;
  preview: ((g: CanvasRenderingContext2D, px: (x: number, y: number) => [number, number], z: number) => void) | null = null;
  private dirty = true;

  constructor(public canvas: HTMLCanvasElement, private hooks: ViewHooks) {
    this.g = canvas.getContext("2d")!;
    new ResizeObserver(() => this.resize()).observe(canvas.parentElement!);
    canvas.addEventListener("wheel", (e) => this.onWheel(e), { passive: false });
    canvas.addEventListener("pointerdown", (e) => this.onDown(e));
    canvas.addEventListener("pointermove", (e) => this.onMove(e));
    canvas.addEventListener("pointerup", (e) => this.onUp(e));
    canvas.addEventListener("pointercancel", (e) => this.onUp(e));
    canvas.addEventListener("pointerleave", () => {
      this.cursor = null;
      this.hooks.hover(-1, null);
      this.request();
    });
    canvas.addEventListener("contextmenu", (e) => e.preventDefault());
    window.addEventListener("keydown", (e) => {
      if (e.code === "Space" && !isTyping(e)) {
        this.space = true;
        canvas.style.cursor = "grab";
        e.preventDefault();
      }
    });
    window.addEventListener("keyup", (e) => {
      if (e.code === "Space") {
        this.space = false;
        canvas.style.cursor = "";
      }
    });
    const tick = () => {
      requestAnimationFrame(tick);
      if (this.selectionOutline.length) {
        this.ants = (this.ants + 0.35) % 8;
        this.dirty = true;
      }
      if (this.dirty) this.draw();
    };
    tick();
  }

  request() {
    this.dirty = true;
  }

  setImage(w: number, h: number) {
    if (w !== this.w || h !== this.h) this.unusedImg = null;
    this.w = w;
    this.h = h;
    this.img.width = w;
    this.img.height = h;
    this.fit = true;
    this.layout();
  }
  update(px: Uint8ClampedArray) {
    const ig = this.img.getContext("2d")!;
    ig.putImageData(new ImageData(new Uint8ClampedArray(px), this.w, this.h), 0, 0);
    this.request();
  }

  private resize() {
    const host = this.canvas.parentElement!;
    this.dpr = window.devicePixelRatio || 1;
    const cw = Math.max(1, host.clientWidth), ch = Math.max(1, host.clientHeight);
    this.canvas.width = Math.round(cw * this.dpr);
    this.canvas.height = Math.round(ch * this.dpr);
    this.canvas.style.width = cw + "px";
    this.canvas.style.height = ch + "px";
    this.layout();
  }
  get viewW() {
    return this.canvas.width / this.dpr;
  }
  get viewH() {
    return this.canvas.height / this.dpr;
  }
  fitZoom() {
    const z = Math.min((this.viewW - 24) / this.w, (this.viewH - 24) / this.h);
    return z >= 1 ? Math.min(32, Math.floor(z)) : Math.max(0.1, z);
  }
  layout() {
    if (this.fit) {
      this.zoom = this.fitZoom();
      this.ox = Math.round((this.viewW - this.w * this.zoom) / 2);
      this.oy = Math.round((this.viewH - this.h * this.zoom) / 2);
    }
    this.request();
    this.onZoom(this.zoom);
  }
  onZoom: (z: number) => void = () => {};

  setZoom(z: number, cx = this.viewW / 2, cy = this.viewH / 2) {
    z = Math.max(1, Math.min(32, Math.round(z)));
    if (z < 1) z = 1;
    const tx = (cx - this.ox) / this.zoom, ty = (cy - this.oy) / this.zoom;
    this.fit = false;
    this.zoom = z;
    this.ox = Math.round(cx - tx * z);
    this.oy = Math.round(cy - ty * z);
    this.request();
    this.onZoom(z);
  }
  setFit() {
    this.fit = true;
    this.layout();
  }

  toTex(clientX: number, clientY: number): [number, number] {
    const r = this.canvas.getBoundingClientRect();
    return [(clientX - r.left - this.ox) / this.zoom, (clientY - r.top - this.oy) / this.zoom];
  }

  private onWheel(e: WheelEvent) {
    e.preventDefault();
    const r = this.canvas.getBoundingClientRect();
    const steps = [1, 2, 3, 4, 6, 8, 12, 16, 24, 32];
    const cur = this.zoom;
    let next: number;
    if (e.deltaY < 0) next = steps.find((s) => s > cur + 0.01) ?? 32;
    else next = [...steps].reverse().find((s) => s < cur - 0.01) ?? 1;
    this.setZoom(next, e.clientX - r.left, e.clientY - r.top);
  }
  private onDown(e: PointerEvent) {
    try {
      this.canvas.setPointerCapture(e.pointerId);
    } catch {}
    if (e.button === 1 || this.space || this.hand) {
      e.preventDefault();
      this.panning = { x: e.clientX, y: e.clientY, ox: this.ox, oy: this.oy };
      this.canvas.style.cursor = "grabbing";
      return;
    }
    const [x, y] = this.toTex(e.clientX, e.clientY);
    this.hooks.down(x, y, e);
  }
  private onMove(e: PointerEvent) {
    if (this.panning) {
      this.fit = false;
      this.ox = this.panning.ox + e.clientX - this.panning.x;
      this.oy = this.panning.oy + e.clientY - this.panning.y;
      this.request();
      return;
    }
    const [x, y] = this.toTex(e.clientX, e.clientY);
    this.cursor = [x, y];
    this.hooks.hover(x, y);
    if (e.buttons) this.hooks.move(x, y, e);
    this.request();
  }
  private onUp(e: PointerEvent) {
    if (this.panning) {
      this.panning = null;
      this.canvas.style.cursor = this.space || this.hand ? "grab" : "";
      return;
    }
    const [x, y] = this.toTex(e.clientX, e.clientY);
    this.hooks.up(x, y, e);
  }

  private segs(g: CanvasRenderingContext2D, s: number[]) {
    g.beginPath();
    const z = this.zoom;
    for (let i = 0; i + 3 < s.length; i += 4) {
      g.moveTo(this.ox + s[i] * z + 0.5, this.oy + s[i + 1] * z + 0.5);
      g.lineTo(this.ox + s[i + 2] * z + 0.5, this.oy + s[i + 3] * z + 0.5);
    }
  }

  draw() {
    this.dirty = false;
    const g = this.g;
    const z = this.zoom;
    g.setTransform(this.dpr, 0, 0, this.dpr, 0, 0);
    g.clearRect(0, 0, this.viewW, this.viewH);
    const W = this.w * z, H = this.h * z;
    const css = getComputedStyle(this.canvas);
    const ca = css.getPropertyValue("--checker-a").trim() || "#ddd";
    const cb = css.getPropertyValue("--checker-b").trim() || "#fff";
    g.save();
    g.beginPath();
    g.rect(this.ox, this.oy, W, H);
    g.clip();
    g.fillStyle = cb;
    g.fillRect(this.ox, this.oy, W, H);
    g.fillStyle = ca;
    const q = 8;
    const x0 = Math.max(this.ox, 0), y0 = Math.max(this.oy, 0);
    const x1 = Math.min(this.ox + W, this.viewW), y1 = Math.min(this.oy + H, this.viewH);
    for (let y = Math.floor((y0 - this.oy) / q); this.oy + y * q < y1; y++)
      for (let x = Math.floor((x0 - this.ox) / q); this.ox + x * q < x1; x++) if ((x + y) % 2) g.fillRect(this.ox + x * q, this.oy + y * q, q, q);
    g.imageSmoothingEnabled = false;
    g.drawImage(this.img, this.ox, this.oy, W, H);
    if (this.unusedImg) g.drawImage(this.unusedImg, this.ox, this.oy, W, H);
    if (this.grid && z >= 8) {
      g.strokeStyle = "rgba(127,127,127,0.35)";
      g.lineWidth = 1;
      g.beginPath();
      for (let x = Math.max(0, Math.floor(-this.ox / z)); x <= this.w && this.ox + x * z <= this.viewW; x++) {
        g.moveTo(this.ox + x * z + 0.5, Math.max(0, this.oy));
        g.lineTo(this.ox + x * z + 0.5, Math.min(this.viewH, this.oy + H));
      }
      for (let y = Math.max(0, Math.floor(-this.oy / z)); y <= this.h && this.oy + y * z <= this.viewH; y++) {
        g.moveTo(Math.max(0, this.ox), this.oy + y * z + 0.5);
        g.lineTo(Math.min(this.viewW, this.ox + W), this.oy + y * z + 0.5);
      }
      g.stroke();
    }
    g.restore();
    if (this.outlines && this.partOutlines.length) {
      this.segs(g, this.partOutlines);
      g.strokeStyle = "rgba(0,0,0,0.55)";
      g.lineWidth = 2.5;
      g.stroke();
      g.strokeStyle = "rgba(255,255,255,0.8)";
      g.lineWidth = 1;
      g.stroke();
    }
    if (this.hoverOutline.length) {
      this.segs(g, this.hoverOutline);
      g.strokeStyle = "rgba(255,255,255,0.95)";
      g.lineWidth = 2;
      g.stroke();
    }
    if (this.focusOutline.length) {
      this.segs(g, this.focusOutline);
      g.strokeStyle = "#00d4ff";
      g.lineWidth = 2;
      g.stroke();
    }
    if (this.selectionOutline.length) {
      this.segs(g, this.selectionOutline);
      g.lineWidth = 1;
      g.setLineDash([4, 4]);
      g.strokeStyle = "#000";
      g.lineDashOffset = -this.ants;
      g.stroke();
      g.strokeStyle = "#fff";
      g.lineDashOffset = -this.ants + 4;
      g.stroke();
      g.setLineDash([]);
    }
    if (this.preview) this.preview(g, (x, y) => [this.ox + x * z, this.oy + y * z], z);
    if (this.cursor && this.brushSize > 0) {
      const [cx, cy] = this.cursor;
      g.beginPath();
      const r = Math.max(0.5, this.brushSize / 2) * z;
      g.arc(this.ox + cx * z, this.oy + cy * z, Math.max(2, r), 0, Math.PI * 2);
      g.strokeStyle = "rgba(0,0,0,0.7)";
      g.lineWidth = 2.5;
      g.stroke();
      g.strokeStyle = "#fff";
      g.lineWidth = 1;
      g.stroke();
    }
    g.strokeStyle = "rgba(127,127,127,0.6)";
    g.lineWidth = 1;
    g.strokeRect(this.ox - 0.5, this.oy - 0.5, W + 1, H + 1);
  }
}

export function isTyping(e: KeyboardEvent) {
  const t = e.target as HTMLElement | null;
  return !!t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
}
