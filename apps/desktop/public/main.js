const isTauri = Boolean(window.__TAURI__?.core?.invoke);
const sessionFieldIds = ["room", "signaling", "ice-servers"];
const defaultRefreshIntervalSeconds = 3;
const defaultCommandTimeoutMs = 2_500;
const userActionTimeoutMs = 6_000;
const allowedRefreshIntervalSeconds = new Set([1, 3, 5, 10, 15]);
const dirtySessionFields = new Set();
let refreshTimerId = null;
let refreshInFlight = null;
let actionInFlight = false;
let lastSession = null;

const htmlEscapeMap = {
  "&": "&amp;",
  "<": "&lt;",
  ">": "&gt;",
  '"': "&quot;",
  "'": "&#039;",
};

function withTimeout(promise, timeoutMs, label) {
  let timeoutId = null;
  const timeout = new Promise((_, reject) => {
    timeoutId = window.setTimeout(
      () => reject(new Error(`${label} timed out after ${timeoutMs}ms`)),
      timeoutMs,
    );
  });

  return Promise.race([promise, timeout]).finally(() => {
    if (timeoutId !== null) {
      window.clearTimeout(timeoutId);
    }
  });
}

async function invokeWithTimeout(command, args = {}, timeoutMs = defaultCommandTimeoutMs) {
  if (!isTauri) {
    return null;
  }
  return withTimeout(window.__TAURI__.core.invoke(command, args), timeoutMs, command);
}

function escapeHtml(value) {
  return String(value ?? "").replace(/[&<>"']/g, (character) => htmlEscapeMap[character]);
}

function detailRows(rows) {
  return rows
    .map(([label, value]) => `<div><dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd></div>`)
    .join("");
}

function statusTone(value) {
  const normalized = String(value ?? "").toLowerCase();
  if (
    normalized.includes("denied")
    || normalized.includes("failed")
    || normalized.includes("offline")
    || normalized.includes("unavailable")
    || normalized.includes("disconnected")
  ) {
    return "bad";
  }
  if (
    normalized.includes("required")
    || normalized.includes("recommended")
    || normalized.includes("negotiating")
    || normalized.includes("starting")
  ) {
    return "warn";
  }
  if (
    normalized.includes("connected")
    || normalized.includes("running")
    || normalized.includes("healthy")
    || normalized.includes("live")
    || normalized.includes("granted")
  ) {
    return "good";
  }
  return "neutral";
}

function setOverviewToken(id, value, tone = statusTone(value)) {
  const element = document.getElementById(id);
  element.textContent = value;
  element.className = `overview-token tone-${tone}`;
}

function setReadinessToken(id, value, tone = statusTone(value)) {
  const element = document.getElementById(id);
  if (!element) {
    return;
  }
  element.textContent = value;
  element.className = `readiness-token tone-${tone}`;
}

function setCommandResult(message, tone = "neutral") {
  const element = document.getElementById("command-result");
  element.textContent = message;
  element.className = `result tone-${tone}`;
}

function setText(id, value) {
  const element = document.getElementById(id);
  if (element) {
    element.textContent = value;
  }
}

function setActionBusy(isBusy, message = null) {
  actionInFlight = isBusy;
  document.body.classList.toggle("is-busy", isBusy);
  document.querySelectorAll("button").forEach((button) => {
    button.disabled = isBusy;
  });
  if (message) {
    setCommandResult(message, "neutral");
  }
  if (!isBusy && lastSession) {
    syncPrototypeControls(lastSession);
    syncCaptureRuntimeControls(lastSession);
    syncReconnectControl(lastSession);
  }
}

function markSessionFieldDirty(event) {
  dirtySessionFields.add(event.target.id);
}

function clearSessionFieldDrafts() {
  dirtySessionFields.clear();
}

function hasDirtySessionFields() {
  return dirtySessionFields.size > 0;
}

function syncSessionFieldValue(id, value, force = false) {
  const field = document.getElementById(id);
  if (!field) {
    return;
  }

  if (!force && (dirtySessionFields.has(id) || document.activeElement === field)) {
    return;
  }

  field.value = value ?? "";
}

function setStatus(status) {
  const container = document.getElementById("status");
  if (!container) {
    return;
  }
  container.innerHTML = detailRows([
    ["Stage", status.stage],
    ["GUI", status.gui],
    ["Transport", status.transport],
    ["macOS Capture", status.capture_macos],
    ["Linux Capture", status.capture_linux],
  ]);
}

function setOverview(session) {
  setOverviewToken(
    "overview-role",
    `${session.mode ?? "unknown"} / ${session.stage ?? "unknown"}`,
    session.mode === "idle" ? "neutral" : "good",
  );
  setOverviewToken("overview-signaling", session.signaling_connected ? "connected" : "offline");
  setOverviewToken(
    "overview-capture",
    `${session.capture_runtime_status ?? "not_started"} / ${session.capture_permission_state ?? "unknown"}`,
  );
  setOverviewToken(
    "overview-transport",
    `${session.transport_stage ?? "n/a"} / ${session.transport_ice_path_kind ?? "unknown"}`,
  );
  setOverviewToken("overview-next", session.next_action ?? "n/a", "neutral");
}

function setPrototypeReadiness(session) {
  const room = session.room ?? document.getElementById("room")?.value.trim();
  const signaling = session.signaling_connected ? "connected" : "offline";
  const hasSource = Boolean(session.selected_source_id || session.source_label);
  const captureStatus = session.capture_runtime_status ?? "not_started";
  const peerReady = Boolean(
    session.active_peer
      || session.remote_description_ready
      || session.local_data_channel_ready
      || session.transport_state === "connected",
  );

  setReadinessToken("ready-room", room ? room : "missing", room ? "good" : "warn");
  setReadinessToken("ready-source", hasSource ? "selected" : "choose source", hasSource ? "good" : "warn");
  setReadinessToken("ready-capture", captureStatus, statusTone(captureStatus));
  setReadinessToken("ready-signaling", signaling, session.signaling_connected ? "good" : "bad");
  setReadinessToken("ready-peer", peerReady ? "active" : "waiting", peerReady ? "good" : "neutral");
}

function setPrototypeFlow(session) {
  setText("host-flow-source", session.source_label ?? session.selected_source_id ?? "none selected");
  setText(
    "host-flow-capture",
    `${session.capture_runtime_status ?? "not_started"} / ${session.capture_permission_state ?? "unknown"}`,
  );
  setText("host-flow-peer", session.active_peer ?? "waiting");
  setText("viewer-flow-room", session.room ?? document.getElementById("room")?.value ?? "demo");
  setText(
    "viewer-flow-signaling",
    session.signaling_connected ? "connected" : (session.signaling_addr ?? "offline"),
  );
  setText(
    "viewer-flow-transport",
    `${session.transport_stage ?? "n/a"} / ${session.transport_ice_path_kind ?? "unknown"}`,
  );
  setPrototypeReadiness(session);
  syncPrototypeControls(session);
}

function setPreviewOverview() {
  setOverviewToken("overview-role", "preview", "neutral");
  setOverviewToken("overview-signaling", "offline", "bad");
  setOverviewToken("overview-capture", "not_started / unknown", "neutral");
  setOverviewToken("overview-transport", "preview / unknown", "neutral");
  setOverviewToken("overview-next", "run inside Tauri", "neutral");
  setPrototypeFlow({
    mode: "preview",
    room: "demo",
    signaling_connected: false,
    signaling_addr: "offline",
    transport_stage: "preview",
    transport_ice_path_kind: "unknown",
    capture_runtime_status: "not_started",
    capture_permission_state: "unknown",
    can_reconnect: false,
  });
}

function isRuntimeActive(status) {
  return status === "starting" || status === "running";
}

function syncCaptureRuntimeControls(session) {
  const startButton = document.getElementById("start-capture-btn");
  const pollButton = document.getElementById("poll-capture-btn");
  const stopButton = document.getElementById("stop-capture-btn");
  const debugButton = document.getElementById("publish-media-btn");
  if (!startButton || !pollButton || !stopButton || !debugButton) {
    return;
  }

  const runtimeStatus = session.capture_runtime_status ?? "not_started";
  const isHost = session.mode === "host";
  const hasSource = Boolean(session.selected_source_id);
  const active = isRuntimeActive(runtimeStatus);

  startButton.disabled = actionInFlight || !isHost || !hasSource || active;
  pollButton.disabled = actionInFlight || runtimeStatus === "not_started";
  stopButton.disabled = actionInFlight
    || (!active
      && runtimeStatus !== "permission_required"
      && runtimeStatus !== "permission_denied"
      && runtimeStatus !== "failed");
  debugButton.disabled = actionInFlight || !isHost || !hasSource;

  startButton.title = !isHost
    ? "Prepare a host session before starting native capture."
    : !hasSource
      ? "Select a capture source before starting native capture."
      : active
        ? "Native capture is already active."
        : "";
  debugButton.title = !isHost || !hasSource
    ? "Prepare a host session and select a source before publishing debug media."
    : "";
}

function syncPrototypeControls(session) {
  const hostButton = document.getElementById("host-prototype-btn");
  const viewerButton = document.getElementById("viewer-prototype-btn");
  const debugButton = document.getElementById("host-debug-btn");
  const demoButton = document.getElementById("local-demo-btn");
  const refreshButton = document.getElementById("flow-refresh-btn");
  if (!hostButton || !viewerButton || !debugButton || !demoButton || !refreshButton) {
    return;
  }

  const isPreview = !isTauri;
  const hasRoom = Boolean(document.getElementById("room")?.value.trim());
  const hasSignaling = Boolean(document.getElementById("signaling")?.value.trim());
  const canStart = !isPreview && hasRoom && hasSignaling;
  const isHost = session.mode === "host";
  const isViewer = session.mode === "viewer";
  const hasSource = Boolean(session.selected_source_id);
  const captureActive = isRuntimeActive(session.capture_runtime_status);
  const peerLive = Boolean(session.active_peer || session.local_data_channel_ready);

  hostButton.disabled = actionInFlight || !canStart;
  viewerButton.disabled = actionInFlight || !canStart;
  debugButton.disabled = actionInFlight || isPreview || !isHost || !hasSource;
  demoButton.disabled = actionInFlight || isPreview;
  refreshButton.disabled = actionInFlight || isPreview;
  hostButton.textContent = isHost
    ? captureActive
      ? "Host Prototype Running"
      : "Restart Host Prototype"
    : "Start Host Prototype";
  viewerButton.textContent = isViewer
    ? peerLive
      ? "Viewer Connected"
      : "Refresh Viewer Session"
    : "Join Viewer";

  hostButton.title = canStart
    ? ""
    : isPreview
      ? "Start the app with cargo tauri dev so desktop commands are available."
      : "Room and signaling address are required.";
  viewerButton.title = canStart
    ? ""
    : isPreview
      ? "Start the app with cargo tauri dev so desktop commands are available."
      : "Room and signaling address are required.";
  debugButton.title = debugButton.disabled
    ? "Start a host session with a selected source before sending a test frame."
    : "";
  demoButton.title = isPreview
    ? "Start the app with cargo tauri dev so desktop commands are available."
    : "Start a local no-network demo session with the current capture catalog.";
}

function setCaptureRuntime(session) {
  const container = document.getElementById("capture-runtime");
  if (!container) {
    return;
  }

  container.innerHTML = detailRows([
    ["Status", session.capture_runtime_status ?? "not_started"],
    ["Permission", session.capture_permission_state ?? "unknown"],
    ["Selected", session.source_label ?? session.selected_source_id ?? "n/a"],
    ["Audio", session.selected_source_audio ? "enabled" : "disabled"],
  ]);
  syncCaptureRuntimeControls(session);
}

function setCaptureCatalog(catalog) {
  const container = document.getElementById("capture-catalog");
  const noteText = catalog.notes?.length ? catalog.notes.join(" | ") : "none";
  container.innerHTML = detailRows([
    ["Backend", catalog.backend],
    ["Origin", catalog.origin],
    ["Permission", catalog.permission_state],
    ["Sources", catalog.sources.length],
    ["Notes", noteText],
  ]);

  const picker = document.getElementById("source-picker");
  picker.innerHTML = catalog.sources
    .map(
      (source) =>
        `<option value="${escapeHtml(source.id)}">${escapeHtml(source.label)} (${escapeHtml(source.kind)}${source.has_audio ? ", audio" : ""})</option>`,
    )
    .join("");

  if (catalog.selected_source_id) {
    picker.value = catalog.selected_source_id;
  } else if (catalog.sources[0]) {
    picker.value = catalog.sources[0].id;
  }

  syncCaptureAudioState(catalog);
  picker.disabled = catalog.sources.length === 0;
}

function formatMetricMs(value) {
  return Number.isFinite(value) ? `${value.toFixed(1)}ms` : "n/a";
}

function formatMetricBitrate(value) {
  if (!Number.isFinite(value)) {
    return "n/a";
  }
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(2)} Mbps`;
  }
  if (value >= 1_000) {
    return `${(value / 1_000).toFixed(1)} kbps`;
  }
  return `${value.toFixed(0)} bps`;
}

function formatMetricBytes(value) {
  if (!Number.isFinite(value)) {
    return "n/a";
  }
  if (value >= 1024 * 1024) {
    return `${(value / (1024 * 1024)).toFixed(2)} MiB`;
  }
  if (value >= 1024) {
    return `${(value / 1024).toFixed(1)} KiB`;
  }
  return `${value.toFixed(0)} B`;
}

function formatPacketLoss(session) {
  if (!Number.isFinite(session.transport_packet_loss_fraction)) {
    return Number.isFinite(session.transport_packets_lost)
      ? `${session.transport_packets_lost} lost`
      : "n/a";
  }

  const percent = (session.transport_packet_loss_fraction * 100).toFixed(2);
  if (Number.isFinite(session.transport_packets_lost)) {
    return `${percent}% / ${session.transport_packets_lost} lost`;
  }
  return `${percent}%`;
}

function syncCaptureAudioState(catalog) {
  const picker = document.getElementById("source-picker");
  const audioToggle = document.getElementById("source-audio");
  const selected = catalog.sources.find((source) => source.id === picker.value) ?? catalog.sources[0];

  audioToggle.disabled = !selected || !selected.has_audio;
  if (catalog.selected_source_id === picker.value) {
    audioToggle.checked = Boolean(catalog.selected_source_audio);
  } else {
    audioToggle.checked = Boolean(selected?.has_audio);
  }
}

function syncReconnectControl(session) {
  const button = document.getElementById("reconnect-btn");
  if (!button) {
    return;
  }

  button.disabled = actionInFlight || !session.can_reconnect;
  button.classList.toggle("secondary", Boolean(session.reconnect_recommended));
  button.classList.toggle("ghost", !session.reconnect_recommended);
  button.title = session.recovery_reason ?? "";
}

function setRecoveryStatus(session) {
  const container = document.getElementById("recovery-status");
  container.innerHTML = detailRows([
    ["State", session.recovery_state ?? "unknown"],
    [
      "Reconnect",
      session.can_reconnect
        ? session.reconnect_recommended
          ? "recommended"
          : "available"
        : "unavailable",
    ],
    ["Reason", session.recovery_reason ?? "n/a"],
  ]);
  syncReconnectControl(session);
}

function setSession(session, { forceFormSync = false } = {}) {
  lastSession = session;
  const container = document.getElementById("session");
  container.innerHTML = detailRows([
    ["Mode", session.mode],
    ["Room", session.room ?? "n/a"],
    ["Signaling", session.signaling_addr ?? "n/a"],
    ["ICE Servers", `${session.ice_server_count ?? 0} / ${session.ice_server_summary ?? "none"}`],
    ["Signal Link", String(session.signaling_connected)],
    ["Source", session.source_label ?? "n/a"],
    [
      "Selected Source",
      `${session.selected_source_id ?? "n/a"} / ${String(session.selected_source_audio)}`,
    ],
    ["Capture Backend", session.capture_backend ?? "n/a"],
    ["Permission", session.capture_permission_state ?? "n/a"],
    ["Capture Runtime", session.capture_runtime_status ?? "not_started"],
    ["Peer", session.active_peer ?? "n/a"],
    ["Transport State", session.transport_state ?? "n/a"],
    ["Transport Stage", session.transport_stage ?? "n/a"],
    [
      "ICE Path",
      `${session.transport_ice_path_kind ?? "unknown"} / ${session.transport_ice_path_summary ?? "n/a"}`,
    ],
    ["RTT", formatMetricMs(session.transport_rtt_ms)],
    ["Outgoing Bitrate", formatMetricBitrate(session.transport_available_outgoing_bitrate_bps)],
    ["Incoming Bitrate", formatMetricBitrate(session.transport_available_incoming_bitrate_bps)],
    [
      "Link Bytes",
      `${formatMetricBytes(session.transport_bytes_sent)} sent / ${formatMetricBytes(session.transport_bytes_received)} recv`,
    ],
    ["Packet Loss", formatPacketLoss(session)],
    ["Media Tracks", session.local_media_track_count ?? 0],
    ["Video Track", String(session.local_video_track_attached)],
    ["Audio Track", String(session.local_audio_track_attached)],
    ["Data Channel", String(session.local_data_channel_ready)],
    ["Stats Reports", session.transport_stats_report_count ?? 0],
    [
      "Video Samples",
      `${session.published_video_sample_count ?? 0} / ${session.last_video_sample_bytes ?? 0}B`,
    ],
    [
      "Audio Samples",
      `${session.published_audio_sample_count ?? 0} / ${session.last_audio_sample_bytes ?? 0}B`,
    ],
    ["Video Payload", session.last_video_capture_summary ?? "n/a"],
    ["Audio Payload", session.last_audio_capture_summary ?? "n/a"],
    [
      "Local Desc",
      `${session.local_description_kind ?? "n/a"} / ${String(session.local_description_ready)}`,
    ],
    [
      "Remote Desc",
      `${session.remote_description_kind ?? "n/a"} / ${String(session.remote_description_ready)}`,
    ],
    ["Local ICE", session.local_candidate_count ?? 0],
    ["Remote ICE", session.remote_candidate_count ?? 0],
    ["Recovery", `${session.recovery_state ?? "unknown"} / ${session.recovery_reason ?? "n/a"}`],
    ["Next Action", session.next_action ?? "n/a"],
  ]);
  document.getElementById("session-log").textContent = session.logs.join("\n");
  document.getElementById("signal-preview").textContent =
    session.last_signaling_message ?? "No signaling messages yet.";
  document.getElementById("transport-diagnostics").innerHTML = detailRows([
    ["Transport Stage", session.transport_stage ?? "n/a"],
    ["ICE Path", session.transport_ice_path_summary ?? "n/a"],
    ["RTT", formatMetricMs(session.transport_rtt_ms)],
    ["Outgoing Bitrate", formatMetricBitrate(session.transport_available_outgoing_bitrate_bps)],
    ["Incoming Bitrate", formatMetricBitrate(session.transport_available_incoming_bitrate_bps)],
    [
      "Link Bytes",
      `${formatMetricBytes(session.transport_bytes_sent)} sent / ${formatMetricBytes(session.transport_bytes_received)} recv`,
    ],
    ["Packet Loss", formatPacketLoss(session)],
    ["Data Channel", String(session.local_data_channel_ready)],
    ["Stats Reports", session.transport_stats_report_count ?? 0],
  ]);
  document.getElementById("transport-notes").textContent =
    session.transport_notes?.join("\n") || "No transport diagnostics yet.";
  setOverview(session);
  setPrototypeFlow(session);
  setCaptureRuntime(session);
  setRecoveryStatus(session);
  syncSessionFieldValue("room", session.room, forceFormSync);
  syncSessionFieldValue("signaling", session.signaling_addr, forceFormSync);
  syncSessionFieldValue("ice-servers", session.ice_servers, forceFormSync);
}

function normalizeRefreshIntervalSeconds(value) {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || !allowedRefreshIntervalSeconds.has(parsed)) {
    return defaultRefreshIntervalSeconds;
  }
  return parsed;
}

function syncUiPreferenceControls(session) {
  const autoRefreshToggle = document.getElementById("auto-refresh-enabled");
  const refreshIntervalField = document.getElementById("refresh-interval");
  if (!autoRefreshToggle || !refreshIntervalField) {
    return;
  }

  autoRefreshToggle.checked = Boolean(session.ui_auto_refresh_enabled);
  refreshIntervalField.value = String(
    normalizeRefreshIntervalSeconds(session.ui_refresh_interval_secs),
  );
  refreshIntervalField.disabled = !autoRefreshToggle.checked;
}

function uiPreferenceValues() {
  return {
    auto_refresh_enabled: document.getElementById("auto-refresh-enabled").checked,
    refresh_interval_secs: normalizeRefreshIntervalSeconds(
      document.getElementById("refresh-interval").value,
    ),
  };
}

function configureRefreshTimer({ auto_refresh_enabled, refresh_interval_secs }) {
  if (refreshTimerId !== null) {
    window.clearInterval(refreshTimerId);
    refreshTimerId = null;
  }

  document.getElementById("refresh-interval").disabled = !auto_refresh_enabled;
  if (!isTauri || !auto_refresh_enabled) {
    return;
  }

  refreshTimerId = window.setInterval(
    refresh,
    normalizeRefreshIntervalSeconds(refresh_interval_secs) * 1000,
  );
}

function formValues() {
  return {
    room: document.getElementById("room").value.trim(),
    signaling_addr: document.getElementById("signaling").value.trim(),
    ice_servers: document.getElementById("ice-servers").value.trim(),
  };
}

async function performRefresh() {
  const [statusResult, sessionResult, captureCatalogResult] = await Promise.allSettled([
    invokeWithTimeout("project_status"),
    invokeWithTimeout("session_snapshot"),
    invokeWithTimeout("capture_catalog"),
  ]);
  const status = statusResult.status === "fulfilled" ? statusResult.value : null;
  const session = sessionResult.status === "fulfilled" ? sessionResult.value : null;
  const captureCatalog =
    captureCatalogResult.status === "fulfilled" ? captureCatalogResult.value : null;

  [statusResult, sessionResult, captureCatalogResult]
    .filter((result) => result.status === "rejected")
    .forEach((result) => console.warn(result.reason));

  if (status) {
    setStatus(status);
    document.getElementById("runtime-badge").textContent = "Tauri runtime connected";
    document.getElementById("runtime-badge").className = "badge live";
  } else {
    document.getElementById("runtime-badge").textContent = isTauri
      ? "Tauri command timeout"
      : "Browser preview";
    document.getElementById("runtime-badge").className = "badge preview";
    setPreviewOverview();
  }

  if (session) {
    setSession(session);
    syncUiPreferenceControls(session);
    configureRefreshTimer({
      auto_refresh_enabled: session.ui_auto_refresh_enabled,
      refresh_interval_secs: session.ui_refresh_interval_secs,
    });
  } else {
    const fallbackMode = isTauri ? "idle" : "preview";
    const fallbackNextAction = isTauri ? "refresh timed out; run local demo or try refresh" : "run inside Tauri";
    document.getElementById("session").innerHTML = `
      <div><dt>Mode</dt><dd>${fallbackMode}</dd></div>
      <div><dt>Room</dt><dd>n/a</dd></div>
      <div><dt>Signaling</dt><dd>n/a</dd></div>
      <div><dt>ICE Servers</dt><dd>0 / none</dd></div>
      <div><dt>Signal Link</dt><dd>false</dd></div>
      <div><dt>Source</dt><dd>n/a</dd></div>
      <div><dt>Selected Source</dt><dd>n/a / false</dd></div>
      <div><dt>Capture Backend</dt><dd>preview</dd></div>
      <div><dt>Permission</dt><dd>unknown</dd></div>
      <div><dt>Capture Runtime</dt><dd>not_started</dd></div>
      <div><dt>Peer</dt><dd>n/a</dd></div>
      <div><dt>Transport State</dt><dd>preview</dd></div>
      <div><dt>Transport Stage</dt><dd>preview</dd></div>
      <div><dt>ICE Path</dt><dd>unknown / preview</dd></div>
      <div><dt>RTT</dt><dd>n/a</dd></div>
      <div><dt>Outgoing Bitrate</dt><dd>n/a</dd></div>
      <div><dt>Incoming Bitrate</dt><dd>n/a</dd></div>
      <div><dt>Link Bytes</dt><dd>n/a / n/a</dd></div>
      <div><dt>Packet Loss</dt><dd>n/a</dd></div>
      <div><dt>Media Tracks</dt><dd>0</dd></div>
      <div><dt>Video Track</dt><dd>false</dd></div>
      <div><dt>Audio Track</dt><dd>false</dd></div>
      <div><dt>Data Channel</dt><dd>false</dd></div>
      <div><dt>Stats Reports</dt><dd>0</dd></div>
      <div><dt>Video Samples</dt><dd>0 / 0B</dd></div>
      <div><dt>Audio Samples</dt><dd>0 / 0B</dd></div>
      <div><dt>Video Payload</dt><dd>n/a</dd></div>
      <div><dt>Audio Payload</dt><dd>n/a</dd></div>
      <div><dt>Local Desc</dt><dd>n/a / false</dd></div>
      <div><dt>Remote Desc</dt><dd>n/a / false</dd></div>
      <div><dt>Local ICE</dt><dd>0</dd></div>
      <div><dt>Remote ICE</dt><dd>0</dd></div>
      <div><dt>Recovery</dt><dd>${fallbackMode} / n/a</dd></div>
      <div><dt>Next Action</dt><dd>${fallbackNextAction}</dd></div>
    `;
    document.getElementById("session-log").textContent =
      isTauri
        ? "Session refresh did not finish quickly. Run Local Demo remains available."
        : "Run inside Tauri to drive the in-memory session manager.";
    document.getElementById("signal-preview").textContent =
      "Run inside Tauri to preview signaling state.";
    document.getElementById("transport-diagnostics").innerHTML = `
      <div><dt>Transport Stage</dt><dd>preview</dd></div>
      <div><dt>ICE Path</dt><dd>preview</dd></div>
      <div><dt>RTT</dt><dd>n/a</dd></div>
      <div><dt>Outgoing Bitrate</dt><dd>n/a</dd></div>
      <div><dt>Incoming Bitrate</dt><dd>n/a</dd></div>
      <div><dt>Link Bytes</dt><dd>n/a / n/a</dd></div>
      <div><dt>Packet Loss</dt><dd>n/a</dd></div>
      <div><dt>Data Channel</dt><dd>false</dd></div>
      <div><dt>Stats Reports</dt><dd>0</dd></div>
    `;
    document.getElementById("transport-notes").textContent =
      "Run inside Tauri to inspect Rust-side transport diagnostics.";
    document.getElementById("capture-runtime").innerHTML = `
      <div><dt>Status</dt><dd>not_started</dd></div>
      <div><dt>Permission</dt><dd>unknown</dd></div>
      <div><dt>Selected</dt><dd>n/a</dd></div>
      <div><dt>Audio</dt><dd>disabled</dd></div>
    `;
    syncCaptureRuntimeControls({
      mode: "preview",
      selected_source_id: null,
      capture_runtime_status: "not_started",
    });
    document.getElementById("recovery-status").innerHTML = `
      <div><dt>State</dt><dd>preview</dd></div>
      <div><dt>Reconnect</dt><dd>unavailable</dd></div>
      <div><dt>Reason</dt><dd>Run inside Tauri to inspect session recovery diagnostics.</dd></div>
    `;
    syncReconnectControl({
      can_reconnect: false,
      reconnect_recommended: false,
      recovery_reason: "Run inside Tauri to inspect session recovery diagnostics.",
    });
    configureRefreshTimer({
      auto_refresh_enabled: false,
      refresh_interval_secs: defaultRefreshIntervalSeconds,
    });
    setPrototypeFlow({
      mode: fallbackMode,
      room: document.getElementById("room")?.value || "demo",
      signaling_connected: false,
      signaling_addr: "offline",
      transport_stage: fallbackMode,
      transport_ice_path_kind: "unknown",
      capture_runtime_status: "not_started",
      capture_permission_state: "unknown",
      can_reconnect: false,
    });
  }

  if (captureCatalog) {
    setCaptureCatalog(captureCatalog);
  } else {
    document.getElementById("capture-catalog").innerHTML = `
      <div><dt>Backend</dt><dd>preview</dd></div>
      <div><dt>Origin</dt><dd>preview</dd></div>
      <div><dt>Permission</dt><dd>unknown</dd></div>
      <div><dt>Sources</dt><dd>0</dd></div>
      <div><dt>Notes</dt><dd>${isTauri ? "Capture catalog refresh timed out." : "Run inside Tauri to inspect capture catalog diagnostics."}</dd></div>
    `;
    document.getElementById("source-picker").innerHTML = "";
    document.getElementById("source-picker").disabled = true;
  }
}

async function refresh() {
  if (actionInFlight) {
    return null;
  }
  if (refreshInFlight) {
    return refreshInFlight;
  }

  refreshInFlight = performRefresh().finally(() => {
    refreshInFlight = null;
  });
  return refreshInFlight;
}

async function runUserAction(message, action) {
  if (actionInFlight) {
    return null;
  }

  setActionBusy(true, message);
  try {
    const result = await action();
    return result;
  } finally {
    setActionBusy(false);
    await refresh();
  }
}

async function runCommand(command, args = {}, options = {}) {
  const { forceFormSync = false, clearDraftOnSuccess = false } = options;
  let result = null;
  try {
    result = await invokeWithTimeout(command, args, userActionTimeoutMs);
  } catch (error) {
    setCommandResult(`${command} did not finish: ${error.message}`, "bad");
    return null;
  }
  if (!result) {
    setCommandResult(`Preview mode: skipped ${command}`, "warn");
    return null;
  }

  setCommandResult(result.message, result.ok ? "good" : "bad");
  setSession(result.session, { forceFormSync: forceFormSync && result.ok });
  if (result.ok && clearDraftOnSuccess) {
    clearSessionFieldDrafts();
  }
  const status = await invokeWithTimeout("project_status").catch((error) => {
    console.warn(error);
    return null;
  });
  if (status) {
    setStatus(status);
  }
  return result;
}

async function updateUiPreferences() {
  const preferences = uiPreferenceValues();
  configureRefreshTimer(preferences);
  const result = await runCommand("update_ui_preferences", preferences);
  if (result?.session) {
    syncUiPreferenceControls(result.session);
    configureRefreshTimer({
      auto_refresh_enabled: result.session.ui_auto_refresh_enabled,
      refresh_interval_secs: result.session.ui_refresh_interval_secs,
    });
  }
}

async function reconnectSession() {
  if (hasDirtySessionFields()) {
    const values = formValues();
    const updateResult = await runCommand("update_session_config", values, {
      forceFormSync: true,
      clearDraftOnSuccess: true,
    });
    if (!updateResult?.ok) {
      return;
    }
  }

  await runCommand("reconnect_session", {}, {
    forceFormSync: true,
    clearDraftOnSuccess: true,
  });
}

async function startHostPrototype() {
  const values = formValues();
  const hostResult = await runCommand("start_host", values, {
    forceFormSync: true,
    clearDraftOnSuccess: true,
  });
  if (!hostResult?.ok) {
    return;
  }

  await applySelectedSource();
  const captureResult = await runCommand("start_capture_stream");
  await runCommand("publish_debug_capture_samples");
  if (!captureResult?.ok) {
    setCommandResult(
      "Host prototype started with test frames; native capture is not available yet.",
      "warn",
    );
  }
}

async function joinViewerPrototype() {
  const values = formValues();
  return runCommand("join_room", values, {
    forceFormSync: true,
    clearDraftOnSuccess: true,
  });
}

async function startLocalDemo() {
  return runCommand("start_local_demo", {}, {
    forceFormSync: true,
    clearDraftOnSuccess: true,
  });
}

async function applySelectedSource() {
  if (!isTauri) {
    return;
  }

  const source_id = document.getElementById("source-picker").value;
  const include_audio = document.getElementById("source-audio").checked;
  if (!source_id) {
    return;
  }

  await runCommand("select_capture_source", { source_id, include_audio });
}

async function load() {
  const specification = await invokeWithTimeout("specification_markdown").catch((error) => {
    console.warn(error);
    return null;
  });
  await refresh();

  document.getElementById("spec").textContent =
    specification ?? "Run inside Tauri to load the saved specification from the Rust backend.";

  sessionFieldIds.forEach((id) => {
    document.getElementById(id).addEventListener("input", markSessionFieldDirty);
  });

  document.getElementById("save-btn").addEventListener("click", async () => {
    const values = formValues();
    await runUserAction("Saving session configuration...", () =>
      runCommand("update_session_config", values, {
        forceFormSync: true,
        clearDraftOnSuccess: true,
      }),
    );
  });

  document.getElementById("host-btn").addEventListener("click", async () => {
    const values = formValues();
    await runUserAction("Preparing host session...", () =>
      runCommand("start_host", values, {
        forceFormSync: true,
        clearDraftOnSuccess: true,
      }),
    );
  });

  document.getElementById("join-btn").addEventListener("click", async () => {
    const values = formValues();
    await runUserAction("Preparing viewer session...", () =>
      runCommand("join_room", values, {
        forceFormSync: true,
        clearDraftOnSuccess: true,
      }),
    );
  });

  document
    .getElementById("host-prototype-btn")
    .addEventListener("click", () =>
      runUserAction("Starting host prototype...", startHostPrototype),
    );
  document
    .getElementById("viewer-prototype-btn")
    .addEventListener("click", () =>
      runUserAction("Joining viewer session...", joinViewerPrototype),
    );
  document
    .getElementById("local-demo-btn")
    .addEventListener("click", () =>
      runUserAction("Starting local desktop demo...", startLocalDemo),
    );
  document.getElementById("host-debug-btn").addEventListener("click", async () => {
    await runUserAction("Sending test capture frame...", () =>
      runCommand("publish_debug_capture_samples"),
    );
  });
  document.getElementById("flow-refresh-btn").addEventListener("click", refresh);

  document.getElementById("source-picker").addEventListener("change", async () => {
    const catalog = await invokeWithTimeout("capture_catalog").catch((error) => {
      console.warn(error);
      return null;
    });
    if (catalog) {
      syncCaptureAudioState(catalog);
    }
    await runUserAction("Selecting capture source...", applySelectedSource);
  });

  document
    .getElementById("source-audio")
    .addEventListener("change", () =>
      runUserAction("Updating capture audio preference...", applySelectedSource),
    );

  document
    .getElementById("auto-refresh-enabled")
    .addEventListener("change", () =>
      runUserAction("Updating refresh preferences...", updateUiPreferences),
    );
  document
    .getElementById("refresh-interval")
    .addEventListener("change", () =>
      runUserAction("Updating refresh preferences...", updateUiPreferences),
    );

  document.getElementById("publish-media-btn").addEventListener("click", async () => {
    await runUserAction("Sending test capture frame...", () =>
      runCommand("publish_debug_capture_samples"),
    );
  });

  document.getElementById("start-capture-btn").addEventListener("click", async () => {
    await runUserAction("Starting native capture...", () => runCommand("start_capture_stream"));
  });

  document.getElementById("poll-capture-btn").addEventListener("click", async () => {
    await runUserAction("Polling native capture...", () => runCommand("poll_capture_stream"));
  });

  document.getElementById("stop-capture-btn").addEventListener("click", async () => {
    await runUserAction("Stopping native capture...", () => runCommand("stop_capture_stream"));
  });

  document.getElementById("stop-btn").addEventListener("click", async () => {
    await runUserAction("Stopping session...", () => runCommand("stop_session"));
  });

  document
    .getElementById("reconnect-btn")
    .addEventListener("click", () => runUserAction("Reconnecting session...", reconnectSession));

  document.getElementById("reset-btn").addEventListener("click", async () => {
    await runUserAction("Resetting session...", () =>
      runCommand("reset_session", {}, {
        forceFormSync: true,
        clearDraftOnSuccess: true,
      }),
    );
  });

  document.getElementById("clear-logs-btn").addEventListener("click", async () => {
    await runUserAction("Clearing session logs...", () => runCommand("clear_session_logs"));
  });

  document.getElementById("refresh-btn").addEventListener("click", refresh);
}

load().catch((error) => {
  document.getElementById("spec").textContent = String(error);
});
