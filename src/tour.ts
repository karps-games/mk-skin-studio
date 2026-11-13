import { onLangChange, t } from "./i18n";

const PAGES = [
  ["tour.welcome", "tour.intro"],
  ["tab.projects", "tour.projects"],
  ["tab.characters", "tour.characters"],
  ["tab.studio", "tour.studio"],
  ["tour.exportTitle", "tour.export"],
] as const;

export function initTutorial() {
  const dialog = document.getElementById("tour-dialog") as HTMLDialogElement;
  const opener = document.getElementById("btn-tour") as HTMLButtonElement;
  const title = document.getElementById("tour-title")!;
  const text = document.getElementById("tour-text")!;
  const progress = document.getElementById("tour-progress")!;
  const back = document.getElementById("tour-back") as HTMLButtonElement;
  const next = document.getElementById("tour-next") as HTMLButtonElement;
  let page = 0;
  const render = () => {
    const [heading, body] = PAGES[page];
    title.textContent = t(heading);
    text.textContent = t(body);
    progress.textContent = t("tour.progress", { n: page + 1, total: PAGES.length });
    back.disabled = page === 0;
    back.textContent = t("tour.back");
    next.textContent = t(page === PAGES.length - 1 ? "tour.done" : "tour.next");
  };
  opener.addEventListener("click", () => {
    page = 0;
    render();
    dialog.showModal();
  });
  back.addEventListener("click", () => { page = Math.max(0, page - 1); render(); });
  next.addEventListener("click", () => {
    if (page === PAGES.length - 1) dialog.close();
    else { page++; render(); }
  });
  document.getElementById("tour-close")!.addEventListener("click", () => dialog.close());
  dialog.addEventListener("close", () => opener.focus());
  dialog.addEventListener("click", (e) => {
    if (e.target !== dialog) return;
    const r = dialog.getBoundingClientRect();
    if (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom) dialog.close();
  });
  onLangChange(() => { if (dialog.open) render(); });
}
