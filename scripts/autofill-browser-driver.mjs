// Test tooling only: Chromium DevTools over inherited pipes, never TCP/HTTP.
import { spawn } from "node:child_process";
import { mkdtemp, realpath, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

export async function launchBrowser(browser, { headless = true } = {}) {
  if (!["Chrome", "Edge"].includes(browser)) throw new Error("Select Chrome or Edge for native acceptance.");
  const executable = browser === "Chrome"
    ? path.join(process.env.ProgramFiles, "Google/Chrome/Application/chrome.exe")
    : path.join(process.env["ProgramFiles(x86)"], "Microsoft/Edge/Application/msedge.exe");
  const parent = await realpath(tmpdir());
  const profile = await mkdtemp(path.join(parent, "keynest-autofill-acceptance-"));
  const child = spawn(executable, [
    ...(headless ? ["--headless"] : []), "--disable-gpu", "--disable-background-networking", "--no-first-run",
    "--no-default-browser-check", "--remote-debugging-pipe", "--enable-unsafe-extension-debugging",
    `--user-data-dir=${profile}`, "about:blank",
  ], { windowsHide: true, stdio: ["ignore", "ignore", "pipe", "pipe", "pipe"] });
  const diagnostic = { executable: path.basename(executable), processId: child.pid, stderrBytes: 0,
    stderrErrorMarkers: 0, stderrFatalMarkers: 0, exitCode: null, signal: null,
    lastCommand: null, lastCompletedCommand: null };
  // Never retain or print raw stderr: it can contain page/extension data.
  child.stderr.on("data", chunk => {
    diagnostic.stderrBytes += chunk.length;
    const markers = chunk.toString("utf8");
    diagnostic.stderrErrorMarkers += (markers.match(/:ERROR:/g) ?? []).length;
    diagnostic.stderrFatalMarkers += (markers.match(/:FATAL:/g) ?? []).length;
  });
  let sequence = 0;
  let incoming = "";
  const pending = new Map();
  const listeners = new Set();
  child.stdio[4].setEncoding("utf8");
  child.stdio[4].on("data", chunk => {
    incoming += chunk;
    let end;
    while ((end = incoming.indexOf("\0")) >= 0) {
      const text = incoming.slice(0, end); incoming = incoming.slice(end + 1);
      if (!text) continue;
      const message = JSON.parse(text);
      if (message.id && pending.has(message.id)) {
        const task = pending.get(message.id); pending.delete(message.id); clearTimeout(task.timer);
        diagnostic.lastCompletedCommand = task.method;
        if (message.error) task.reject(new Error(`${task.method}: ${message.error.message}`));
        else task.resolve(message.result);
      } else for (const listener of listeners) listener(message);
    }
  });
  function failPending() {
    for (const task of pending.values()) { clearTimeout(task.timer); task.reject(new Error("Disposable browser connection ended.")); }
    pending.clear();
  }
  child.on("error", failPending);
  child.on("exit", (code, signal) => {
    diagnostic.exitCode = code; diagnostic.signal = signal;
    if (code) process.stderr.write(`Disposable browser exited with code ${code}; signal ${signal ?? "none"}.\n`);
    failPending();
  });
  function send(method, params = {}, sessionId) {
    if (child.exitCode !== null || child.signalCode !== null) return Promise.reject(new Error("Disposable browser is already closed."));
    const id = ++sequence;
    diagnostic.lastCommand = method;
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Browser command timed out: ${method}`)); }, 15000);
      pending.set(id, { resolve, reject, timer, method });
      child.stdio[3].write(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }) + "\0");
    });
  }
  async function close() {
    try { await send("Browser.close"); } catch { child.kill(); }
    if (child.exitCode === null) await Promise.race([
      new Promise(resolve => child.once("exit", resolve)),
      new Promise(resolve => { const timer = setTimeout(() => { child.kill(); resolve(); }, 3000); timer.unref(); }),
    ]);
    failPending();
    // Resolve and verify the unique absolute target before recursive removal.
    const resolved = await realpath(profile);
    if (path.dirname(resolved).toLowerCase() !== parent.toLowerCase()
      || !path.basename(resolved).startsWith("keynest-autofill-acceptance-")) throw new Error("Unsafe browser fixture cleanup target.");
    await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
  return { send, close, listeners, profile, processId: child.pid, diagnostics: () => ({ ...diagnostic }) };
}
