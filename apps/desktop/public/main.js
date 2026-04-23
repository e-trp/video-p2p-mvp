async function invoke(command) {
  if (!window.__TAURI__?.core?.invoke) {
    return null;
  }
  return window.__TAURI__.core.invoke(command);
}

const isTauri = Boolean(window.__TAURI__?.core?.invoke);

async function invokeWithArgs(command, args = {}) {
  if (!isTauri) {
    return null;
  }
  return window.__TAURI__.core.invoke(command, args);
}

function setStatus(status) {
  const container = document.getElementById("status");
  container.innerHTML = `
    <div><dt>Stage</dt><dd>${status.stage}</dd></div>
    <div><dt>GUI</dt><dd>${status.gui}</dd></div>
    <div><dt>Transport</dt><dd>${status.transport}</dd></div>
    <div><dt>macOS Capture</dt><dd>${status.capture_macos}</dd></div>
    <div><dt>Linux Capture</dt><dd>${status.capture_linux}</dd></div>
  `;
}

function setSession(session) {
  const container = document.getElementById("session");
  container.innerHTML = `
    <div><dt>Mode</dt><dd>${session.mode}</dd></div>
    <div><dt>Room</dt><dd>${session.room ?? "n/a"}</dd></div>
    <div><dt>Signaling</dt><dd>${session.signaling_addr ?? "n/a"}</dd></div>
    <div><dt>Signal Link</dt><dd>${String(session.signaling_connected)}</dd></div>
    <div><dt>Source</dt><dd>${session.source_label ?? "n/a"}</dd></div>
    <div><dt>Peer</dt><dd>${session.active_peer ?? "n/a"}</dd></div>
    <div><dt>Transport State</dt><dd>${session.transport_state ?? "n/a"}</dd></div>
    <div><dt>Media Tracks</dt><dd>${session.local_media_track_count ?? 0}</dd></div>
    <div><dt>Video Track</dt><dd>${String(session.local_video_track_attached)}</dd></div>
    <div><dt>Audio Track</dt><dd>${String(session.local_audio_track_attached)}</dd></div>
    <div><dt>Video Samples</dt><dd>${session.published_video_sample_count ?? 0} / ${session.last_video_sample_bytes ?? 0}B</dd></div>
    <div><dt>Audio Samples</dt><dd>${session.published_audio_sample_count ?? 0} / ${session.last_audio_sample_bytes ?? 0}B</dd></div>
    <div><dt>Local Desc</dt><dd>${session.local_description_kind ?? "n/a"} / ${String(session.local_description_ready)}</dd></div>
    <div><dt>Remote Desc</dt><dd>${session.remote_description_kind ?? "n/a"} / ${String(session.remote_description_ready)}</dd></div>
    <div><dt>Local ICE</dt><dd>${session.local_candidate_count ?? 0}</dd></div>
    <div><dt>Remote ICE</dt><dd>${session.remote_candidate_count ?? 0}</dd></div>
    <div><dt>Next Action</dt><dd>${session.next_action ?? "n/a"}</dd></div>
  `;
  document.getElementById("session-log").textContent = session.logs.join("\n");
  document.getElementById("signal-preview").textContent =
    session.last_signaling_message ?? "No signaling messages yet.";
  document.getElementById("room").value = session.room ?? document.getElementById("room").value;
  document.getElementById("signaling").value =
    session.signaling_addr ?? document.getElementById("signaling").value;
  document.getElementById("source").value =
    session.source_label ?? document.getElementById("source").value;
}

function formValues() {
  return {
    room: document.getElementById("room").value.trim(),
    signaling_addr: document.getElementById("signaling").value.trim(),
    source_label: document.getElementById("source").value.trim() || null,
  };
}

async function refresh() {
  const [status, session] = await Promise.all([
    invoke("project_status"),
    invoke("session_snapshot"),
  ]);

  if (status) {
    setStatus(status);
    document.getElementById("runtime-badge").textContent = "Tauri runtime connected";
    document.getElementById("runtime-badge").className = "badge live";
  } else {
    document.getElementById("runtime-badge").textContent = "Browser preview";
    document.getElementById("runtime-badge").className = "badge preview";
    document.getElementById("status").innerHTML = `
      <div><dt>Mode</dt><dd>Browser preview without Tauri runtime</dd></div>
    `;
  }

  if (session) {
    setSession(session);
  } else {
    document.getElementById("session").innerHTML = `
      <div><dt>Mode</dt><dd>preview</dd></div>
      <div><dt>Room</dt><dd>n/a</dd></div>
      <div><dt>Signaling</dt><dd>n/a</dd></div>
      <div><dt>Signal Link</dt><dd>false</dd></div>
      <div><dt>Source</dt><dd>n/a</dd></div>
      <div><dt>Peer</dt><dd>n/a</dd></div>
      <div><dt>Transport State</dt><dd>preview</dd></div>
      <div><dt>Media Tracks</dt><dd>0</dd></div>
      <div><dt>Video Track</dt><dd>false</dd></div>
      <div><dt>Audio Track</dt><dd>false</dd></div>
      <div><dt>Video Samples</dt><dd>0 / 0B</dd></div>
      <div><dt>Audio Samples</dt><dd>0 / 0B</dd></div>
      <div><dt>Local Desc</dt><dd>n/a / false</dd></div>
      <div><dt>Remote Desc</dt><dd>n/a / false</dd></div>
      <div><dt>Local ICE</dt><dd>0</dd></div>
      <div><dt>Remote ICE</dt><dd>0</dd></div>
      <div><dt>Next Action</dt><dd>run inside Tauri</dd></div>
    `;
    document.getElementById("session-log").textContent =
      "Run inside Tauri to drive the in-memory session manager.";
    document.getElementById("signal-preview").textContent =
      "Run inside Tauri to preview signaling state.";
  }
}

async function runCommand(command, args = {}) {
  const result = await invokeWithArgs(command, args);
  if (!result) {
    document.getElementById("command-result").textContent =
      `Preview mode: skipped ${command}`;
    return;
  }

  document.getElementById("command-result").textContent = result.message;
  setSession(result.session);
  const status = await invoke("project_status");
  if (status) {
    setStatus(status);
  }
}

async function load() {
  const specification = await invoke("specification_markdown");
  await refresh();

  document.getElementById("spec").textContent =
    specification ?? "Run inside Tauri to load the saved specification from the Rust backend.";

  document.getElementById("save-btn").addEventListener("click", async () => {
    const values = formValues();
    await runCommand("update_session_config", values);
  });

  document.getElementById("host-btn").addEventListener("click", async () => {
    const values = formValues();
    await runCommand("start_host", values);
  });

  document.getElementById("join-btn").addEventListener("click", async () => {
    const { room, signaling_addr } = formValues();
    await runCommand("join_room", { room, signaling_addr });
  });

  document.getElementById("mock-btn").addEventListener("click", async () => {
    await runCommand("mark_mock_streaming", { peer: "pending-direct-peer" });
  });

  document.getElementById("webrtc-btn").addEventListener("click", async () => {
    await runCommand("mark_webrtc_planned");
  });

  document.getElementById("offer-btn").addEventListener("click", async () => {
    await runCommand("create_local_offer");
  });

  document.getElementById("publish-media-btn").addEventListener("click", async () => {
    await runCommand("publish_placeholder_media");
  });

  document.getElementById("answer-btn").addEventListener("click", async () => {
    const sdp = document.getElementById("remote-answer").value;
    await runCommand("accept_remote_answer", { sdp });
  });

  document.getElementById("ice-btn").addEventListener("click", async () => {
    const candidate = document.getElementById("ice-candidate").value;
    const sdp_mid = document.getElementById("ice-mid").value.trim() || null;
    const mlineRaw = document.getElementById("ice-mline").value.trim();
    const sdp_mline_index = mlineRaw === "" ? null : Number.parseInt(mlineRaw, 10);
    await runCommand("add_remote_ice_candidate", {
      candidate,
      sdp_mid,
      sdp_mline_index: Number.isNaN(sdp_mline_index) ? null : sdp_mline_index,
    });
  });

  document.getElementById("stop-btn").addEventListener("click", async () => {
    await runCommand("stop_session");
  });

  document.getElementById("reset-btn").addEventListener("click", async () => {
    await runCommand("reset_session");
  });

  document.getElementById("clear-logs-btn").addEventListener("click", async () => {
    await runCommand("clear_session_logs");
  });

  document.getElementById("refresh-btn").addEventListener("click", refresh);

  if (isTauri) {
    window.setInterval(refresh, 3000);
  }
}

load().catch((error) => {
  document.getElementById("spec").textContent = String(error);
});
