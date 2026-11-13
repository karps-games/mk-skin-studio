import { invoke } from "@tauri-apps/api/core";
import { onLangChange, t } from "./i18n";

interface GameCandidate {
  path: string;
  id: string;
  region: string;
  format: "ISO" | "WBFS" | "RVZ" | "WIA";
  sizeBytes: number;
}
interface DiscoveryResult { games: GameCandidate[]; truncated: boolean }
const LAST_GAME = "mkss.lastGamePath";
function lastPath(): string | null {
  try { return localStorage.getItem(LAST_GAME); } catch { return null; }
}

export class GameDiscovery {
  private games: GameCandidate[] = [];
  private selected = "";
  private manual = false;
  private epoch = 0;
  private pending = false;
  private opening = false;
  private truncated = false;
  private phase: "idle" | "scanning" | "done" | "error" = "idle";
  private readonly status = document.getElementById("game-scan-state")!;
  private readonly results = document.getElementById("game-detected")!;
  private readonly select = document.getElementById("detected-game") as HTMLSelectElement;
  private readonly path = document.getElementById("detected-path")!;
  private readonly rescan = document.getElementById("btn-scan-game") as HTMLButtonElement;
  private readonly use = document.getElementById("btn-use-detected") as HTMLButtonElement;
  private readonly limit = document.getElementById("game-scan-limit")!;

  constructor(private readonly openGame: (path: string) => Promise<boolean>) {
    this.rescan.addEventListener("click", () => void this.scan());
    this.select.addEventListener("change", () => { this.selected = this.select.value; this.render(); });
    this.use.addEventListener("click", () => void this.openSelected());
    onLangChange(() => this.render());
    this.render();
  }

  invalidate() {
    this.epoch++;
    if (this.phase === "scanning") this.phase = "idle";
    this.render();
  }

  remember(path: string) {
    try { localStorage.setItem(LAST_GAME, path); } catch {}
  }

  choose(path: string) {
    if (this.opening) return;
    this.epoch++;
    this.manual = true;
    this.phase = "done";
    this.truncated = false;
    this.games = [{ path, id: "", region: "", format: /\.wbfs$/i.test(path) ? "WBFS" : /\.rvz$/i.test(path) ? "RVZ" : /\.wia$/i.test(path) ? "WIA" : "ISO", sizeBytes: 0 }];
    this.selected = path;
    this.render();
    this.use.focus();
  }

  async scan() {
    if (this.pending || this.opening) return;
    const epoch = ++this.epoch;
    this.pending = true;
    this.phase = "scanning";
    this.manual = false;
    this.games = [];
    this.selected = "";
    this.truncated = false;
    this.render();
    try {
      const result = await invoke<DiscoveryResult>("scan_for_game", { lastPath: lastPath() });
      if (epoch !== this.epoch) return;
      this.games = result.games;
      this.truncated = result.truncated;
      const previous = lastPath();
      this.selected = this.games.find(g => g.path === previous)?.path ?? this.games[0]?.path ?? "";
      this.phase = "done";
    } catch {
      if (epoch !== this.epoch) return;
      this.phase = "error";
    } finally {
      this.pending = false;
      this.render();
    }
  }

  private async openSelected() {
    if ((this.pending && this.phase === "scanning") || this.opening || !this.games.some(g => g.path === this.selected)) return;
    this.opening = true;
    this.render();
    try { await this.openGame(this.selected); }
    finally { this.opening = false; this.render(); }
  }

  private render() {
    this.status.dataset.phase = this.phase;
    this.status.textContent = this.phase === "scanning" ? t("game.scanning")
      : this.phase === "error" ? t("game.scanFailed")
      : this.phase === "idle" ? t("game.scanIdle")
      : this.manual ? t("game.manualReady") : this.games.length ? t("game.scanFound", { n: this.games.length }) : t("game.scanEmpty");
    this.rescan.disabled = this.pending || this.opening;
    this.use.disabled = (this.pending && this.phase === "scanning") || this.opening || !this.selected;
    this.select.disabled = (this.pending && this.phase === "scanning") || this.opening;
    const manualButton = document.getElementById("btn-open-image") as HTMLButtonElement;
    manualButton.textContent = t(this.selected ? "game.change" : "game.chooseManual");
    manualButton.dataset.i18n = this.selected ? "game.change" : "game.chooseManual";
    manualButton.disabled = this.opening;
    document.getElementById("detected-choice")!.classList.toggle("hidden", this.games.length < 2);
    const name = document.getElementById("game-selected-name")!;
    name.classList.toggle("hidden", this.games.length > 1);
    name.textContent = this.selected.split(/[\\/]/).pop() ?? "";
    this.results.classList.toggle("hidden", this.phase !== "done" || this.games.length === 0);
    this.limit.classList.toggle("hidden", !this.truncated || this.phase !== "done" || this.games.length > 0);
    const region: Record<string, string> = { USA: t("game.regionUsa"), Europe: t("game.regionEurope"), Japan: t("game.regionJapan"), Korea: t("game.regionKorea") };
    this.select.replaceChildren(...this.games.map(g => {
      const name = g.path.split(/[\\/]/).pop() ?? g.path;
      const opt = document.createElement("option");
      opt.value = g.path;
      opt.textContent = `${name} · ${region[g.region] ?? g.region} · ${g.format}`;
      return opt;
    }));
    this.select.value = this.selected;
    const verbatim = "\\\\?\\";
    const unc = verbatim + "UNC\\";
    const displayPath = this.selected.toUpperCase().startsWith(unc)
      ? "\\\\" + this.selected.slice(unc.length)
      : this.selected.startsWith(verbatim) && /^[a-z]:\\/i.test(this.selected.slice(verbatim.length))
        ? this.selected.slice(verbatim.length) : this.selected;
    this.path.textContent = displayPath;
  }
}
