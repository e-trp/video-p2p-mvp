// DOM actions in the actual Tauri window. IPC, media, and session objects are never mocked.
// The source-picker race check gates delivery of real IPC responses, without changing their data.
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
  async function focusQaWindow() {
    await invoke("gui_qa_focus");
    await until(async () => (await invoke("gui_qa_window_state")).focused, "foreground QA controls");
  }
  const checkpoint = (name, publish = false) => invoke("gui_qa_checkpoint", { name, publish });
  const checkpointWait = (name) => until(() => checkpoint(name), `native checkpoint ${name}`, 30000);
  async function refresh() { await click("flow-refresh-btn"); }
  async function selectPreferenceSource() {
    const options = [...el("source-picker").options];
    assert(options.length > 0, "No source/fallback options");
    const native = row("capture-catalog", "Origin") !== "blueprintfallback";
    const selected = native ? config.native_source_id : options[0].value;
    assert(selected && options.some((option) => option.value === selected),
      "Persistent QA fixture is absent from the source catalog");
    await change("source-picker", selected);
    if (!el("source-audio").disabled) await change("source-audio", false);
    const state = await snapshot();
    assert(state.selected_source_id === selected && !state.selected_source_audio, "Source/audio choice not applied");
    return { source: selected, sourceLabel: state.source_label, native };
  }
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
    await until(async () => {
      const state = await invoke("gui_qa_window_state");
      return state.focused && state.visible;
    }, "foreground Tauri QA window");
    await until(() => el("runtime-badge")?.textContent === "Tauri runtime connected"
      && !el("spec").textContent.includes("loading..."), "real Tauri boot", 15000);
    // load() installs UI listeners after the initial refresh and specification have resolved.
    await sleep(150);
    await check("Runtime and complete initial render", async () => {
      assert(!document.body.textContent.includes("loading..."), "Initial loading placeholders remain");
      assert(el("spec").textContent.length > 100, "Rust specification command did not load");
      assert(row("session", "Mode") === "idle", "Fresh/restored app did not start idle");
      assert(el("viewer-playback").hidden && el("viewer-sound-btn").disabled, "Idle player controls are active");
      return { nativeWindow: await invoke("gui_qa_window_state"),
        permission: row("capture-catalog", "Permission"), origin: row("capture-catalog", "Origin"),
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
      await check("Source selection and audio opt-out persist", selectPreferenceSource);
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
    } else if (config.scenario === "source-picker") {
      await change("refresh-interval", "1");
      await check("Focused source options survive Auto Refresh", async () => {
        const picker = el("source-picker");
        picker.focus();
        const options = [...picker.options];
        assert(options.length > 1, "Two sources are required for selection regression");
        const value = picker.value;
        await sleep(2200);
        assert(options.every((option, index) => picker.options[index] === option), "Auto Refresh replaced focused source options");
        assert(picker.value === value, "Auto Refresh changed focused selection");
        picker.blur();
        return { sources: options.length, selected: value,
          nativeOpenStateSupported: CSS.supports("selector(select:open)") };
      });
      el("source-picker").blur();
      await change("auto-refresh-enabled", false);
      await check("Source change survives an older refresh response", async () => {
        const picker = el("source-picker");
        const original = picker.value;
        await change("source-picker", original);
        const chosen = [...picker.options].find((option) => option.value !== original).value;
        let releaseRefresh;
        let releaseSelection;
        const refreshGate = new Promise((resolve) => { releaseRefresh = resolve; });
        const selectionGate = new Promise((resolve) => { releaseSelection = resolve; });
        let catalogs = 0;
        let refreshReady = false;
        const submitted = [];
        const tauri = window.__TAURI__;
        const gatedInvoke = async (command, args) => {
          if (command === "select_capture_source") submitted.push({ ...args });
          const ordinal = command === "capture_catalog" ? ++catalogs : 0;
          const result = await invoke(command, args);
          if (ordinal === 1) { refreshReady = true; await refreshGate; }
          if (ordinal === 2) await selectionGate;
          return result;
        };
        window.__TAURI__ = { ...tauri, core: { ...tauri.core, invoke: gatedInvoke } };
        try {
          el("flow-refresh-btn").click();
          await until(() => refreshReady, "older catalog response");
          picker.value = chosen;
          picker.dispatchEvent(new Event("change", { bubbles: true }));
          releaseRefresh();
          // Deliver the older refresh before the former handler's metadata request.
          await sleep(150);
          releaseSelection();
          await until(() => submitted.length > 0, "source selection IPC");
          await idle();
          const state = await snapshot();
          assert(submitted[0].source_id === chosen, `Selected ${chosen}, submitted ${submitted[0].source_id}`);
          assert(state.selected_source_id === chosen && picker.value === chosen, "Selected source did not survive refresh");
          return { original, chosen, submitted, persisted: state.selected_source_id };
        } finally {
          releaseRefresh(); releaseSelection();
          window.__TAURI__ = tauri;
        }
      });
      await check("Selected source and audio opt-out survive focused refresh and blur", async () => {
        const picker = el("source-picker");
        const chosen = picker.value;
        assert(!el("source-audio").disabled, "An audio-capable source is required");
        await change("source-audio", false);
        picker.focus();
        await refresh();
        assert(picker.value === chosen && !el("source-audio").checked, "Focused refresh changed source/audio choice");
        picker.blur();
        await refresh();
        const state = await snapshot();
        assert(state.selected_source_id === chosen && !state.selected_source_audio, "Blur discarded persisted source/audio choice");
        return { source: chosen, audio: state.selected_source_audio };
      });
      await check("Unavailable source is rejected without selecting a different source", async () => {
        const picker = el("source-picker");
        const previous = (await snapshot()).selected_source_id;
        // A stale menu entry models a window disappearing before its selection reaches Rust.
        const unavailable = new Option("Unavailable QA window", "qa-source-no-longer-present");
        picker.add(unavailable);
        await change("source-picker", unavailable.value);
        assert(el("command-result").classList.contains("tone-bad"), "Missing source was accepted");
        assert((await snapshot()).selected_source_id === previous, "Missing source silently selected another window");
        unavailable.remove();
        picker.blur();
        await refresh();
        assert(picker.value === previous, "Refresh did not restore the authoritative source");
      });
      await check("Failed catalog/session refresh preserves confirmed source controls", async () => {
        form("catalog-refresh-qa"); await refresh(); await click("host-btn");
        const before = await snapshot();
        const picker = el("source-picker");
        assert(before.selected_source_id && !el("start-capture-btn").disabled, "Prepared host source controls are unavailable");
        const originalTauri = window.__TAURI__;
        const originalInvoke = originalTauri.core.invoke;
        const failures = [];
        const failingInvoke = async (command, args) => {
          const value = await originalInvoke(command, args);
          if (["session_snapshot", "capture_catalog"].includes(command)) {
            failures.push(command);
            throw new Error("Controlled refresh delivery failure");
          }
          return value;
        };
        window.__TAURI__ = { ...originalTauri, core: { ...originalTauri.core, invoke: failingInvoke } };
        try {
          // The preceding action may still have its post-action refresh in flight.
          await until(async () => {
            await refresh();
            return failures.includes("session_snapshot") && failures.includes("capture_catalog");
          }, "controlled refresh failures");
          await sleep(150);
          assert(picker.value === before.selected_source_id && !picker.disabled,
            "Failed refresh discarded the confirmed source options");
          assert(!el("start-capture-btn").disabled && row("session", "Mode") === "host",
            "Failed refresh replaced confirmed host controls with idle/preview state");
          assert(el("runtime-badge").textContent.includes("incomplete"), "Stale diagnostics are not marked");
        } finally {
          window.__TAURI__ = originalTauri;
        }
        await refresh();
        await until(() => el("runtime-badge").textContent === "Tauri runtime connected", "refresh recovery");
        assert((await snapshot()).selected_source_id === before.selected_source_id, "Recovery changed the selected source");
        await click("stop-btn");
        return { failures, selectedSource: before.selected_source_id };
      });
      await check("Window commands remain responsive while a session command waits", async () => {
        await invoke("gui_qa_hold_session");
        await until(() => invoke("gui_qa_session_held"), "controlled session contention");
        const pending = invoke("session_snapshot");
        const windowState = await invoke("gui_qa_window_state");
        const held = await invoke("gui_qa_session_held");
        const session = await pending;
        assert(windowState.visible && held, "Session command blocked the native main thread until the lock was released");
        assert(session.stage === "stopped", "Queued session command did not return its real result");
        for (const [command, args] of [["unknown_qa_command", {}], ["select_capture_source", {}]]) {
          let rejected = false;
          try { await invoke(command, args); } catch { rejected = true; }
          assert(rejected, `Invalid IPC request did not reject: ${command}`);
        }
        return { windowState, held, stage: session.stage };
      });
    } else if (config.scenario === "preferences") {
      await check("Save preferences with a source owned by the runner", async () => {
        await selectPreferenceSource();
        await change("refresh-interval", "15");
        await change("auto-refresh-enabled", false);
        form("tauri-gui-saved", config.signaling_addr, "stun:127.0.0.1:3478");
        await click("save-btn");
        evidence.saved = await snapshot();
        return evidence.saved;
      });
    } else if (config.scenario === "restore" || config.scenario === "removed-source") {
      await check("Preferences survive actual application restart", async () => {
        const state = await snapshot();
        assert(el("room").value === "tauri-gui-saved" && el("signaling").value === config.signaling_addr, "Session settings not restored");
        assert(state.ice_server_count === 1 && !state.ui_auto_refresh_enabled && state.ui_refresh_interval_secs === 15, "ICE/refresh preferences not restored");
        assert(config.expected_source_id, "Missing exact saved source ID from the preceding process");
        if (config.scenario === "restore") {
          assert(state.selected_source_id === config.expected_source_id && !state.selected_source_audio,
            "Exact source/audio preferences not restored");
        }
        assert(state.mode === "idle" && state.viewer_generation == null, "Restart restores an active session");
        return { source: state.selected_source_id, interval: state.ui_refresh_interval_secs };
      });
      if (config.scenario === "removed-source") {
        await check("Closed fixture is removed without silently selecting another source", async () => {
          if (config.expected_source_id !== config.native_source_id) {
            assert(!config.expected_source_id?.startsWith("macos-"),
              "Saved native source is not the runner's controlled fixture");
            return { qaResult: "BLOCKED", reason: "Native source removal requires Screen Recording permission" };
          }
          await until(async () => {
            await refresh();
            return row("capture-catalog", "Origin") === "runtime"
              && ![...el("source-picker").options].some((option) => option.value === config.expected_source_id);
          }, "closed fixture leaves native catalog");
          const state = await snapshot();
          assert(state.selected_source_id === null && state.mode === "idle",
            "Missing source was retained or silently replaced in an idle session");
          assert(state.logs.some((line) => line.includes(`capture source dropped from refreshed catalog: ${config.expected_source_id}`)),
            "Missing-source cleanup was not observed");
          return { removedSource: config.expected_source_id, state };
        });
      }
    } else if (config.scenario === "native-host") {
      await check("Native host ready", async () => {
        assert(config.native_source_id, "Missing fixture source ID");
        await change("auto-refresh-enabled", false);
        await refresh();
        assert([...el("source-picker").options].some((option) => option.value === config.native_source_id),
          "Fixture is absent from native catalog; check Screen Recording permission");
        await change("source-picker", config.native_source_id);
        await change("source-audio", config.native_audio);
        form("native-playback-qa"); await refresh(); await click("host-prototype-btn");
        await until(async () => {
          await refresh();
          return (await snapshot()).capture_runtime_status === "running";
        }, "native capture running");
        const state = await snapshot();
        assert(state.selected_source_id === config.native_source_id && state.selected_source_audio === config.native_audio,
          "Host did not preserve fixture selection");
        return state;
      });
      assert(results.at(-1).result === "PASS", "Native source startup failed");
      await check("Native publication runs without host diagnostic polling", async () => {
        await until(async () => {
          await refresh();
          return (await snapshot()).transport_state === "connected";
        }, "native viewer connected", 30000);
        const before = await snapshot();
        await sleep(2200);
        const after = await snapshot();
        assert(!after.ui_auto_refresh_enabled, "Host polling is enabled");
        assert(after.encoded_video_frame_count > before.encoded_video_frame_count + 5, "Native video publication stalled");
        if (config.native_audio) assert(after.encoded_audio_packet_count > before.encoded_audio_packet_count + 20, "Native audio publication stalled");
        else assert(after.encoded_audio_packet_count === 0, "Audio opt-out published encoded audio");
        return { before, after };
      });
      if (config.native_alternate_source_id) {
        await checkpointWait("initial-viewer");
        for (const [sourceId, label, published, observed] of [
          [config.native_alternate_source_id, "silent alternate", "alternate-host", "alternate-viewer"],
          [config.native_source_id, "original tone", "restored-host", "restored-viewer"],
        ]) {
          await check(`Source change stops capture before starting ${label}`, async () => {
            await focusQaWindow();
            await change("source-picker", sourceId);
            const stopped = await snapshot();
            assert(stopped.selected_source_id === sourceId && stopped.capture_runtime_status !== "running", "Source change retained old capture");
            await sleep(1200); const settled = await snapshot();
            assert(settled.encoded_video_frame_count === stopped.encoded_video_frame_count
              && settled.encoded_audio_packet_count === stopped.encoded_audio_packet_count, "Old source published after selection completed");
            await click("start-capture-btn");
            await until(async () => (await snapshot()).encoded_video_frame_count > settled.encoded_video_frame_count + 5,
              "new source publication");
            return { stopped, running: await snapshot() };
          });
          assert(results.at(-1).result === "PASS", "Source switch failed");
          await checkpoint(published, true);
          await checkpointWait(observed);
        }
        await checkpointWait("source-closed");
        await check("Closing the captured source and refreshing stops publication", async () => {
          await focusQaWindow();
          await until(async () => {
            await refresh();
            return ![...el("source-picker").options].some((option) => option.value === config.native_source_id);
          }, "closed source leaves catalog");
          const before = await snapshot();
          assert(before.selected_source_id !== config.native_source_id && before.capture_runtime_status !== "running", "Closed source retained active capture");
          await sleep(1500); const after = await snapshot();
          assert(after.encoded_video_frame_count === before.encoded_video_frame_count
            && after.encoded_audio_packet_count === before.encoded_audio_packet_count, "Closed source or fallback keeps publishing");
          return { before, after };
        });
        assert(results.at(-1).result === "PASS", "Source closure failed");
        await checkpoint("closure-host", true);
      }
      await until(() => invoke("gui_qa_stop_requested"), "viewer completion", 45000);
      await check("Native host Stop releases capture and publication", async () => {
        await focusQaWindow();
        await stopRemainsStopped();
        const before = await snapshot(); await sleep(1200); const after = await snapshot();
        assert(after.capture_runtime_status !== "running", "Stop retained capture");
        assert(after.encoded_video_frame_count === before.encoded_video_frame_count
          && after.encoded_audio_packet_count === before.encoded_audio_packet_count, "Publication continued after Stop");
        return after;
      });
    } else if (config.scenario === "native-viewer") {
      await change("auto-refresh-enabled", false);
      let mediaReady = false;
      let audio;
      let source;
      let analyser;
      let gain;
      try {
        await check("Native window pixels decode in the desktop viewer", async () => {
          form("native-playback-qa"); await refresh(); await click("viewer-prototype-btn");
          const video = el("viewer-video");
          await until(() => peers.at(-1)?.connectionState === "connected" && video.videoWidth > 100
            && video.getVideoPlaybackQuality().totalVideoFrames > 3, "native decoded video", 20000);
          assert(video.muted, "Native viewer did not start muted");
          const canvas = document.createElement("canvas");
          canvas.width = video.videoWidth; canvas.height = video.videoHeight;
          const context = canvas.getContext("2d"); context.drawImage(video, 0, 0);
          const pixel = [...context.getImageData(Math.floor(canvas.width * 0.8), Math.floor(canvas.height * 0.7), 1, 1).data];
          assert(pixel[0] > 140 && pixel[1] > 30 && pixel[1] < 110 && pixel[2] < 80, `Unexpected native fixture pixel: ${pixel}`);
          mediaReady = true;
          return { width: canvas.width, height: canvas.height, pixel, state: await snapshot() };
        });
        await check(config.native_audio ? "Native audio reaches decoded PCM with viewer polling disabled"
          : "Native audio opt-out retains video without audio", async () => {
          assert(mediaReady, "Native video prerequisite failed");
          const before = await snapshot();
          let peak = 0;
          let rms = 0;
          if (config.native_audio) {
            audio = new AudioContext();
            await Promise.race([audio.resume(), sleep(5000).then(() => { throw new Error("Web Audio activation timed out"); })]);
            source = audio.createMediaStreamSource(el("viewer-video").srcObject);
            analyser = audio.createAnalyser();
            gain = audio.createGain(); gain.gain.value = 0;
            source.connect(analyser); analyser.connect(gain); gain.connect(audio.destination);
            await until(() => {
              const pcm = new Float32Array(analyser.fftSize); analyser.getFloatTimeDomainData(pcm);
              peak = Math.max(...pcm.map(Math.abs));
              rms = Math.sqrt(pcm.reduce((sum, value) => sum + value * value, 0) / pcm.length);
              return rms > 0.001;
            }, "nonzero decoded native PCM");
          }
          await sleep(2200);
          const after = await snapshot();
          assert(!after.ui_auto_refresh_enabled && after.decoded_video_frame_count > before.decoded_video_frame_count + 5, "Native video stalled without polling");
          if (config.native_audio) assert(after.received_audio_sample_count > before.received_audio_sample_count + 48000, "Native audio samples stalled");
          else assert(after.received_audio_sample_count === 0 && after.received_audio_packet_count === 0, "Audio opt-out reached the viewer");
          return { peak, rms, audioContext: audio?.state, before, after, outputGain: 0 };
        });
        if (config.native_alternate_source_id) {
          assert(mediaReady && audio?.state === "running", "Source lifecycle requires decoded native audio/video");
          const originalPeer = peers.at(-1);
          const originalGeneration = (await snapshot()).viewer_generation;
          const readPcm = () => {
            const pcm = new Float32Array(analyser.fftSize); analyser.getFloatTimeDomainData(pcm);
            return Math.sqrt(pcm.reduce((sum, value) => sum + value * value, 0) / pcm.length);
          };
          const readPixel = () => {
            const video = el("viewer-video");
            const canvas = document.createElement("canvas"); canvas.width = video.videoWidth; canvas.height = video.videoHeight;
            const ctx = canvas.getContext("2d"); ctx.drawImage(video, 0, 0);
            return [...ctx.getImageData(Math.floor(canvas.width * 0.8), Math.floor(canvas.height * 0.7), 1, 1).data];
          };
          await checkpoint("initial-viewer", true);
          for (const [silent, published, observed] of [
            [true, "alternate-host", "alternate-viewer"],
            [false, "restored-host", "restored-viewer"],
          ]) {
            await checkpointWait(published);
            await focusQaWindow();
            await check(silent ? "Viewer decodes the silent alternate source" : "Viewer resumes original pixels and PCM without reconnect", async () => {
              const before = await snapshot();
              await until(() => {
                const pixel = readPixel();
                return silent ? pixel[2] > 140 && pixel[0] < 80 : pixel[0] > 140 && pixel[2] < 80;
              }, "replacement source pixels");
              // Drain packets and decoder concealment from the preceding source before measuring.
              await sleep(1500);
              let rms = readPcm();
              if (!silent) await until(() => { rms = readPcm(); return rms > 0.001; }, "restored tone PCM");
              let maxRms = rms;
              const end = Date.now() + 1200;
              while (Date.now() < end) { maxRms = Math.max(maxRms, readPcm()); await sleep(50); }
              const after = await snapshot();
              assert(peers.at(-1) === originalPeer && after.viewer_generation === originalGeneration, "Source change recreated the viewer peer");
              assert(after.decoded_video_frame_count > before.decoded_video_frame_count + 5
                && after.received_audio_sample_count > before.received_audio_sample_count + 48000, "Replacement video/audio stream did not advance");
              if (silent) assert(maxRms <= 0.0001, `Original tone leaked into silent source: ${maxRms}`);
              return { pixel: readPixel(), rms, maxRms, before, after };
            });
            assert(results.at(-1).result === "PASS", "Replacement playback failed");
            await checkpoint(observed, true);
          }
          await checkpointWait("closure-host");
          await focusQaWindow();
          await check("Source closure stops new decoded media without replacing the viewer", async () => {
            await sleep(1500); const before = await snapshot();
            await sleep(1500); const after = await snapshot();
            assert(after.decoded_video_frame_count === before.decoded_video_frame_count, "Viewer continues decoding after source closure");
            assert(peers.at(-1) === originalPeer && after.viewer_generation === originalGeneration, "Source closure replaced viewer peer");
            const rms = readPcm();
            assert(rms <= 0.0001, `Tone persists after source closure: ${rms}`);
            return { before, after, rms };
          });
        }
        await check("Native viewer sound controls preserve mute state", async () => {
          assert(mediaReady, "Native video prerequisite failed");
          await click("viewer-sound-btn");
          assert(!el("viewer-video").muted, "Enable Sound did not unmute native playback");
          await click("viewer-sound-btn");
          assert(el("viewer-video").muted, "Mute Sound did not mute native playback");
          return { gesture: "programmatic DOM clicks; audible output is not certified" };
        });
        await check("Native viewer Stop ends tracks and prevents revival", async () => {
          const peer = peers.at(-1);
          const tracks = el("viewer-video").srcObject?.getTracks() ?? [];
          assert(mediaReady && tracks.length > 0, "No native media tracks to stop");
          await stopRemainsStopped();
          assert(tracks.every((track) => track.readyState === "ended") && peer.signalingState === "closed", "Native viewer retained peer/tracks");
          return { tracks: tracks.map((track) => ({ kind: track.kind, state: track.readyState })) };
        });
      } finally {
        source?.disconnect(); analyser?.disconnect(); gain?.disconnect();
        if (audio) await audio.close();
      }
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
