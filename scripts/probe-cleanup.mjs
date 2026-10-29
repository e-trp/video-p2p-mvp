import { rm } from "node:fs/promises";
import { setTimeout as delay } from "node:timers/promises";

async function waitFor(predicate, timeoutMs) {
  const deadline = performance.now() + timeoutMs;
  while (!predicate()) {
    if (performance.now() >= deadline) return false;
    await delay(25);
  }
  return true;
}

// processGroup is only valid for a child spawned with detached: true on POSIX.
// Signal that private group so browser helpers cannot outlive their launcher.
export async function stopProbeChild(child, {
  processGroup = false, graceMs = 0, terminateMs = 3000, killMs = 3000,
} = {}) {
  if (!child.pid) return; // A failed spawn owns no process.
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  function groupAlive() {
    try { process.kill(-child.pid, 0); return true; }
    catch (error) { if (error.code === "ESRCH") return false; throw error; }
  }
  const stopped = () => exited() && (!processGroup || !groupAlive());
  function signal(name) {
    try {
      if (processGroup) process.kill(-child.pid, name);
      else if (!exited()) child.kill(name);
    } catch (error) { if (error.code !== "ESRCH") throw error; }
  }
  if (await waitFor(stopped, graceMs)) return;
  signal("SIGTERM");
  if (await waitFor(stopped, terminateMs)) return;
  signal("SIGKILL");
  if (!await waitFor(stopped, killMs)) {
    throw new Error(`Probe process${processGroup ? " group" : ""} ${child.pid} did not stop after SIGKILL`);
  }
}

// A directory entry can appear between recursive enumeration and rmdir. Retry
// only transient removal failures, with a finite budget; retain persistent errors.
export async function removeProbeScratch(path, {
  remove = rm, maxRetries = 5, retryDelayMs = 100,
} = {}) {
  for (let attempt = 0; ; attempt += 1) {
    try { await remove(path, { recursive: true, force: true }); return; }
    catch (error) {
      if (attempt >= maxRetries || !["EBUSY", "ENOTEMPTY", "EPERM", "EMFILE", "ENFILE"].includes(error.code)) {
        throw new Error(`Could not remove probe scratch directory ${path}: ${error.message}`, { cause: error });
      }
      await delay(retryDelayMs * (attempt + 1));
    }
  }
}
