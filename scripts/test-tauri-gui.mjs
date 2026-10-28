// Launch the real, opt-in Tauri GUI QA build with isolated preferences and local peers.
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer, createConnection } from "node:net";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { once } from "node:events";
import { promisify } from "node:util";

if (process.platform !== "darwin") throw new Error("This GUI QA run currently requires macOS");
const root = resolve(import.meta.dirname, "..");
const binaryFlag = process.argv.indexOf("--binary");
const outputFlag = process.argv.indexOf("--output");
const scenarioFlag = process.argv.indexOf("--scenario");
const selectedScenario = scenarioFlag < 0 ? null : process.argv[scenarioFlag + 1];
assert(selectedScenario === null || ["controls", "preferences", "viewer", "viewer-visibility", "source-picker"].includes(selectedScenario), "Unknown --scenario; use preferences for the complete save/restart/removal check");
const binary = binaryFlag < 0 ? join(root, "apps/desktop/src-tauri/target/debug/desktop-app")
  : resolve(process.argv[binaryFlag + 1]);
const output = outputFlag < 0 ? await mkdtemp(join(tmpdir(), "video-p2p-tauri-qa-"))
  : resolve(process.argv[outputFlag + 1]);
await mkdir(output, { recursive: true });
const children = new Set();
let interrupted = false;
let logs = "";
function interrupt() {
  interrupted = true;
  for (const child of children) child.kill("SIGTERM");
}
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
function launch(program, args, options = {}) {
  if (interrupted) throw new Error("GUI QA interrupted");
  const child = spawn(program, args, { cwd: root, stdio: ["ignore", "pipe", "pipe"], ...options });
  children.add(child);
  child.on("error", (error) => { logs += `${program}: ${error}\n`; });
  child.stdout.on("data", (data) => { logs += `${program}: ${data}`; });
  child.stderr.on("data", (data) => { logs += `${program}: ${data}`; });
  child.on("exit", () => children.delete(child));
  return child;
}
async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  child.kill("SIGTERM");
  const force = setTimeout(() => child.kill("SIGKILL"), 2000);
  try { await once(child, "exit"); } finally { clearTimeout(force); }
}
const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function freePort() {
  const server = createServer();
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}
async function ready(port, process) {
  const deadline = Date.now() + 5000;
  while (Date.now() < deadline) {
    assert(process.exitCode === null, "Signaling exited before listening");
    const connected = await new Promise((resolve) => {
      const socket = createConnection({ host: "127.0.0.1", port });
      socket.once("error", () => resolve(false));
      socket.once("connect", () => { socket.destroy(); resolve(true); });
    });
    if (connected) return;
    await delay(50);
  }
  throw new Error("Signaling startup timed out");
}
async function scenario(name, configDir, signaling, fixtureSource, expectedSource) {
  await mkdir(configDir, { recursive: true });
  const report = join(output, `${name}.json`);
  const app = launch(binary, [], { env: { ...process.env,
    VIDEO_P2P_MVP_CONFIG_DIR: configDir,
    VIDEO_P2P_GUI_QA_REPORT: report,
    VIDEO_P2P_GUI_QA_SCENARIO: name,
    VIDEO_P2P_GUI_QA_SIGNALING: signaling,
    VIDEO_P2P_GUI_QA_NATIVE_SOURCE: fixtureSource ?? "",
    VIDEO_P2P_GUI_QA_EXPECTED_SOURCE: expectedSource ?? "",
  } });
  let timedOut = false;
  const timeout = setTimeout(() => { timedOut = true; app.kill("SIGKILL"); }, 90000);
  const [code, signal] = await once(app, "exit").finally(() => clearTimeout(timeout));
  let result;
  try { result = JSON.parse(await readFile(report, "utf8")); }
  catch (error) { throw new Error(`${name} produced no readable GUI evidence (exit=${code}, signal=${signal}, timeout=${timedOut}): ${error}\n${logs.slice(-5000)}`); }
  for (const test of result.results ?? []) {
    console.log(`${test.result}: ${name} / ${test.name}`);
    if (test.result !== "PASS") console.log(test.error ?? test.details?.reason);
  }
  if (result.fatal) console.log(result.fatal);
  if (timedOut) console.log(`FAIL: ${name} timed out; partial evidence preserved`);
  return { name, success: !timedOut && code === 0 && result.complete && result.success,
    savedSource: result.saved?.selected_source_id,
    checks: result.results?.length ?? 0, failed: result.results?.filter((test) => test.result === "FAIL").map((test) => test.name),
    blocked: result.results?.filter((test) => test.result === "BLOCKED").map((test) => test.name) };
}
async function preferenceScenarios(initial, configDir, signaling) {
  const scratch = await mkdtemp(join(tmpdir(), "video-p2p-preference-source-"));
  let fixture;
  try {
    const executable = join(scratch, "fixture");
    await promisify(execFile)("xcrun", ["clang", "-fobjc-arc", "-fblocks", "-Wall", "-Wextra", "-Werror",
      join(root, "scripts/native-capture-fixture.m"), "-framework", "Cocoa", "-o", executable]);
    // This silent window belongs to the runner and survives both desktop processes.
    // No audio capture/isolation claim depends on its launch context.
    fixture = launch(executable, ["--silent"]);
    let identity = "";
    fixture.stdout.on("data", (data) => { identity += data; });
    const deadline = Date.now() + 10000;
    while (!/window_id=(\d+) process_id=(\d+)/.test(identity)) {
      assert(fixture.exitCode === null && fixture.signalCode === null, "Preference source exited before becoming ready");
      assert(Date.now() < deadline, "Preference source did not report its window ID");
      await delay(50);
    }
    const [, windowId, pid] = identity.match(/window_id=(\d+) process_id=(\d+)/);
    const source = `macos-window-${windowId}`;
    await writeFile(join(output, "preference-source.json"), JSON.stringify({ source, pid: Number(pid) }, null, 2));
    const saved = await scenario(initial, configDir, signaling, source);
    assert(fixture.exitCode === null && fixture.signalCode === null, "Preference source disappeared during save");
    const restored = await scenario("restore", configDir, signaling, source, saved.savedSource);
    assert(fixture.exitCode === null && fixture.signalCode === null, "Preference source disappeared during restart");
    await writeFile(join(output, "saved-session.conf"), await readFile(join(configDir, "session.conf")));
    await stop(fixture);
    const removed = await scenario("removed-source", configDir, signaling, source, saved.savedSource);
    return [saved, restored, removed];
  } finally {
    if (fixture) await stop(fixture);
    await rm(scratch, { recursive: true, force: true });
  }
}
let success = false;
try {
  const port = await freePort();
  const signaling = `127.0.0.1:${port}`;
  const server = launch(join(root, "target/debug/signaling-server"), [signaling]);
  await ready(port, server);
  const controlConfig = join(output, "controls-config");
  const results = [];
  if (selectedScenario === null || selectedScenario === "source-picker")
    results.push(await scenario("source-picker", join(output, "source-picker-config"), signaling));
  if (selectedScenario === null || selectedScenario === "controls" || selectedScenario === "preferences")
    results.push(...await preferenceScenarios(selectedScenario === "preferences" ? "preferences" : "controls", controlConfig, signaling));
  if (selectedScenario === null || selectedScenario === "viewer" || selectedScenario === "viewer-visibility") {
    const host = launch(join(root, "target/debug/examples/playback_probe"), ["host", signaling]);
    results.push(await scenario(selectedScenario === "viewer-visibility" ? selectedScenario : "viewer", join(output, "viewer-config"), signaling));
    await stop(host);
  }
  success = results.every((result) => result.success);
  await writeFile(join(output, "summary.json"), JSON.stringify({ binary, signaling, success, results }, null, 2));
  console.log(`GUI QA ${success ? "PASS" : "FAIL"}; evidence: ${output}`);
} finally {
  for (const child of [...children]) await stop(child);
  await writeFile(join(output, "process.log"), logs);
  console.log(`Processes stopped; logs: ${join(output, "process.log")}`);
}
if (!success) process.exitCode = 1;
