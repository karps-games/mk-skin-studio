import en, { type Dict } from "./en";
import pt from "./pt";
import es from "./es";
import fr from "./fr";
import ja from "./ja";
import { keys } from "../platform";

export const LANGS = [
  { code: "en", name: "English", locale: "en-US" },
  { code: "pt", name: "Português", locale: "pt-BR" },
  { code: "es", name: "Español", locale: "es-ES" },
  { code: "fr", name: "Français", locale: "fr-FR" },
  { code: "ja", name: "日本語", locale: "ja-JP" },
] as const;
export type Lang = (typeof LANGS)[number]["code"];
export type Key = keyof Dict;

export const DICTS: Record<Lang, Dict> = { en, pt, es, fr, ja };
const STORE = "karps.lang";
const isLang = (v: unknown): v is Lang => LANGS.some((l) => l.code === v);

let lang: Lang = (() => {
  try {
    const saved = localStorage.getItem(STORE);
    return isLang(saved) ? saved : "en";
  } catch {
    return "en";
  }
})();
const listeners: (() => void)[] = [];

export const getLang = () => lang;
export const locale = () => LANGS.find((l) => l.code === lang)!.locale;

export function t(key: Key, vars?: Record<string, string | number>): string {
  let s = DICTS[lang][key] ?? en[key] ?? key;
  if (vars) for (const [k, v] of Object.entries(vars)) s = s.split(`{${k}}`).join(String(v));
  return keys(s);
}

export function errText(raw: unknown): string {
  const s = String(raw);
  if (!s.startsWith("E:")) return s;
  const [code, ...params] = s.slice(2).split("|");
  const vars: Record<string, string> = {};
  for (const p of params) {
    const i = p.indexOf("=");
    if (i > 0) vars[p.slice(0, i)] = p.slice(i + 1);
  }
  const key = `err.${code}` as Key;
  if (!(key in en)) return s;
  if (!("file" in vars)) vars.file = "";
  return t(key, vars);
}

export function applyStatic(root: ParentNode = document) {
  root.querySelectorAll<HTMLElement>("[data-i18n]").forEach((el) => (el.textContent = t(el.dataset.i18n as Key)));
  root.querySelectorAll<HTMLElement>("[data-i18n-title]").forEach((el) => (el.title = t(el.dataset.i18nTitle as Key)));
  root.querySelectorAll<HTMLInputElement>("[data-i18n-ph]").forEach((el) => (el.placeholder = t(el.dataset.i18nPh as Key)));
  root.querySelectorAll<HTMLElement>("[data-i18n-aria]").forEach((el) => el.setAttribute("aria-label", t(el.dataset.i18nAria as Key)));
  root.querySelectorAll<HTMLElement>("[data-keys]").forEach((el) => (el.textContent = keys(el.textContent ?? "")));
  document.documentElement.lang = locale();
}

export function setLang(l: Lang) {
  if (l === lang) return;
  lang = l;
  try {
    localStorage.setItem(STORE, l);
  } catch {}
  applyStatic();
  listeners.forEach((fn) => fn());
}

export const onLangChange = (fn: () => void) => listeners.push(fn);

export function flagSvg(l: Lang, size = 20): string {
  const h = Math.round((size * 2) / 3);
  const open = `<svg class="flag" width="${size}" height="${h}" viewBox="0 0 30 20" aria-hidden="true">`;
  switch (l) {
    case "en": {
      const stripes = [0, 2, 4, 6, 8, 10, 12].map((i) => `<rect y="${(i * 20) / 13}" width="30" height="${20 / 13}" fill="#b22234"/>`).join("");
      return `${open}<rect width="30" height="20" fill="#fff"/>${stripes}<rect width="13" height="${(7 * 20) / 13}" fill="#3c3b6e"/></svg>`;
    }
    case "pt":
      return `${open}<rect width="30" height="20" fill="#009c3b"/><path d="M15 2.2 27.6 10 15 17.8 2.4 10Z" fill="#ffdf00"/><circle cx="15" cy="10" r="4.6" fill="#002776"/></svg>`;
    case "es":
      return `${open}<rect width="30" height="20" fill="#aa151b"/><rect y="5" width="30" height="10" fill="#f1bf00"/></svg>`;
    case "fr":
      return `${open}<rect width="10" height="20" fill="#002654"/><rect x="10" width="10" height="20" fill="#fff"/><rect x="20" width="10" height="20" fill="#ce1126"/></svg>`;
    case "ja":
      return `${open}<rect width="30" height="20" fill="#fff"/><circle cx="15" cy="10" r="6" fill="#bc002d"/></svg>`;
  }
}

export function mountLangPicker(host: HTMLElement) {
  const render = () => {
    host.innerHTML = `<button type="button" class="lang-btn" aria-haspopup="listbox" title="${t("top.lang")}">${flagSvg(lang)}<span>${lang.toUpperCase()}</span><span class="lang-caret" aria-hidden="true">▾</span></button>
      <ul class="lang-menu hidden" role="listbox">${LANGS.map(
        (l) => `<li role="option" data-l="${l.code}" class="${l.code === lang ? "on" : ""}">${flagSvg(l.code)}<span>${l.name}</span></li>`,
      ).join("")}</ul>`;
    const menu = host.querySelector(".lang-menu")!;
    host.querySelector(".lang-btn")!.addEventListener("click", (e) => {
      e.stopPropagation();
      menu.classList.toggle("hidden");
    });
    host.querySelectorAll<HTMLElement>("li[data-l]").forEach((li) =>
      li.addEventListener("click", () => {
        setLang(li.dataset.l as Lang);
        render();
      }),
    );
  };
  document.addEventListener("click", () => host.querySelector(".lang-menu")?.classList.add("hidden"));
  render();
}
