import { hsvToRgb, rgbToHsv } from "./doc";
import { hex, parseHex, type RGBA } from "./paint";

export class ColorPicker {
  primary: RGBA = [255, 0, 0, 255];
  secondary: RGBA = [255, 255, 255, 255];
  recent: RGBA[] = [];
  palette: RGBA[] = [];
  private h = 0;
  private s = 1;
  private v = 1;
  onChange: (c: RGBA) => void = () => {};
  private sv: HTMLCanvasElement;
  private hue: HTMLCanvasElement;
  private alpha: HTMLCanvasElement;
  private hexInput: HTMLInputElement;

  constructor(private root: HTMLElement) {
    root.innerHTML = `
      <div class="cp-main">
        <canvas class="cp-sv" width="160" height="100"></canvas>
        <canvas class="cp-hue" width="14" height="100"></canvas>
        <canvas class="cp-alpha checker" width="14" height="100"></canvas>
        <div class="cp-side">
          <div class="cp-swatches">
            <button type="button" class="cp-sw cp-secondary checker" data-i18n-title="cp.secondary"><i></i></button>
            <button type="button" class="cp-sw cp-primary checker" data-i18n-title="cp.primary"><i></i></button>
          </div>
          <button type="button" class="icon-btn cp-swap" data-i18n-title="cp.swap"><svg viewBox="0 0 24 24" aria-hidden="true"><path fill="currentColor" d="M7 7h9.2l-2.6-2.6L15 3l5 5-5 5-1.4-1.4L16.2 9H7V7Zm10 10H7.8l2.6 2.6L9 21l-5-5 5-5 1.4 1.4L7.8 15H17v2Z"/></svg></button>
          <input class="cp-hex" spellcheck="false" maxlength="9" data-i18n-title="cp.hex" />
        </div>
      </div>
      <div class="cp-row"><span data-i18n="cp.recent"></span><div class="cp-recent"></div></div>
      <div class="cp-row"><span data-i18n="cp.palette"></span><div class="cp-palette"></div></div>`;
    this.sv = root.querySelector(".cp-sv")!;
    this.hue = root.querySelector(".cp-hue")!;
    this.alpha = root.querySelector(".cp-alpha")!;
    this.hexInput = root.querySelector(".cp-hex")!;
    const drag = (el: HTMLCanvasElement, fn: (fx: number, fy: number) => void) => {
      const at = (e: PointerEvent) => {
        const r = el.getBoundingClientRect();
        fn(Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)), Math.max(0, Math.min(1, (e.clientY - r.top) / r.height)));
      };
      el.addEventListener("pointerdown", (e) => {
        try {
          el.setPointerCapture(e.pointerId);
        } catch {}
        at(e);
      });
      el.addEventListener("pointermove", (e) => e.buttons && at(e));
    };
    drag(this.sv, (fx, fy) => {
      this.s = fx;
      this.v = 1 - fy;
      this.fromHsv();
    });
    drag(this.hue, (_, fy) => {
      this.h = fy * 360;
      this.fromHsv();
    });
    drag(this.alpha, (_, fy) => {
      this.primary = [this.primary[0], this.primary[1], this.primary[2], Math.round((1 - fy) * 255)];
      this.emit();
    });
    this.hexInput.addEventListener("change", () => {
      const c = parseHex(this.hexInput.value);
      if (c) this.set(c);
      else this.render();
    });
    root.querySelector(".cp-swap")!.addEventListener("click", () => this.swap());
    root.querySelector(".cp-secondary")!.addEventListener("click", () => this.swap());
    this.set(this.primary);
  }

  private fromHsv() {
    const [r, g, b] = hsvToRgb(this.h, this.s, this.v);
    this.primary = [Math.round(r * 255), Math.round(g * 255), Math.round(b * 255), this.primary[3]];
    this.emit();
  }
  private emit() {
    this.render();
    this.onChange(this.primary);
  }
  set(c: RGBA, secondary = false) {
    if (secondary) {
      this.secondary = [...c] as RGBA;
      this.render();
      return;
    }
    this.primary = [...c] as RGBA;
    const [h, s, v] = rgbToHsv(c[0] / 255, c[1] / 255, c[2] / 255);
    if (s > 0 && v > 0) this.h = h;
    this.s = s;
    this.v = v;
    this.emit();
  }
  swap() {
    const p = this.primary;
    this.primary = this.secondary;
    this.secondary = p;
    this.set(this.primary);
  }
  use(c: RGBA) {
    const k = hex(c);
    this.recent = [c, ...this.recent.filter((x) => hex(x) !== k)].slice(0, 12);
    this.renderRows();
  }
  setPalette(p: RGBA[]) {
    this.palette = p;
    this.renderRows();
  }
  private swatches(host: HTMLElement, list: RGBA[]) {
    host.innerHTML = list.map((c, i) => `<button type="button" class="cp-chip" data-i="${i}" title="${hex(c)}" style="background:${hex(c)}"></button>`).join("");
    host.querySelectorAll<HTMLButtonElement>(".cp-chip").forEach((b) =>
      b.addEventListener("click", (e) => {
        const c = list[Number(b.dataset.i)];
        if ((e as MouseEvent).altKey) this.set(c, true);
        else this.set(c);
      }),
    );
  }
  private renderRows() {
    this.swatches(this.root.querySelector(".cp-recent")!, this.recent);
    this.swatches(this.root.querySelector(".cp-palette")!, this.palette);
  }
  render() {
    const [r, g, b, a] = this.primary;
    const sg = this.sv.getContext("2d")!;
    const W = this.sv.width, H = this.sv.height;
    const [hr, hg, hb] = hsvToRgb(this.h, 1, 1);
    sg.fillStyle = `rgb(${hr * 255},${hg * 255},${hb * 255})`;
    sg.fillRect(0, 0, W, H);
    let gr = sg.createLinearGradient(0, 0, W, 0);
    gr.addColorStop(0, "#fff");
    gr.addColorStop(1, "rgba(255,255,255,0)");
    sg.fillStyle = gr;
    sg.fillRect(0, 0, W, H);
    gr = sg.createLinearGradient(0, 0, 0, H);
    gr.addColorStop(0, "rgba(0,0,0,0)");
    gr.addColorStop(1, "#000");
    sg.fillStyle = gr;
    sg.fillRect(0, 0, W, H);
    sg.beginPath();
    sg.arc(this.s * W, (1 - this.v) * H, 5, 0, Math.PI * 2);
    sg.strokeStyle = this.v > 0.5 ? "#000" : "#fff";
    sg.lineWidth = 2;
    sg.stroke();
    const hgx = this.hue.getContext("2d")!;
    const hh = this.hue.height;
    const hgr = hgx.createLinearGradient(0, 0, 0, hh);
    for (let i = 0; i <= 6; i++) {
      const [rr, gg, bb] = hsvToRgb(i * 60, 1, 1);
      hgr.addColorStop(i / 6, `rgb(${rr * 255},${gg * 255},${bb * 255})`);
    }
    hgx.fillStyle = hgr;
    hgx.fillRect(0, 0, this.hue.width, hh);
    hgx.fillStyle = "#000";
    hgx.fillRect(0, (this.h / 360) * hh - 1, this.hue.width, 3);
    hgx.fillStyle = "#fff";
    hgx.fillRect(0, (this.h / 360) * hh, this.hue.width, 1);
    const ag = this.alpha.getContext("2d")!;
    ag.clearRect(0, 0, this.alpha.width, this.alpha.height);
    const agr = ag.createLinearGradient(0, 0, 0, this.alpha.height);
    agr.addColorStop(0, `rgb(${r},${g},${b})`);
    agr.addColorStop(1, `rgba(${r},${g},${b},0)`);
    ag.fillStyle = agr;
    ag.fillRect(0, 0, this.alpha.width, this.alpha.height);
    ag.fillStyle = "#000";
    ag.fillRect(0, (1 - a / 255) * this.alpha.height - 1, this.alpha.width, 3);
    ag.fillStyle = "#fff";
    ag.fillRect(0, (1 - a / 255) * this.alpha.height, this.alpha.width, 1);
    (this.root.querySelector(".cp-primary i") as HTMLElement).style.background = `rgba(${r},${g},${b},${a / 255})`;
    const s = this.secondary;
    (this.root.querySelector(".cp-secondary i") as HTMLElement).style.background = `rgba(${s[0]},${s[1]},${s[2]},${s[3] / 255})`;
    if (document.activeElement !== this.hexInput) this.hexInput.value = hex(this.primary);
  }
}
