import { mkdirSync, readFileSync, writeFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { tmpdir } from "node:os";
import { spawn } from "node:child_process";
import { createServer } from "vite";

export const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
export const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

export function browserPath() {
  if (process.env.MKSS_BROWSER) return process.env.MKSS_BROWSER;
  const candidates = {
    win32: ["C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe", "C:/Program Files/Microsoft/Edge/Application/msedge.exe", "C:/Program Files/Google/Chrome/Application/chrome.exe"],
    darwin: ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge", "/Applications/Chromium.app/Contents/MacOS/Chromium"],
    linux: ["/usr/bin/google-chrome", "/usr/bin/google-chrome-stable", "/usr/bin/chromium", "/usr/bin/chromium-browser", "/usr/bin/microsoft-edge"],
  }[process.platform] ?? [];
  const found = candidates.find((p) => existsSync(p));
  if (!found) throw new Error("No Chromium-based browser found: set MKSS_BROWSER to Edge/Chrome/Chromium");
  return found;
}

export async function startHarness({ fixture, scratchName = "mkss-ui", middleware }) {
  const scratch = resolve(process.env.MKSS_UI_SCRATCH || tmpdir(), scratchName);
  mkdirSync(scratch, { recursive: true });
  const sourceHtml = readFileSync(resolve(root, "index.html"), "utf8");
  const server = await createServer({
    root,
    configFile: false,
    logLevel: "error",
    server: { host: "127.0.0.1", port: 0, watch: { ignored: ["**/src-tauri/**", "**/design/**"] } },
    plugins: [
      {
        name: "ui-fixture",
        configureServer(s) {
          s.middlewares.use((req, res, next) => {
            if (middleware && middleware(req, res)) return;
            if (req.url.startsWith("/ui-test")) {
              res.setHeader("Content-Type", "text/html");
              res.end(sourceHtml.replace("<head>", "<head><script>" + fixture + "</script>"));
            } else next();
          });
        },
      },
    ],
  });
  await server.listen();
  const origin = server.resolvedUrls.local[0];
  const profile = resolve(scratch, "profile-" + Date.now());
  const browser = spawn(
    browserPath(),
    ["--headless=new", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--ignore-gpu-blocklist", "--disable-background-networking", "--no-first-run", "--remote-debugging-port=0", `--user-data-dir=${profile}`, ...(process.platform === "linux" ? ["--no-sandbox", "--disable-dev-shm-usage"] : []), "about:blank"],
    { stdio: ["ignore", "ignore", "pipe"] },
  );
  let browserLog = "";
  let browserExit = null;
  browser.stderr.on("data", (d) => (browserLog = (browserLog + d).slice(-4000)));
  browser.on("exit", (code) => (browserExit = code));
  let ws, serial = 0;
  const pending = new Map();
  const runtimeErrors = [];
  const send = (method, params = {}) =>
    new Promise((res, rej) => {
      const id = ++serial;
      const timer = setTimeout(() => rej(Error("CDP timeout " + method)), 90000);
      pending.set(id, { resolve: (v) => (clearTimeout(timer), res(v)), reject: (e) => (clearTimeout(timer), rej(e)) });
      ws.send(JSON.stringify({ id, method, params }));
    });
  let port;
  for (let i = 0; i < 600 && !port && browserExit === null; i++) {
    await sleep(100);
    try {
      const line = readFileSync(resolve(profile, "DevToolsActivePort"), "utf8").split("\n")[0];
      if (/^\d+$/.test(line)) port = line;
    } catch {}
  }
  if (!port) throw Error(`Browser did not start (exit ${browserExit}): ${browserPath()}\n${browserLog}`);
  let page;
  for (let i = 0; i < 100 && !page; i++) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      page = targets.find((t) => t.type === "page");
    } catch {}
    if (!page) await sleep(100);
  }
  if (!page) throw Error("Browser has no page target");
  ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((r) => ws.addEventListener("open", r));
  ws.addEventListener("message", (e) => {
    const m = JSON.parse(e.data);
    if (m.id && pending.has(m.id)) {
      const p = pending.get(m.id);
      pending.delete(m.id);
      m.error ? p.reject(Error(JSON.stringify(m.error))) : p.resolve(m.result);
    }
    if (m.method === "Runtime.exceptionThrown") runtimeErrors.push(m.params.exceptionDetails?.exception?.description ?? m.params.exceptionDetails?.text ?? "error");
  });
  await send("Page.enable");
  await send("Runtime.enable");
  const value = async (expression) => {
    const r = await send("Runtime.evaluate", { expression, returnByValue: true, awaitPromise: true });
    if (r.exceptionDetails) throw Error(r.exceptionDetails.exception?.description ?? JSON.stringify(r.exceptionDetails));
    return r.result.value;
  };
  const wait = async (expression, tries = 200) => {
    for (let i = 0; i < tries; i++) {
      if (await value(expression)) return;
      await sleep(60);
    }
    throw Error("Timeout: " + expression);
  };
  const click = async (sel) => {
    await value(`document.querySelector(${JSON.stringify(sel)}).click()`);
    await sleep(120);
  };
  const size = async (w, h) => {
    await send("Emulation.setDeviceMetricsOverride", { width: w, height: h, deviceScaleFactor: 1, mobile: false });
    await sleep(250);
  };
  const mouse = async (type, x, y, button = "left", modifiers = 0) => {
    const buttons = type === "mouseReleased" || button === "none" ? 0 : button === "left" ? 1 : button === "right" ? 2 : 4;
    await send("Input.dispatchMouseEvent", { type, x, y, button: type === "mouseMoved" && !buttons ? "none" : button, buttons: type === "mouseMoved" ? buttons : undefined, clickCount: 1, modifiers });
  };
  const drag = async (pts, button = "left", modifiers = 0) => {
    await mouse("mouseMoved", pts[0][0], pts[0][1], "none");
    await mouse("mousePressed", pts[0][0], pts[0][1], button, modifiers);
    for (const p of pts.slice(1)) await mouse("mouseMoved", p[0], p[1], button, modifiers);
    const l = pts[pts.length - 1];
    await mouse("mouseReleased", l[0], l[1], button, modifiers);
    await sleep(80);
  };
  const shot = async (file) => {
    const { data } = await send("Page.captureScreenshot", { format: "png" });
    writeFileSync(file, Buffer.from(data, "base64"));
    return file;
  };
  const close = async () => {
    try {
      ws?.close();
    } catch {}
    browser.kill();
    await server.close();
  };
  return { send, value, wait, click, size, mouse, drag, shot, close, origin, runtimeErrors, scratch };
}

export const layoutProbe = `(()=>{const out=[];const d=document.documentElement;
 if(d.scrollHeight>innerHeight+1||d.scrollWidth>innerWidth+1)out.push({page:[d.scrollWidth,d.scrollHeight,innerWidth,innerHeight]});
 const SCROLL='.layer-list,.parts-list,.exp-list,.lang-menu,.tex-pop,.char-grid,.proj-list';
 for(const el of document.querySelectorAll('body *')){if(el.closest('.hidden,.sr-only,svg'))continue;const r=el.getBoundingClientRect();if(!r.width||!r.height)continue;
  if(el.closest(SCROLL)&&!el.matches(SCROLL))continue;
  if(r.bottom>innerHeight+0.5||r.right>innerWidth+0.5)out.push({el:el.tagName+'.'+el.className+'#'+el.id,bottom:r.bottom,right:r.right});
  const s=getComputedStyle(el);if((el.scrollHeight>el.clientHeight+1&&s.overflowY!=='visible')||(el.scrollWidth>el.clientWidth+1&&s.overflowX!=='visible'&&!['INPUT','SELECT'].includes(el.tagName))){if(!el.matches(SCROLL+',.ts-text b,.ts-text small,.p3d-label,.p3d-help,.ed-status,.lname,.parts-list li span,.opt span,.crumbs,.crumbs *,.title-input,.done-line .path'))out.push({clip:el.tagName+'.'+el.className+'#'+el.id,sh:el.scrollHeight,ch:el.clientHeight,sw:el.scrollWidth,cw:el.clientWidth,below:[...el.querySelectorAll('*')].filter(c=>{const q=c.getBoundingClientRect();return q.height>0&&q.bottom>el.getBoundingClientRect().bottom+1}).slice(0,4).map(c=>c.tagName+'.'+c.className+'#'+c.id+':'+Math.round(c.getBoundingClientRect().height))});}
 }return out;})()`;
