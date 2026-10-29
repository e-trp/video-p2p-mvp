import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { access, mkdir, mkdtemp, readFile, readdir, rm, rmdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import { removeProbeScratch, stopProbeChild } from "./probe-cleanup.mjs";

async function scratch(t) {
  const path = await mkdtemp(join(tmpdir(), "video-p2p-cleanup-test-"));
  t.after(() => rm(path, { recursive: true, force: true }));
  return path;
}

test("late profile write reproduces ENOTEMPTY and bounded cleanup removes it", async (t) => {
  const path = await scratch(t);
  const profile = join(path, "chrome", "Default");
  await mkdir(profile, { recursive: true });
  let lateWrite = true;
  // Control the actual enumeration/rmdir gap, without depending on OS timing.
  const remove = async (target, options) => {
    if (lateWrite) {
      lateWrite = false;
      assert.deepEqual(await readdir(profile), []);
      await writeFile(join(profile, "Network Persistent State"), "late write");
      await rmdir(profile); // Real filesystem ENOTEMPTY, as in the original log.
    }
    await rm(target, options);
  };
  await assert.rejects(remove(path, { recursive: true, force: true }), { code: "ENOTEMPTY" });
  assert.equal(await readFile(join(profile, "Network Persistent State"), "utf8"), "late write");
  await rm(join(profile, "Network Persistent State"));
  lateWrite = true;
  await removeProbeScratch(path, { remove, retryDelayMs: 1 });
  await assert.rejects(access(path), { code: "ENOENT" });
});

test("persistent profile errors exhaust the budget and preserve diagnostics", async (t) => {
  const path = await scratch(t);
  await writeFile(join(path, "still-writing"), "evidence");
  let attempts = 0;
  await assert.rejects(removeProbeScratch(path, {
    maxRetries: 2, retryDelayMs: 1,
    remove: async () => { attempts += 1; await rmdir(path); },
  }), (error) => error.cause?.code === "ENOTEMPTY" && error.message.includes(path));
  assert.equal(attempts, 3);
  assert.equal(await readFile(join(path, "still-writing"), "utf8"), "evidence");
});

test("non-transient errors fail immediately; missing scratch is already clean", async (t) => {
  const path = await scratch(t);
  let attempts = 0;
  const failure = Object.assign(new Error("read-only filesystem"), { code: "EROFS" });
  await assert.rejects(removeProbeScratch(path, {
    remove: async () => { attempts += 1; throw failure; },
  }), (error) => error.cause === failure);
  assert.equal(attempts, 1);
  await removeProbeScratch(path);
  await removeProbeScratch(path);
});

function fixture(t, code, args = []) {
  const child = spawn(process.execPath, ["-e", code, ...args], {
    detached: true, stdio: ["ignore", "ignore", "pipe", "ipc"],
  });
  t.after(() => stopProbeChild(child, { processGroup: true, terminateMs: 100 }));
  return child;
}

const posix = { skip: process.platform === "win32", timeout: 10000 };

test("shutdown waits for a helper's final profile write and group exit", posix, async (t) => {
  const path = await scratch(t);
  const marker = join(path, "Network Persistent State");
  const helper = `
    process.on('SIGTERM', () => setTimeout(() => {
      require('node:fs').writeFileSync(process.argv[1], 'flushed');
      process.exit(0);
    }, 150));
    setInterval(() => {}, 1000);
    process.send('ready');
  `;
  const child = fixture(t, `
    const helper = require('node:child_process').spawn(process.execPath,
      ['-e', ${JSON.stringify(helper)}, process.argv[1]], { stdio: ['ignore', 'ignore', 'ignore', 'ipc'] });
    process.on('SIGTERM', () => {});
    helper.on('message', () => process.send(helper.pid));
    helper.on('exit', () => process.exit(0));
  `, [marker]);
  const [helperPid] = await once(child, "message");
  await stopProbeChild(child, { processGroup: true });
  assert.equal(await readFile(marker, "utf8"), "flushed");
  assert.throws(() => process.kill(helperPid, 0), { code: "ESRCH" });
  assert.throws(() => process.kill(-child.pid, 0), { code: "ESRCH" });
  await removeProbeScratch(path);
  await assert.rejects(access(path), { code: "ENOENT" });
});

test("a child ignoring SIGTERM is killed within the shutdown bound", posix, async (t) => {
  const child = fixture(t, "process.on('SIGTERM', () => {}); setInterval(() => {}, 1000); process.send('ready');");
  await once(child, "message");
  await stopProbeChild(child, { processGroup: true, terminateMs: 100 });
  assert.equal(child.signalCode, "SIGKILL");
  await stopProbeChild(child, { processGroup: true });
});

test("graceful exit needs no signal and failed spawn owns nothing", posix, async (t) => {
  const child = fixture(t, "process.send('ready'); setTimeout(() => process.exit(0), 100);");
  await once(child, "message");
  await stopProbeChild(child, { processGroup: true, graceMs: 1000 });
  assert.equal(child.exitCode, 0);
  const failed = spawn(`/missing-probe-${process.pid}`);
  await once(failed, "error");
  await stopProbeChild(failed, { processGroup: true });
});
