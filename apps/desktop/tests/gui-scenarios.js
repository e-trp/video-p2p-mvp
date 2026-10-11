// DOM actions in the actual Tauri window. IPC, media, and session objects are never mocked.
(async () => {
  if (window.__videoP2pGuiQaStarted) return;
  window.__videoP2pGuiQaStarted = true;
  const invoke = window.__TAURI__.core.invoke;
  const config = await invoke("gui_qa_configuration");
  const results = [];
  const errors = [];
  const evidence = { scenario: config.scenario, userAgent: navigator.userAgent, results, errors };
  const peers = [];
  // Observe real browser peers for failure evidence without changing their configuration or methods.
  window.RTCPeerConnection = new Proxy(window.RTCPeerConnection, {
    construct(target, args) {
      const peer = Reflect.construct(target, args);
      peers.push(peer);
      return peer;
    },
  });
  const el = (id) => document.getElementById(id);
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const row = (id, label) => [...el(id).querySelectorAll("div")]
    .find((item) => item.querySelector("dt")?.textContent === label)?.querySelector("dd")?.textContent;
  const snapshot = () => invoke("session_snapshot");
  const persist = () => invoke("gui_qa_report", { payload: JSON.stringify(evidence, null, 2) });
  window.addEventListener("error", (event) => errors.push(String(event.message)));
  window.addEventListener("unhandledrejection", (event) => errors.push(String(event.reason)));
  async function until(predicate, label, timeout = 10000) {
    const deadline = Date.now() + timeout;
    while (!(await predicate())) {
      if (Date.now() >= deadline) throw new Error(`Timed out: ${label}`);
      await sleep(50);
    }
  }
  async function idle() {
    await sleep(120);
    await until(() => !document.body.classList.contains("is-busy"), "UI action completion");
    await sleep(120);
  }
  async function click(id) {
    assert(!el(id).disabled, `${id} unexpectedly disabled`);
    el(id).click();
    await idle();
  }
  function input(id, value) {
    el(id).value = value;
    el(id).dispatchEvent(new Event("input", { bubbles: true }));
  }
  async function change(id, value) {
    assert(!el(id).disabled, `${id} unexpectedly disabled`);
    if (el(id).type === "checkbox") el(id).checked = value;
    else el(id).value = value;
    el(id).dispatchEvent(new Event("change", { bubbles: true }));
    await idle();
  }
  function form(room, signaling = config.signaling_addr, ice = "") {
    input("room", room); input("signaling", signaling); input("ice-servers", ice);
  }
  async function check(name, action) {
    evidence.running = name;
    await persist();
    const started = Date.now();
    try {
      const details = await action();
      results.push({ name, result: details?.qaResult ?? "PASS", durationMs: Date.now() - started, details });
    } catch (error) {
      const video = el("viewer-video");
      const peer = peers.at(-1);
      const stats = peer ? [...(await peer.getStats()).values()]
        .filter((stat) => ["transport", "candidate-pair", "local-candidate", "remote-candidate"].includes(stat.type)) : [];
      results.push({ name, result: "FAIL", durationMs: Date.now() - started,
        error: `${error.message ?? error}\n${error.stack ?? ""}`, commandResult: el("command-result").textContent,
        overview: el("overview-role").textContent, snapshot: await snapshot(),
        player: { width: video.videoWidth, height: video.videoHeight, paused: video.paused,
          muted: video.muted, currentTime: video.currentTime, readyState: video.readyState,
          status: el("viewer-playback-status").textContent, secure: isSecureContext,
          visibility: document.visibilityState,
          tracks: video.srcObject?.getTracks().map((track) => ({ kind: track.kind, state: track.readyState })) },
        peer: peer ? { connection: peer.connectionState, ice: peer.iceConnectionState,
          gathering: peer.iceGatheringState, signaling: peer.signalingState, stats } : null });
    }
    await persist();
  }
  async function refresh() { await click("flow-refresh-btn"); }
  async function stopRemainsStopped() {
    await click("stop-btn");
    for (let n = 0; n < 3; n++) await refresh();
    const state = await snapshot();
    assert(state.stage === "stopped", `Stop revived session: ${state.stage}`);
    assert(el("viewer-video").srcObject === null, "Stop retained viewer media");
    if (state.viewer_generation != null) {
      assert(!(await invoke("viewer_poll_signals", { generation: state.viewer_generation })).active, "Stopped generation remains active");
    }
    assert(row("recovery-status", "State") === "stopped", "Stopped recovery not rendered");
    return state.recovery_state;
  }
  try {
    await until(() => el("runtime-badge")?.textContent === "Tauri runtime connected"
      && !el("spec").textContent.includes("loading..."), "real Tauri boot", 15000);
    // load() installs UI listeners after the initial refresh and specification have resolved.
    await sleep(150);
    await check("Runtime and complete initial render", async () => {
      assert(!document.body.textContent.includes("loading..."), "Initial loading placeholders remain");
      assert(el("spec").textContent.length > 100, "Rust specification command did not load");
      assert(row("session", "Mode") === "idle", "Fresh/restored app did not start idle");
      assert(el("viewer-playback").hidden && el("viewer-sound-btn").disabled, "Idle player controls are active");
      return { permission: row("capture-catalog", "Permission"), origin: row("capture-catalog", "Origin"),
        sources: el("source-picker").options.length, width: innerWidth, height: innerHeight };
    });

    if (config.scenario === "controls") {
      await check("Idle capture and reconnect guards", async () => {
        for (const id of ["start-capture-btn", "stop-capture-btn", "publish-media-btn", "host-debug-btn", "reconnect-btn"])
          assert(el(id).disabled, `Idle enabled ${id}`);
        form("", ""); await refresh();
        assert(el("host-prototype-btn").disabled && el("viewer-prototype-btn").disabled, "Empty configuration enables prototype start");
      });
      await check("Local demo and shared busy state", async () => {
        el("local-demo-btn").click();
        assert(document.body.classList.contains("is-busy"), "Action does not enter busy state");
        assert([...document.querySelectorAll("button")].every((button) => button.disabled), "Concurrent actions remain enabled");
        await idle();
        const state = await snapshot();
        assert(state.mode === "host" && state.stage === "mock_streaming", "Demo state incorrect");
        assert(state.room === "local-demo" && state.active_peer === "local viewer demo", "Demo labels incorrect");
        assert(state.capture_runtime_status === "not_started" && !state.signaling_connected, "Demo starts real capture/signaling");
        assert(el("overview-role").textContent === "host / mock_streaming", "Demo overview incorrect");
      });
      await check("Stop remains stopped after repeated refresh", stopRemainsStopped);
      await check("Reset returns idle and disables reconnect", async () => {
        await click("reset-btn");
        assert((await snapshot()).mode === "idle" && el("reconnect-btn").disabled, "Reset retained role");
      });
      await check("Save and render escaped configuration", async () => {
        form('qa-<img src=x onerror="window.qaInjected=1">', config.signaling_addr, "stun:127.0.0.1:3478");
        await click("save-btn");
        assert(row("session", "Room") === el("room").value, "Room text not rendered literally");
        assert(!el("session").querySelector("img") && !window.qaInjected, "Room markup executed");
        assert((await snapshot()).ice_server_count === 1, "ICE configuration not saved");
      });
      await check("Invalid ICE is rejected without overwriting saved config", async () => {
        const before = await snapshot();
        input("ice-servers", "https://invalid.example"); await click("save-btn");
        assert(el("command-result").classList.contains("tone-bad"), "Invalid ICE shows success");
        assert(el("command-result").textContent.includes("invalid ICE"), "Validation lacks useful message");
        assert((await snapshot()).ice_servers === before.ice_servers, "Invalid ICE replaced saved config");
      });
      await check("Auto Refresh preserves all unsaved session fields", async () => {
        await change("refresh-interval", "1");
        form("unsaved-room", "127.0.0.1:65534", "stun:unsaved.example:3478");
        el("room").blur();
        await sleep(2200);
        assert(el("room").value === "unsaved-room" && el("signaling").value === "127.0.0.1:65534"
          && el("ice-servers").value === "stun:unsaved.example:3478", "Background refresh destroys form draft");
      });
      await check("Auto Refresh toggle and interval boundaries persist", async () => {
        for (const interval of ["1", "3", "5", "10", "15"]) {
          await change("refresh-interval", interval);
          assert((await snapshot()).ui_refresh_interval_secs === Number(interval), `Interval ${interval} not applied`);
        }
        await change("auto-refresh-enabled", false);
        assert(el("refresh-interval").disabled && !(await snapshot()).ui_auto_refresh_enabled, "Refresh disable not applied");
      });
      await check("Prototype buttons respond to edits without Auto Refresh", async () => {
        input("room", ""); input("signaling", ""); await refresh();
        assert(el("host-prototype-btn").disabled && el("viewer-prototype-btn").disabled, "Empty form allows prototype start");
        form("tauri-gui-qa");
        assert(!el("host-prototype-btn").disabled && !el("viewer-prototype-btn").disabled, "Valid edits leave prototype buttons disabled");
        input("room", "");
        assert(el("host-prototype-btn").disabled && el("viewer-prototype-btn").disabled, "Cleared room leaves start enabled");
        form("tauri-gui-qa");
      });
      await check("Source selection and audio opt-out persist", async () => {
        const options = [...el("source-picker").options];
        assert(options.length > 0, "No source/fallback options");
        const selected = options[Math.min(1, options.length - 1)].value;
        await change("source-picker", selected);
        if (!el("source-audio").disabled) await change("source-audio", false);
        const state = await snapshot();
        assert(state.selected_source_id === selected && !state.selected_source_audio, "Source/audio choice not applied");
        return { source: selected, sourceLabel: state.source_label };
      });
      await check("Prepare Host signals without starting native capture", async () => {
        form("tauri-gui-qa"); await refresh(); await click("host-btn");
        const state = await snapshot();
        assert(state.mode === "host" && state.signaling_connected && state.local_offer_ready, "Host negotiation did not start");
        assert(state.capture_runtime_status === "not_started", "Prepare Host starts capture");
        assert(!el("start-capture-btn").disabled, "Prepared host cannot start capture");
      });
      await check("Debug action remains separate from encoded capture", async () => {
        await click("host-debug-btn");
        const state = await snapshot();
        assert(state.encoded_video_frame_count === 0 && state.encoded_audio_packet_count === 0, "Debug payload counted as encoded media");
        assert(state.capture_runtime_status === "not_started", "Debug action starts capture");
      });
      await check("Start Native Capture and Host Prototype expose blocked permission", async () => {
        if (row("capture-catalog", "Origin") !== "blueprintfallback") {
          return { qaResult: "BLOCKED", reason: "This permission-denied regression requires the fallback catalog" };
        }
        await click("start-capture-btn");
        await click("poll-capture-btn");
        const blocked = await snapshot();
        assert(["permission_required", "permission_denied", "failed"].includes(blocked.capture_runtime_status), "Fallback reports active capture");
        assert(["permission_required", "permission_denied", "failed"].includes(row("capture-runtime", "Status")), "Capture permission status not rendered");
        assert(!el("command-result").textContent.includes("undefined"), "Fallback capture error lost its reason");
        await click("host-prototype-btn");
        const state = await snapshot();
        assert(state.mode === "host" && state.signaling_connected, "Host Prototype did not prepare signaling");
        assert(state.capture_runtime_status !== "running", "Fallback reports live capture");
        assert(el("command-result").classList.contains("tone-warn") || el("command-result").classList.contains("tone-bad"), "Host Prototype hides blocked capture");
        await click("stop-capture-btn");
        return { captureStatus: blocked.capture_runtime_status, permission: blocked.capture_permission_state };
      });
      await check("Host Stop and edited reconnect rebuild role", async () => {
        await stopRemainsStopped();
        assert(!el("reconnect-btn").disabled, "Stopped host cannot reconnect");
        form("tauri-gui-reconnect"); await click("reconnect-btn");
        const state = await snapshot();
        assert(state.mode === "host" && state.room === "tauri-gui-reconnect" && state.signaling_connected, "Edited reconnect did not rebuild host");
        assert(state.capture_runtime_status === "not_started", "Reconnect starts host capture automatically");
        await click("stop-btn");
      });
      await check("Unavailable signaling exposes recovery guidance", async () => {
        form("tauri-gui-offline", "127.0.0.1:0"); await click("join-btn");
        const state = await snapshot();
        assert(!state.signaling_connected && state.reconnect_recommended, "Offline signaling lacks reconnect guidance");
        assert(row("recovery-status", "State") === "signaling_unavailable", "Offline recovery not rendered");
        await click("stop-btn");
      });
      await check("Save stable preferences and Reset reloads them", async () => {
        form("tauri-gui-saved", config.signaling_addr, "stun:127.0.0.1:3478"); await click("save-btn");
        input("room", "discarded-draft"); await click("reset-btn");
        assert(el("room").value === "tauri-gui-saved" && el("ice-servers").value === "stun:127.0.0.1:3478", "Reset did not restore saved settings");
        evidence.saved = await snapshot();
      });
      await check("Diagnostics expand and Clear Logs updates UI", async () => {
        const details = document.querySelector("details.diagnostics");
        details.querySelector("summary").click();
        assert(details.open, "Diagnostics cannot expand");
        await click("clear-logs-btn");
        assert((await snapshot()).logs.length <= 1, "Logs not cleared");
        assert(el("spec").textContent.length > 100, "Diagnostics specification disappeared");
      });
      for (const width of [1120, 800, 600]) {
        await check(`Window layout at ${width}px`, async () => {
          await invoke("gui_qa_resize", { width, height: 760 });
          await until(() => innerWidth === width, "native window resize");
          const overflow = [...document.querySelectorAll("main *")].filter((item) => {
            const box = item.getBoundingClientRect(); return box.width > 0 && box.right > innerWidth + 1;
          }).slice(0, 10).map((item) => ({ tag: item.tagName, id: item.id, class: item.className, text: item.textContent.slice(0, 90) }));
          assert(document.documentElement.scrollWidth <= innerWidth + 1, `Horizontal overflow: ${document.documentElement.scrollWidth}/${innerWidth}; ${JSON.stringify(overflow)}`);
          const required = ["room", "signaling", "source-picker", "save-btn", "stop-btn", "reset-btn"];
          for (const id of required) {
            const box = el(id).getBoundingClientRect();
            assert(box.width > 20 && box.left >= 0 && box.right <= innerWidth + 1, `${id} clipped horizontally`);
          }
          return { width: innerWidth, scrollWidth: document.documentElement.scrollWidth };
        });
      }
    } else if (config.scenario === "restore") {
      await check("Preferences survive actual application restart", async () => {
        const state = await snapshot();
        assert(el("room").value === "tauri-gui-saved" && el("signaling").value === config.signaling_addr, "Session settings not restored");
        assert(state.ice_server_count === 1 && !state.ui_auto_refresh_enabled && state.ui_refresh_interval_secs === 15, "ICE/refresh preferences not restored");
        assert(!state.selected_source_audio && state.selected_source_id != null, "Source/audio preferences not restored");
        assert(state.mode === "idle" && state.viewer_generation == null, "Restart restores an active session");
        return { source: state.selected_source_id, interval: state.ui_refresh_interval_secs };
      });
    } else if (config.scenario === "viewer") {
      await change("auto-refresh-enabled", false);
      let mediaReady = false;
      await check("Join Viewer with Auto Refresh disabled negotiates real media", async () => {
        form("playback-probe"); await refresh(); await click("viewer-prototype-btn");
        await until(() => peers.at(-1)?.connectionState === "connected"
          && el("viewer-video").videoWidth === 64
          && el("viewer-video").getVideoPlaybackQuality().totalVideoFrames > 3,
        "Tauri decoded video", 20000);
        assert(!el("viewer-playback").hidden && el("viewer-video").muted, "Viewer absent or not muted at startup");
        assert(el("viewer-video").videoWidth === 64 && el("viewer-video").videoHeight === 36, "Incorrect decoded dimensions");
        const canvas = document.createElement("canvas"); canvas.width = 64; canvas.height = 36;
        const context = canvas.getContext("2d"); context.drawImage(el("viewer-video"), 0, 0);
        const pixel = [...context.getImageData(32, 18, 1, 1).data];
        assert(pixel[0] > 140 && pixel[1] > 30 && pixel[1] < 100 && pixel[2] < 70, `Wrong decoded pixel: ${pixel}`);
        mediaReady = true;
        return { pixel, state: await snapshot() };
      });
      await check("Decoded video/audio continues without diagnostic polling", async () => {
        if (!mediaReady) return { qaResult: "BLOCKED", reason: "BUG-008: no established decoded Tauri playback" };
        const before = await snapshot();
        await sleep(1800);
        const after = await snapshot();
        assert(after.decoded_video_frame_count > before.decoded_video_frame_count + 5, "Video does not advance");
        assert(after.received_audio_sample_count > before.received_audio_sample_count + 48000, "Audio does not advance");
        assert(!after.ui_auto_refresh_enabled, "Viewer enabled refresh to stream");
        return { beforeFrames: before.decoded_video_frame_count, afterFrames: after.decoded_video_frame_count,
          beforeSamples: before.received_audio_sample_count, afterSamples: after.received_audio_sample_count };
      });
      await check("Sound control state and pause/resume", async () => {
        if (!mediaReady) return { qaResult: "BLOCKED", reason: "BUG-008: no established decoded Tauri playback; real user gestures also require Accessibility" };
        await click("viewer-sound-btn");
        assert(!el("viewer-video").muted && el("viewer-sound-btn").textContent === "Mute Sound", "Enable Sound did not update state");
        await click("viewer-sound-btn");
        assert(el("viewer-video").muted && el("viewer-sound-btn").textContent === "Enable Sound", "Mute Sound did not update state");
        el("viewer-video").pause(); await sleep(120);
        assert(el("viewer-video").paused, "Pause did not apply");
        await el("viewer-video").play();
        assert(!el("viewer-video").paused, "Resume did not apply");
      });
      await check("Stop closes actual viewer tracks and prevents revival", async () => {
        const peer = peers.at(-1);
        const tracks = el("viewer-video").srcObject?.getTracks()
          ?? peer?.getReceivers().map((receiver) => receiver.track).filter(Boolean) ?? [];
        await stopRemainsStopped();
        assert(tracks.every((track) => track.readyState === "ended"), "Viewer tracks remain live after Stop");
        assert(peer?.signalingState === "closed", "Stop retained an open browser peer");
        assert(el("viewer-playback-status").textContent.includes("Join a room"), "Stop retains playback status");
      });
      await check("Reconnect creates a fresh viewer generation", async () => {
        const previousGeneration = (await snapshot()).viewer_generation;
        // Use a new room: reconnecting to an established host also requires rebuilding that host.
        form("tauri-gui-viewer-reconnect");
        await click("reconnect-btn");
        const state = await snapshot();
        assert(state.mode === "viewer" && state.viewer_generation != null
          && state.viewer_generation !== previousGeneration && state.room === "tauri-gui-viewer-reconnect", "Reconnect did not create a fresh viewer");
        await click("reset-btn");
        assert(el("viewer-video").srcObject === null && (await snapshot()).mode === "idle", "Reset retains player");
      });
    } else {
      throw new Error(`Unknown GUI QA scenario: ${config.scenario}`);
    }
    await check("No unhandled frontend errors", () => assert(errors.length === 0, errors.join("\n")));
    evidence.complete = true;
    evidence.success = results.every((result) => result.result === "PASS");
  } catch (error) {
    evidence.fatal = String(error.stack ?? error);
    evidence.success = false;
  }
  delete evidence.running;
  await persist();
  await invoke("stop_session");
  await invoke("gui_qa_finish", { success: evidence.success });
})();
