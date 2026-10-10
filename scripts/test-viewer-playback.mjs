// Real Rust host + TCP signaling + SessionManager bridge + WebView/Chrome decoding.
import assert from "node:assert/strict";
import { spawn, execFileSync } from "node:child_process";
import { createServer as createHttpServer } from "node:http";
import { createServer as createTcpServer } from "node:net";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { createInterface } from "node:readline";

if (process.platform !== "darwin") throw new Error("The synthetic VideoToolbox host requires macOS");
const root = resolve(import.meta.dirname, "..");
const chromeMode = process.argv.includes("--chrome") || process.env.VIEWER_TEST_BROWSER === "chrome";
const recoveryMode = process.argv.includes("--keyframe-recovery");
const scratch = await mkdtemp(resolve(tmpdir(), "video-p2p-playback-"));
const children = [];
let logs = "";
let httpServer;
let hostReport = null;
function child(program, args, options = {}) {
  const process = spawn(program, args, { cwd: root, stdio: ["pipe", "pipe", "pipe"], ...options });
  children.push(process);
  process.on("error", (error) => { logs += `${program}: ${error.message}\n`; });
  process.stderr.on("data", (data) => { logs = (logs + data).slice(-6000); });
  return process;
}
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function until(predicate, timeout = 15000) {
  const start = Date.now();
  while (!(await predicate())) {
    if (Date.now() - start > timeout) throw new Error(`Playback integration timed out\n${logs}`);
    await delay(50);
  }
}
class Bridge {
  constructor(process) {
    this.process = process; this.next = 0; this.pending = new Map();
    createInterface({ input: process.stdout }).on("line", (line) => {
      const result = JSON.parse(line);
      const pending = this.pending.get(result.id);
      this.pending.delete(result.id);
      if (result.error) pending?.reject(new Error(result.error)); else pending?.resolve(result.result);
    });
    process.on("exit", () => { for (const pending of this.pending.values()) pending.reject(new Error(`Viewer bridge exited\n${logs}`)); this.pending.clear(); });
  }
  invoke(command, args = {}) {
    return new Promise((resolve, reject) => {
      const id = ++this.next;
      this.pending.set(id, { resolve, reject });
      this.process.stdin.write(`${JSON.stringify({ id, command, args })}\n`);
    });
  }
}

const html = `<!doctype html><video id="video" autoplay muted playsinline></video><script>
window.finishProbe = (result) => { window.probeResult = result; window.webkit?.messageHandlers?.probe?.postMessage(result); };
window.addEventListener("error", (event) => window.finishProbe({ error: String(event.error?.message ?? event.message) }));
window.addEventListener("unhandledrejection", (event) => window.finishProbe({ error: String(event.reason?.message ?? event.reason) }));
</script><script type="module">
import { ViewerPlayer } from "/viewer-player.mjs";
const video = document.getElementById("video");
async function invoke(command, args = {}) {
  const response = await fetch("/invoke", { method: "POST", body: JSON.stringify({ command, args }) });
  const value = await response.json(); if (value.error) throw new Error(value.error); return value.result;
}
let answerShape = null;
const player = new ViewerPlayer({ video, invoke, peerFactory: (config) => {
  const peer = new RTCPeerConnection(config);
  const setLocal = peer.setLocalDescription.bind(peer);
  peer.setLocalDescription = async (description) => {
    answerShape = description.sdp.split("\\r\\n").filter((line) => /^(m=|a=group:|a=rtcp|a=rtpmap|a=fmtp)/.test(line));
    return setLocal(description);
  };
  return peer;
} });
function finish(result) { window.finishProbe(result); }
try {
  player.sync(await invoke("start"));
  const audio = new AudioContext();
  await Promise.race([audio.resume(), new Promise((_, reject) => setTimeout(() => reject(new Error("Web Audio activation timed out: " + audio.state)), 5000))]);
  const analyser = audio.createAnalyser();
  const silent = audio.createGain(); silent.gain.value = 0;
  analyser.connect(silent).connect(audio.destination);
  let source; let amplitude = 0;
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    if (player.context?.stream?.getAudioTracks().length && !source) {
      source = audio.createMediaStreamSource(player.context.stream); source.connect(analyser);
    }
    const pcm = new Float32Array(analyser.fftSize); analyser.getFloatTimeDomainData(pcm);
    amplitude = Math.max(amplitude, ...pcm.map(Math.abs));
    if (player.context?.report?.video_frames > 3 && video.videoWidth === 64 && amplitude > 0.001) break;
    if (player.context?.error && player.context.closed) throw new Error(player.context.error);
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  const canvas = document.createElement("canvas"); canvas.width = 64; canvas.height = 36;
  const drawing = canvas.getContext("2d"); drawing.drawImage(video, 0, 0);
  const pixel = [...drawing.getImageData(32, 18, 1, 1).data];
  const backend = await invoke("snapshot");
  const host = await invoke("host_snapshot");
  const result = { width: video.videoWidth, height: video.videoHeight, amplitude, pixel, backend, host, state: player.context?.state };
  const tracks = player.context.stream.getTracks();
  const peer = player.context.peer;
  await invoke("stop");
  const stoppedSession = await invoke("refresh");
  player.sync(stoppedSession);
  const cleanupDeadline = Date.now() + 1000;
  while ((peer.signalingState !== "closed" || tracks.some((track) => track.readyState !== "ended")) && Date.now() < cleanupDeadline) {
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
  result.cleanup = { stage: stoppedSession.stage, mediaCleared: video.srcObject === null, playerStopped: player.context === null,
    signalingState: peer.signalingState, connectionState: peer.connectionState, tracks: tracks.map((track) => ({ kind: track.kind, state: track.readyState })) };
  result.stopped = result.cleanup.stage === "stopped" && result.cleanup.mediaCleared && result.cleanup.playerStopped
    && result.cleanup.signalingState === "closed" && result.cleanup.tracks.every((track) => track.state === "ended");
  await audio.close();
  finish(result);
} catch (error) { player.stop(); finish({ error: String(error.message ?? error), answerShape, codecs: globalThis.RTCRtpReceiver?.getCapabilities?.("video")?.codecs }); }
</script>`;

try {
  const reserve = createTcpServer();
  await new Promise((resolve, reject) => { reserve.once("error", reject); reserve.listen(0, "127.0.0.1", resolve); });
  const address = `127.0.0.1:${reserve.address().port}`;
  await new Promise((resolve) => reserve.close(resolve));
  const server = child(resolve(root, "target/debug/signaling-server"), [address]);
  let ready = false;
  server.stdout.on("data", () => { ready = true; });
  await until(() => ready || server.exitCode !== null);
  assert.equal(server.exitCode, null, `Signaling server exited\n${logs}`);
  const env = { ...process.env, VIDEO_P2P_MVP_CONFIG_DIR: resolve(scratch, "preferences") };
  const host = child(resolve(root, "target/debug/examples/playback_probe"), ["host", address, ...(recoveryMode ? ["--drop-first-keyframe"] : [])], { env });
  createInterface({ input: host.stdout }).on("line", (line) => { hostReport = JSON.parse(line); });
  const bridge = new Bridge(child(resolve(root, "target/debug/examples/playback_probe"), ["bridge", address], { env }));
  httpServer = createHttpServer(async (request, response) => {
    try {
      if (request.url === "/invoke") {
        let body = ""; for await (const chunk of request) body += chunk;
        const { command, args } = JSON.parse(body);
        response.setHeader("Content-Type", "application/json");
        response.end(JSON.stringify({ result: command === "host_snapshot" ? hostReport : await bridge.invoke(command, args) }));
      } else if (request.url === "/viewer-player.mjs") {
        response.setHeader("Content-Type", "text/javascript");
        response.end(await readFile(resolve(root, "apps/desktop/public/viewer-player.mjs")));
      } else { response.setHeader("Content-Type", "text/html"); response.end(html); }
    } catch (error) { response.end(JSON.stringify({ error: String(error.message ?? error) })); }
  });
  await new Promise((resolve, reject) => { httpServer.once("error", reject); httpServer.listen(0, "127.0.0.1", resolve); });
  const url = `http://127.0.0.1:${httpServer.address().port}/`;
  let result;
  if (!chromeMode) {
    const binary = resolve(scratch, "webkit-probe");
    execFileSync("xcrun", ["clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror", "-framework", "Cocoa", "-framework", "WebKit", resolve(root, "scripts/viewer-webkit-probe.m"), "-o", binary]);
    const webview = child(binary, [url]);
    createInterface({ input: webview.stdout }).on("line", (line) => { result = JSON.parse(line); });
    await until(() => result || webview.exitCode !== null, 25000);
    assert.ok(result, `WKWebView did not return a result\n${logs}`);
  } else {
    const chrome = process.env.VIEWER_TEST_CHROME ?? "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    const browser = child(chrome, ["--headless", "--remote-debugging-port=0", `--user-data-dir=${resolve(scratch, "chrome")}`, "--autoplay-policy=no-user-gesture-required", "--no-first-run", "--no-default-browser-check", "about:blank"]);
    let endpoint;
    browser.stderr.on("data", (data) => { endpoint ??= /DevTools listening on (ws:\/\/[^\s]+)/.exec(String(data))?.[1]; });
    await until(() => endpoint);
    const socket = new WebSocket(endpoint);
    await new Promise((resolve) => socket.addEventListener("open", resolve, { once: true }));
    let counter = 0;
    const pending = new Map();
    socket.addEventListener("message", (event) => {
      const value = JSON.parse(event.data);
      if (value.id) { const task = pending.get(value.id); pending.delete(value.id); if (value.error) task.reject(new Error(value.error.message)); else task.resolve(value.result); }
    });
    function cdp(method, params = {}, sessionId) {
      return new Promise((resolve, reject) => { const id = ++counter; pending.set(id, { resolve, reject }); socket.send(JSON.stringify({ id, method, params, sessionId })); });
    }
    const { targetId } = await cdp("Target.createTarget", { url: "about:blank" });
    const { sessionId } = await cdp("Target.attachToTarget", { targetId, flatten: true });
    await cdp("Page.navigate", { url }, sessionId);
    await until(async () => {
      const value = await cdp("Runtime.evaluate", { expression: "window.probeResult", returnByValue: true }, sessionId);
      result = value.result?.value;
      return result;
    }, 25000);
    socket.close();
  }
  assert.equal(result.error, undefined, JSON.stringify(result));
  assert.equal(result.width, 64);
  assert.equal(result.height, 36);
  assert.ok(result.pixel[0] > 120 && result.pixel[1] > 30 && result.pixel[2] < 100, `wrong decoded color: ${result.pixel}`);
  assert.ok(result.amplitude > 0.001, `decoded audio is silent: ${result.amplitude}`);
  assert.ok(result.backend.video_frames > 3);
  assert.ok(result.backend.audio_samples > 0);
  assert.equal(result.backend.connection, "connected");
  assert.equal(result.stopped, true, JSON.stringify(result.cleanup));
  if (recoveryMode) {
    assert.ok(result.host?.initial_keyframe_dropped, "the probe did not drop its initial keyframe");
    assert.ok(result.host.received_requests > 0, "the browser did not request a keyframe through RTCP");
    assert.ok(result.host.recovery_keyframes > 0, "the host did not publish a feedback-driven keyframe");
  }
  console.log(`${chromeMode ? "Chrome" : "WKWebView"} playback passed: ${result.width}x${result.height}, ${result.backend.video_frames} decoded frames, ${result.backend.audio_samples} audio samples; stop released media.${recoveryMode ? " Browser RTCP keyframe recovery verified." : ""}`);
} finally {
  const exits = children.map((process) => process.exitCode !== null || process.signalCode !== null ? undefined : new Promise((resolve) => process.once("exit", resolve)));
  for (const process of children) if (process.exitCode === null && process.signalCode === null) process.kill();
  if (httpServer) { httpServer.closeAllConnections(); await new Promise((resolve) => httpServer.close(resolve)); }
  await Promise.all(exits);
  await rm(scratch, { recursive: true, force: true });
}
