import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const langs = ["en", "pt", "es", "fr", "ja"];
const parse = (l) => {
  const src = readFileSync(join(root, "src/i18n", `${l}.ts`), "utf8");
  const map = new Map();
  for (const m of src.matchAll(/^\s*"([^"]+)":\s*"((?:[^"\\]|\\.)*)",?\s*$/gm)) {
    if (map.has(m[1])) throw new Error(`${l}: duplicate key ${m[1]}`);
    map.set(m[1], m[2]);
  }
  return map;
};
const dicts = Object.fromEntries(langs.map((l) => [l, parse(l)]));
const errors = [];
const ph = (s) => [...s.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort().join(",");
const en = dicts.en;
for (const l of langs.slice(1)) {
  const d = dicts[l];
  for (const k of en.keys()) if (!d.has(k)) errors.push(`${l}: missing ${k}`);
  for (const k of d.keys()) if (!en.has(k)) errors.push(`${l}: extra ${k}`);
  for (const [k, v] of d) {
    if (en.has(k) && ph(v) !== ph(en.get(k))) errors.push(`${l}: placeholders differ for ${k}: {${ph(v)}} vs {${ph(en.get(k))}}`);
    if (!v.trim()) errors.push(`${l}: empty ${k}`);
  }
}
const html = readFileSync(join(root, "index.html"), "utf8");
const used = new Set([...html.matchAll(/data-i18n(?:-title|-ph|-aria)?="([^"]+)"/g)].map((m) => m[1]));
const walk = (dir) => readdirSync(dir).flatMap((f) => (statSync(join(dir, f)).isDirectory() ? walk(join(dir, f)) : [join(dir, f)]));
for (const f of walk(join(root, "src")).filter((f) => f.endsWith(".ts") && !/[\\/]i18n[\\/]/.test(f))) {
  const s = readFileSync(f, "utf8");
  for (const m of s.matchAll(/\bt\("([^"]+)"/g)) used.add(m[1]);
  for (const m of s.matchAll(/data-i18n(?:-title|-ph|-aria)?="([^"$]+)"/g)) used.add(m[1]);
}
const dynamic = {
  tool: ["brush", "eraser", "fill", "picker", "line", "rect", "select", "wand", "move", "hand"],
  blend: ["normal", "multiply", "screen", "overlay", "color", "hue"],
  color: ["red", "orange", "yellow", "green", "cyan", "blue", "purple", "pink", "brown", "beige", "white", "gray", "black"],
  part: ["head", "face", "hands", "arms", "torso", "legs", "feet", "eyes", "hair", "cap", "crown", "shell", "dress", "emblem", "gloves", "shoes", "skin", "other"],
};
for (const [p, ks] of Object.entries(dynamic)) for (const k of ks) used.add(`${p}.${k}`);
for (const k of used) if (!en.has(k)) errors.push(`used but undefined: ${k}`);
for (const l of langs) for (const [k, v] of dicts[l]) if (/MK Skin Studio/.test(v) && /Mario Kart Skin/i.test(v)) errors.push(`${l}: ${k} misnames the product`);
if (!langs.every((l) => dicts[l].get("legal.na"))) errors.push("legal.na missing");
console.log(`i18n: ${en.size} keys x ${langs.length} languages, ${used.size} keys referenced`);
if (errors.length) {
  console.error(errors.join("\n"));
  process.exit(1);
}
console.log("i18n parity OK");
