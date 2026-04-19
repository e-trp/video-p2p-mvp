async function invoke(command) {
  if (!window.__TAURI__?.core?.invoke) {
    return null;
  }
  return window.__TAURI__.core.invoke(command);
}

async function load() {
  const status = await invoke("project_status");
  const specification = await invoke("specification_markdown");

  if (status) {
    const container = document.getElementById("status");
    container.innerHTML = `
      <div><dt>Stage</dt><dd>${status.stage}</dd></div>
      <div><dt>GUI</dt><dd>${status.gui}</dd></div>
      <div><dt>Transport</dt><dd>${status.transport}</dd></div>
      <div><dt>macOS Capture</dt><dd>${status.capture_macos}</dd></div>
      <div><dt>Linux Capture</dt><dd>${status.capture_linux}</dd></div>
    `;
  } else {
    document.getElementById("status").innerHTML = `
      <div><dt>Mode</dt><dd>Browser preview without Tauri runtime</dd></div>
    `;
  }

  document.getElementById("spec").textContent =
    specification ?? "Run inside Tauri to load the saved specification from the Rust backend.";
}

load().catch((error) => {
  document.getElementById("spec").textContent = String(error);
});
