const CRC = (() => {
  const t = new Uint32Array(256);
  for (let n = 0; n < 256; n++) {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    t[n] = c >>> 0;
  }
  return t;
})();
function crc32(parts: Uint8Array[]): number {
  let c = 0xffffffff;
  for (const p of parts) for (let i = 0; i < p.length; i++) c = CRC[(c ^ p[i]) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}
async function pipe(data: Uint8Array, stream: CompressionStream | DecompressionStream): Promise<Uint8Array> {
  const body = new Blob([data as BlobPart]).stream().pipeThrough(stream as unknown as ReadableWritablePair<Uint8Array, Uint8Array>);
  return new Uint8Array(await new Response(body).arrayBuffer());
}
function chunk(type: string, data: Uint8Array): Uint8Array {
  const out = new Uint8Array(12 + data.length);
  const dv = new DataView(out.buffer);
  dv.setUint32(0, data.length);
  const t = new TextEncoder().encode(type);
  out.set(t, 4);
  out.set(data, 8);
  dv.setUint32(8 + data.length, crc32([t, data]));
  return out;
}

export async function encodePng(px: Uint8Array | Uint8ClampedArray, w: number, h: number): Promise<Uint8Array> {
  const stride = w * 4;
  const raw = new Uint8Array((stride + 1) * h);
  for (let y = 0; y < h; y++) {
    const o = y * (stride + 1);
    raw[o] = 1;
    const s = y * stride;
    for (let x = 0; x < stride; x++) raw[o + 1 + x] = (px[s + x] - (x >= 4 ? px[s + x - 4] : 0)) & 0xff;
  }
  const ihdr = new Uint8Array(13);
  const dv = new DataView(ihdr.buffer);
  dv.setUint32(0, w);
  dv.setUint32(4, h);
  ihdr.set([8, 6, 0, 0, 0], 8);
  const idat = await pipe(raw, new CompressionStream("deflate"));
  const parts = [new Uint8Array([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", idat), chunk("IEND", new Uint8Array(0))];
  const out = new Uint8Array(parts.reduce((a, p) => a + p.length, 0));
  let o = 0;
  for (const p of parts) {
    out.set(p, o);
    o += p.length;
  }
  return out;
}

function paeth(a: number, b: number, c: number) {
  const p = a + b - c;
  const pa = Math.abs(p - a), pb = Math.abs(p - b), pc = Math.abs(p - c);
  return pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
}

export interface Rgba {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

export async function decodePng(bytes: Uint8Array): Promise<Rgba> {
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let p = 8;
  let w = 0, h = 0, depth = 0, ctype = 0, interlace = 0;
  const idat: Uint8Array[] = [];
  while (p + 8 <= bytes.length) {
    const len = dv.getUint32(p);
    const type = String.fromCharCode(...bytes.subarray(p + 4, p + 8));
    const data = bytes.subarray(p + 8, p + 8 + len);
    if (type === "IHDR") {
      w = dv.getUint32(p + 8);
      h = dv.getUint32(p + 12);
      depth = data[8];
      ctype = data[9];
      interlace = data[12];
    } else if (type === "IDAT") idat.push(data);
    else if (type === "IEND") break;
    p += 12 + len;
  }
  const channels = ({ 0: 1, 2: 3, 4: 2, 6: 4 } as Record<number, number>)[ctype];
  if (depth !== 8 || !channels || interlace !== 0) return decodeViaCanvas(bytes);
  const all = new Uint8Array(idat.reduce((a, c) => a + c.length, 0));
  let o = 0;
  for (const c of idat) {
    all.set(c, o);
    o += c.length;
  }
  const raw = await pipe(all, new DecompressionStream("deflate"));
  const stride = w * channels;
  const cur = new Uint8Array(stride);
  const prev = new Uint8Array(stride);
  const out = new Uint8ClampedArray(w * h * 4);
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)];
    const row = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    for (let x = 0; x < stride; x++) {
      const a = x >= channels ? cur[x - channels] : 0;
      const b = prev[x];
      const c = x >= channels ? prev[x - channels] : 0;
      const v = row[x];
      cur[x] = (f === 0 ? v : f === 1 ? v + a : f === 2 ? v + b : f === 3 ? v + ((a + b) >> 1) : v + paeth(a, b, c)) & 0xff;
    }
    for (let x = 0; x < w; x++) {
      const d = (y * w + x) * 4;
      const s = x * channels;
      if (channels === 4) out.set(cur.subarray(s, s + 4), d);
      else if (channels === 3) (out[d] = cur[s]), (out[d + 1] = cur[s + 1]), (out[d + 2] = cur[s + 2]), (out[d + 3] = 255);
      else if (channels === 2) (out[d] = out[d + 1] = out[d + 2] = cur[s]), (out[d + 3] = cur[s + 1]);
      else (out[d] = out[d + 1] = out[d + 2] = cur[s]), (out[d + 3] = 255);
    }
    prev.set(cur);
  }
  return { width: w, height: h, data: out };
}

async function decodeViaCanvas(bytes: Uint8Array): Promise<Rgba> {
  const bmp = await createImageBitmap(new Blob([bytes as BlobPart], { type: "image/png" }), { premultiplyAlpha: "none", colorSpaceConversion: "none" });
  const c = document.createElement("canvas");
  c.width = bmp.width;
  c.height = bmp.height;
  const g = c.getContext("2d", { willReadFrequently: true })!;
  g.drawImage(bmp, 0, 0);
  return { width: c.width, height: c.height, data: g.getImageData(0, 0, c.width, c.height).data };
}

export function b64ToBytes(b64: string): Uint8Array {
  const s = atob(b64.replace(/^data:[^,]*,/, ""));
  const out = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) out[i] = s.charCodeAt(i);
  return out;
}
export function bytesToB64(b: Uint8Array): string {
  let s = "";
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode(...b.subarray(i, i + 0x8000));
  return btoa(s);
}
export const decodeDataUrl = (url: string) => decodePng(b64ToBytes(url));
export async function pngDataUrl(px: Uint8Array | Uint8ClampedArray, w: number, h: number) {
  return "data:image/png;base64," + bytesToB64(await encodePng(px, w, h));
}
