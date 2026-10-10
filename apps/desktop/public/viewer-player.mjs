// A receive-only WebRTC peer: the WebView owns jitter buffering, decoding and A/V output.
export class ViewerPlayer {
  constructor({ video, invoke, onStatus = () => {}, peerFactory, streamFactory, pollMs = 100, statsMs = 1000 }) {
    this.video = video;
    this.invoke = invoke;
    this.onStatus = onStatus;
    this.peerFactory = peerFactory ?? ((config) => {
      if (typeof globalThis.RTCPeerConnection !== "function") throw new Error("WebRTC playback is unavailable in this WebView. Update the system WebView and reconnect.");
      return new globalThis.RTCPeerConnection(config);
    });
    this.streamFactory = streamFactory ?? (() => new globalThis.MediaStream());
    this.pollMs = pollMs;
    this.statsMs = statsMs;
    this.context = null;
    video.addEventListener("playing", () => this.mediaState("playing"));
    video.addEventListener("pause", () => this.mediaState("paused"));
    video.addEventListener("error", () => {
      if (this.context) void this.fail(this.context, new Error("Video playback failed"));
    });
  }

  sync(session) {
    if (session.mode !== "viewer" || !session.viewer_generation || session.stage === "stopped") {
      this.stop();
      return;
    }
    if (this.context?.generation === session.viewer_generation) return;
    this.stop();
    const context = {
      generation: session.viewer_generation, closed: false, peer: null, stream: null,
      pendingIce: [], localIce: [], answerSent: false, outgoing: Promise.resolve(),
      state: "waiting", error: null, lastStats: 0, report: null, dataReady: false,
      timer: null, wake: null,
    };
    this.context = context;
    this.video.muted = true;
    this.notify(context);
    void this.run(context).catch((error) => this.fail(context, error));
  }

  current(context) { return this.context === context && !context.closed; }

  notify(context) {
    if (this.context !== context) return;
    this.onStatus({
      ...playbackStatus(context), muted: this.video.muted,
      hasAudio: Boolean(!context.closed && context.stream?.getAudioTracks().length),
      videoFrames: context.report?.video_frames ?? 0, audioSamples: context.report?.audio_samples ?? 0,
    });
  }

  mediaState(state) {
    const context = this.context;
    if (context && this.current(context) && this.video.srcObject === context.stream && context.stream?.getTracks().length) {
      context.state = state;
      context.error = null;
      this.notify(context);
    }
  }

  async setMuted(muted) {
    const context = this.context;
    if (!context || !this.current(context)) return;
    this.video.muted = muted;
    await this.play(context);
    this.notify(context);
  }

  async play(context) {
    try {
      await this.video.play();
      if (this.current(context)) {
        context.state = "playing";
        context.error = null;
      }
    } catch (error) {
      if (!this.current(context)) return;
      context.state = "blocked";
      context.error = `Click Play or Enable Sound to allow playback: ${error.message ?? error}`;
    }
    this.notify(context);
  }

  closeContext(context) {
    context.closed = true;
    if (context.timer !== null) clearTimeout(context.timer);
    context.wake?.();
    if (context.peer) {
      context.peer.ontrack = null;
      context.peer.onicecandidate = null;
      context.peer.ondatachannel = null;
      context.peer.onconnectionstatechange = null;
      context.peer.close();
    }
    context.stream?.getTracks().forEach((track) => track.stop());
    if (this.video.srcObject === context.stream) {
      this.video.pause();
      this.video.srcObject = null;
    }
    context.pendingIce.length = 0;
    context.localIce.length = 0;
  }

  stop() {
    if (!this.context) return;
    this.closeContext(this.context);
    this.context = null;
    this.onStatus({ state: "idle", error: null, muted: true, hasAudio: false, videoFrames: 0, audioSamples: 0 });
  }

  async fail(context, error) {
    if (!this.current(context)) return;
    context.state = error.unsupported ? "unsupported" : "failed";
    context.error = String(error.message ?? error);
    this.closeContext(context);
    this.notify(context);
    // A rejected/stale report cannot affect the replacement session.
    try {
      await this.invoke("viewer_report_playback", {
        generation: context.generation,
        report: { ...emptyReport(), connection_state: "failed", playback_state: context.state, error: context.error },
      });
    } catch { /* The backend may already have stopped or replaced this generation. */ }
  }

  createPeer(context, iceServers) {
    try {
      context.peer = this.peerFactory({ bundlePolicy: "max-bundle", iceServers: iceServers.map((server) => ({
        urls: server.urls, ...(server.username ? { username: server.username, credential: server.credential } : {}),
      })) });
      context.stream = this.streamFactory();
    } catch (error) {
      error.unsupported = true;
      throw error;
    }
    this.video.srcObject = context.stream;
    context.peer.ontrack = (event) => {
      if (!this.current(context)) return;
      if (!context.stream.getTracks().some((track) => track.id === event.track.id)) context.stream.addTrack(event.track);
      this.notify(context);
      void this.play(context);
    };
    context.peer.onicecandidate = (event) => {
      if (!event.candidate || !this.current(context)) return;
      const candidate = event.candidate.toJSON();
      const signal = { kind: "ice", candidate: candidate.candidate, sdp_mid: candidate.sdpMid ?? null, sdp_mline_index: candidate.sdpMLineIndex ?? null };
      if (!context.answerSent) {
        if (context.localIce.length >= 128) { void this.fail(context, new Error("Too many pending local ICE candidates")); return; }
        context.localIce.push(signal);
      } else {
        void this.send(context, signal).catch((error) => this.fail(context, error));
      }
    };
    context.peer.ondatachannel = (event) => {
      event.channel.onopen = () => { if (this.current(context)) context.dataReady = true; };
      event.channel.onclose = () => { context.dataReady = false; };
    };
    context.peer.onconnectionstatechange = () => {
      if (!this.current(context)) return;
      if (context.peer.connectionState === "failed") {
        void this.fail(context, new Error("Peer connection failed. Reconnect both peers to retry."));
        return;
      }
      this.notify(context);
    };
  }

  send(context, signal) {
    context.outgoing = context.outgoing.then(async () => {
      if (!this.current(context)) return;
      await this.invoke("viewer_send_signal", { generation: context.generation, signal });
    });
    return context.outgoing;
  }

  async accept(context, signal) {
    const peer = context.peer;
    if (signal.kind === "offer") {
      context.answerSent = false;
      await peer.setRemoteDescription({ type: "offer", sdp: signal.sdp });
      if (!this.current(context)) return;
      for (const candidate of context.pendingIce.splice(0)) {
        await peer.addIceCandidate(candidate);
        if (!this.current(context)) return;
      }
      const answer = await peer.createAnswer();
      if (!this.current(context)) return;
      await peer.setLocalDescription(answer);
      if (!this.current(context)) return;
      await this.send(context, { kind: "answer", sdp: peer.localDescription.sdp });
      if (!this.current(context)) return;
      context.answerSent = true;
      for (const ice of context.localIce.splice(0)) await this.send(context, ice);
    } else if (signal.kind === "ice") {
      const candidate = { candidate: signal.candidate, sdpMid: signal.sdp_mid, sdpMLineIndex: signal.sdp_mline_index };
      if (peer.remoteDescription) await peer.addIceCandidate(candidate);
      else {
        if (context.pendingIce.length >= 128) throw new Error("Too many pending remote ICE candidates");
        context.pendingIce.push(candidate);
      }
    } else throw new Error("Unexpected viewer signaling message");
  }

  async run(context) {
    while (this.current(context)) {
      const batch = await this.invoke("viewer_poll_signals", { generation: context.generation });
      if (!this.current(context)) return;
      if (batch.generation !== context.generation) throw new Error("Viewer signaling generation changed");
      if (!batch.active) { this.stop(); return; }
      if (!context.peer) this.createPeer(context, batch.ice_servers);
      for (const signal of batch.signals) {
        if (!this.current(context)) return;
        await this.accept(context, signal);
      }
      if (!this.current(context)) return;
      if (Date.now() - context.lastStats >= this.statsMs) {
        const stats = await context.peer.getStats();
        if (!this.current(context)) return;
        context.report = collectReport(stats, context.peer, this.video, context);
        await this.invoke("viewer_report_playback", { generation: context.generation, report: context.report });
        if (!this.current(context)) return;
        context.lastStats = Date.now();
        this.notify(context);
      }
      await new Promise((resolve) => {
        context.wake = resolve;
        context.timer = setTimeout(resolve, this.pollMs);
      });
      context.timer = null;
      context.wake = null;
    }
  }
}

function playbackStatus(context) {
  if (!context.closed && context.peer?.connectionState === "disconnected") {
    return { state: "waiting", error: "Connection interrupted; waiting for recovery." };
  }
  return { state: context.state, error: context.error };
}

function emptyReport() {
  return {
    connection_state: "new", playback_state: "waiting", error: null,
    video_packets: 0, audio_packets: 0, video_frames: 0, audio_samples: 0,
    bytes_received: 0, packets_lost: 0, rtt_ms: null, ice_path_kind: "unknown",
    ice_path_summary: "candidate pair not selected yet", data_channel_ready: false, stats_reports: 0,
  };
}

export function collectReport(stats, peer, video, context) {
  const playback = playbackStatus(context);
  const result = { ...emptyReport(), connection_state: peer.connectionState ?? "new",
    playback_state: playback.state, error: playback.error, data_channel_ready: context.dataReady,
    stats_reports: stats.size };
  let selectedPair = null;
  for (const stat of stats.values()) {
    if (stat.type === "inbound-rtp" && !stat.isRemote) {
      const kind = stat.kind ?? stat.mediaType;
      if (kind === "video") { result.video_packets += stat.packetsReceived ?? 0; result.video_frames += stat.framesDecoded ?? 0; }
      if (kind === "audio") { result.audio_packets += stat.packetsReceived ?? 0; result.audio_samples += stat.totalSamplesReceived ?? 0; }
      result.bytes_received += stat.bytesReceived ?? 0;
      result.packets_lost += stat.packetsLost ?? 0;
    }
    if (stat.type === "transport" && stat.selectedCandidatePairId) selectedPair = stats.get(stat.selectedCandidatePairId);
  }
  selectedPair ??= [...stats.values()].find((stat) => stat.type === "candidate-pair" && stat.nominated && stat.state === "succeeded");
  if (selectedPair) {
    const local = stats.get(selectedPair.localCandidateId);
    const remote = stats.get(selectedPair.remoteCandidateId);
    result.ice_path_kind = [local, remote].some((candidate) => candidate?.candidateType === "relay") ? "relay" : "direct";
    result.ice_path_summary = `${local?.candidateType ?? "unknown"} → ${remote?.candidateType ?? "unknown"}`;
    const rtt = selectedPair.currentRoundTripTime * 1000;
    result.rtt_ms = Number.isFinite(rtt) ? rtt : null;
    result.bytes_received = selectedPair.bytesReceived ?? result.bytes_received;
  }
  result.video_frames = Math.max(result.video_frames, video.getVideoPlaybackQuality?.().totalVideoFrames ?? video.webkitDecodedFrameCount ?? 0);
  return result;
}
