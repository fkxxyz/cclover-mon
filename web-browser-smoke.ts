import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

const browser = process.env.CCLOVER_BROWSER ?? Bun.which("chromium") ?? Bun.which("google-chrome") ?? Bun.which("google-chrome-stable");

if (!browser) {
  console.error("web-browser smoke test requires Chromium/Chrome; set CCLOVER_BROWSER to the executable path");
  process.exit(1);
}

const server = Bun.spawn([
  "./target/release/cclover-mon",
  "--http",
  "--http-bind",
  "127.0.0.1:0",
], {
  stdout: "ignore",
  stderr: "pipe",
});

async function waitForAddress(): Promise<string> {
  const decoder = new TextDecoder();
  let text = "";
  for await (const chunk of server.stderr) {
    text += decoder.decode(chunk, { stream: true });
    const match = text.match(/HTTP monitor listening on (http:\/\/[^\s]+)/);
    if (match) return match[1];
    if (text.length > 16_384) text = text.slice(-8_192);
  }
  throw new Error(`HTTP monitor exited before reporting its listen address (exit ${await server.exited})`);
}

function allocatePort(): number {
  const listener = Bun.listen({
    hostname: "127.0.0.1",
    port: 0,
    socket: {
      data() {},
    },
  });
  const port = listener.port;
  listener.stop(true);
  return port;
}

async function waitForPageTarget(port: number, url: string): Promise<string> {
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      if (response.ok) {
        const targets = await response.json() as Array<{ type?: string; url?: string; webSocketDebuggerUrl?: string }>;
        const expectedOrigin = new URL(url).origin;
        const target = targets.find((entry) => entry.type === "page" && entry.url && new URL(entry.url).origin === expectedOrigin && entry.webSocketDebuggerUrl);
        if (target?.webSocketDebuggerUrl) return target.webSocketDebuggerUrl;
      }
    } catch {
      // DevTools endpoint may not be ready yet.
    }
    await Bun.sleep(100);
  }
  throw new Error("Chromium DevTools endpoint did not expose the cclover-mon page");
}

async function waitForRenderedDashboard(webSocketUrl: string): Promise<void> {
  const socket = new WebSocket(webSocketUrl);
  await new Promise<void>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("timed out connecting to Chromium DevTools")), 5_000);
    socket.addEventListener("open", () => {
      clearTimeout(timer);
      resolve();
    }, { once: true });
    socket.addEventListener("error", () => {
      clearTimeout(timer);
      reject(new Error("failed to connect to Chromium DevTools"));
    }, { once: true });
  });

  let id = 0;
  const pending = new Map<number, (value: unknown) => void>();
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(String(event.data)) as { id?: number; result?: unknown };
    if (message.id === undefined) return;
    const resolve = pending.get(message.id);
    if (!resolve) return;
    pending.delete(message.id);
    resolve(message.result);
  });

  const evaluate = (expression: string) => new Promise<any>((resolve) => {
    const requestId = ++id;
    pending.set(requestId, resolve);
    socket.send(JSON.stringify({
      id: requestId,
      method: "Runtime.evaluate",
      params: { expression, returnByValue: true },
    }));
  });

  try {
    const deadline = Date.now() + 10_000;
    while (Date.now() < deadline) {
      const result = await evaluate(`(() => {
        const panel = document.querySelector('.cclover-panel');
        const texts = panel ? [...panel.querySelectorAll('text[data-cclover-must-fit]')] : [];
        const tolerance = 1 / (window.devicePixelRatio || 1);
        return {
          panel: !!panel,
          svg: !!document.querySelector('svg'),
          mustFitCount: texts.length,
          typographyFits: texts.length > 0 && texts.every(text => {
            const maxWidth = Number(text.dataset.ccloverMaxWidth);
            return Number.isFinite(maxWidth) && text.getComputedTextLength() <= maxWidth + tolerance;
          }),
        };
      })()`);
      const value = result?.result?.value as {
        panel?: boolean;
        svg?: boolean;
        mustFitCount?: number;
        typographyFits?: boolean;
      } | undefined;
      if (value?.panel && value?.svg && value.mustFitCount && value.typographyFits) return;
      await Bun.sleep(100);
    }
    throw new Error("browser DOM never reached typography-conformant rendered dashboard state");
  } finally {
    socket.close();
  }
}

let browserProcess: ReturnType<typeof Bun.spawn> | undefined;
const browserProfile = mkdtempSync(join(tmpdir(), "cclover-web-browser-"));
try {
  const url = await waitForAddress();
  const debugPort = allocatePort();
  browserProcess = Bun.spawn([
    browser,
    "--headless=new",
    "--disable-gpu",
    "--disable-dev-shm-usage",
    "--no-sandbox",
    `--remote-debugging-port=${debugPort}`,
    `--user-data-dir=${browserProfile}`,
    "--no-first-run",
    "--no-default-browser-check",
    url,
  ], {
    stdout: "ignore",
    stderr: "ignore",
  });

  const target = await waitForPageTarget(debugPort, url);
  await waitForRenderedDashboard(target);
  console.log(`web browser smoke: ok (${browser})`);
} finally {
  if (browserProcess) {
    browserProcess.kill(9);
    await browserProcess.exited;
  }
  server.kill(9);
  await server.exited;
  rmSync(browserProfile, { recursive: true, force: true });
}
