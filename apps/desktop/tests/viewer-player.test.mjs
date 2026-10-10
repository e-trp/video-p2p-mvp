import test from "node:test";
import assert from "node:assert/strict";
import { ViewerPlayer, collectReport } from "../public/viewer-player.mjs";

class Video extends EventTarget {
  constructor() { super(); this.muted = true; this.srcObject = null; this.blockPlay = false; }
  async play() { if (this.blockPlay) throw new Error("user gesture required"); this.dispatchEvent(new Event("playing")); }
  pause() { this.dispatchEvent(new Event("pause")); }
  getVideoPlaybackQuality() { return { totalVideoFrames: 3 }; }
}
class Stream {
  constructor() { this.tracks = []; }
  getTracks() { return this.tracks; }
  getAudioTracks() { return this.tracks.filter((track) => track.kind === "audio"); }
  addTrack(track) { this.tracks.push(track); }
}
class Peer {
  constructor() { this.operations = []; this.connectionState = "connected"; this.closed = false; }
  async setRemoteDescription(offer) { this.operations.push("offer"); this.remoteDescription = offer; }
  async addIceCandidate() { this.operations.push("ice"); }
  async createAnswer() { this.operations.push("answer"); return { type: "answer", sdp: "answer-sdp" }; }
  async setLocalDescription(answer) {
    this.localDescription = answer;
    this.onicecandidate?.({ candidate: { toJSON: () => ({ candidate: "local", sdpMid: "0", sdpMLineIndex: 0 }) } });
  }
  async getStats() { return new Map(); }
  close() { this.closed = true; }
}
const session = (generation) => ({ mode: "viewer", stage: "awaiting_peer", viewer_generation: generation });
const batch = (generation, signals = []) => ({ generation, active: true, signals, ice_servers: [] });
async function until(predicate) {
  const deadline = Date.now() + 1000;
  while (!predicate()) { assert.ok(Date.now() < deadline, "condition timed out"); await new Promise((resolve) => setTimeout(resolve, 5)); }
}

test("ICE is buffered until SDP is applied and local answer is sent before local ICE", async (t) => {
  const peer = new Peer();
  const sent = [];
  let polls = 0;
  const player = new ViewerPlayer({ video: new Video(), peerFactory: () => peer, streamFactory: () => new Stream(), pollMs: 5,
    invoke: async (command, args) => {
      if (command === "viewer_poll_signals") return batch(1, polls++ ? [] : [
        { kind: "ice", candidate: "remote", sdp_mid: "0", sdp_mline_index: 0 }, { kind: "offer", sdp: "offer-sdp" },
      ]);
      if (command === "viewer_send_signal") sent.push(args.signal.kind);
    },
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => sent.length === 2);
  assert.deepEqual(peer.operations, ["offer", "ice", "answer"]);
  assert.deepEqual(sent, ["answer", "ice"]);
});

test("a delayed poll from an old generation cannot create a peer or send signals", async (t) => {
  let resolveOld;
  const oldPoll = new Promise((resolve) => { resolveOld = resolve; });
  const peers = [];
  const sent = [];
  const player = new ViewerPlayer({ video: new Video(), streamFactory: () => new Stream(), pollMs: 5,
    peerFactory: () => { const peer = new Peer(); peers.push(peer); return peer; },
    invoke: async (command, args) => {
      if (command === "viewer_poll_signals") return args.generation === 1 ? oldPoll : batch(2);
      if (command === "viewer_send_signal") sent.push(args.generation);
    },
  });
  t.after(() => player.stop());
  player.sync(session(1));
  player.sync(session(2));
  resolveOld(batch(1, [{ kind: "offer", sdp: "old" }]));
  await until(() => peers.length === 1);
  assert.equal(player.context.generation, 2);
  assert.deepEqual(sent, []);
});

test("autoplay rejection recovers on a user action and stop releases peer, tracks and media", async (t) => {
  const video = new Video();
  video.blockPlay = true;
  const peer = new Peer();
  const statuses = [];
  const player = new ViewerPlayer({ video, peerFactory: () => peer, streamFactory: () => new Stream(), pollMs: 5,
    onStatus: (status) => statuses.push(status), invoke: async (command) => command === "viewer_poll_signals" ? batch(1) : undefined,
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => peer.ontrack);
  let stopped = false;
  peer.ontrack({ track: { id: "audio", kind: "audio", stop: () => { stopped = true; } } });
  await until(() => statuses.at(-1).state === "blocked");
  assert.equal(statuses.at(-1).hasAudio, true);
  video.blockPlay = false;
  await player.setMuted(false);
  assert.equal(statuses.at(-1).state, "playing");
  assert.equal(video.muted, false);
  player.sync({ ...session(1), stage: "stopped" });
  assert.equal(stopped, true);
  assert.equal(peer.closed, true);
  assert.equal(video.srcObject, null);
  assert.equal(player.context, null);
});

test("unsupported playback reports a failure without repeatedly constructing peers", async (t) => {
  let factories = 0;
  let report;
  const player = new ViewerPlayer({ video: new Video(), peerFactory: () => { factories++; throw new Error("unsupported WebRTC"); },
    invoke: async (command, args) => {
      if (command === "viewer_poll_signals") return batch(1);
      if (command === "viewer_report_playback") report = args.report;
    },
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => report);
  player.sync(session(1));
  assert.equal(report.playback_state, "unsupported");
  assert.equal(factories, 1);
});

test("a temporary disconnection clears on recovery and preserves autoplay guidance", async (t) => {
  const video = new Video();
  video.blockPlay = true;
  const peer = new Peer();
  const statuses = [];
  const player = new ViewerPlayer({ video, peerFactory: () => peer, streamFactory: () => new Stream(), pollMs: 5,
    onStatus: (status) => statuses.push(status), invoke: async (command) => command === "viewer_poll_signals" ? batch(1) : undefined,
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => peer.ontrack);
  peer.ontrack({ track: { id: "video", kind: "video", stop() {} } });
  await until(() => statuses.at(-1).state === "blocked");
  const blocked = statuses.at(-1).error;
  peer.connectionState = "disconnected";
  peer.onconnectionstatechange();
  assert.equal(statuses.at(-1).state, "waiting");
  assert.match(statuses.at(-1).error, /interrupted/);
  assert.equal(collectReport(new Map(), peer, video, player.context).playback_state, "waiting");
  peer.connectionState = "connected";
  peer.onconnectionstatechange();
  assert.equal(statuses.at(-1).state, "blocked");
  assert.equal(statuses.at(-1).error, blocked);
  video.blockPlay = false;
  await player.setMuted(true);
  peer.connectionState = "disconnected";
  peer.onconnectionstatechange();
  peer.connectionState = "connected";
  peer.onconnectionstatechange();
  assert.equal(statuses.at(-1).state, "playing");
  assert.equal(statuses.at(-1).error, null);
});

test("stopping while buffered ICE is applied prevents later peer operations", async (t) => {
  const peer = new Peer();
  let resolveIce;
  peer.addIceCandidate = async () => {
    peer.operations.push("ice");
    await new Promise((resolve) => { resolveIce = resolve; });
  };
  const sent = [];
  const player = new ViewerPlayer({ video: new Video(), peerFactory: () => peer, streamFactory: () => new Stream(),
    invoke: async (command, args) => {
      if (command === "viewer_poll_signals") return batch(1, [
        { kind: "ice", candidate: "remote", sdp_mid: "0", sdp_mline_index: 0 },
        { kind: "offer", sdp: "offer-sdp" },
      ]);
      if (command === "viewer_send_signal") sent.push(args.signal.kind);
    },
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => resolveIce);
  player.stop();
  resolveIce();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(peer.operations, ["offer", "ice"]);
  assert.deepEqual(sent, []);
  assert.equal(peer.closed, true);
});

test("peer failure releases media and ignores stats that arrive after failure", async (t) => {
  const peer = new Peer();
  const video = new Video();
  const reports = [];
  let resolveStats;
  peer.getStats = () => new Promise((resolve) => { resolveStats = resolve; });
  const player = new ViewerPlayer({ video, peerFactory: () => peer, streamFactory: () => new Stream(),
    invoke: async (command, args) => {
      if (command === "viewer_poll_signals") return batch(1);
      if (command === "viewer_report_playback") reports.push(args.report);
    },
  });
  t.after(() => player.stop());
  player.sync(session(1));
  await until(() => resolveStats);
  let stopped = false;
  peer.ontrack({ track: { id: "video", kind: "video", stop: () => { stopped = true; } } });
  peer.connectionState = "failed";
  peer.onconnectionstatechange();
  resolveStats(new Map());
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(peer.closed, true);
  assert.equal(stopped, true);
  assert.equal(video.srcObject, null);
  assert.deepEqual(reports.map((report) => report.connection_state), ["failed"]);
  assert.equal(player.context.state, "failed");
});

test("WebRTC stats report decoded video, received audio samples and the selected relay path", () => {
  const stats = new Map([
    ["video", { type: "inbound-rtp", kind: "video", packetsReceived: 12, framesDecoded: 7, bytesReceived: 800 }],
    ["audio", { type: "inbound-rtp", mediaType: "audio", packetsReceived: 20, totalSamplesReceived: 19200, bytesReceived: 300, packetsLost: 1 }],
    ["transport", { type: "transport", selectedCandidatePairId: "pair" }],
    ["pair", { type: "candidate-pair", localCandidateId: "local", remoteCandidateId: "remote", currentRoundTripTime: 0.01, bytesReceived: 1300 }],
    ["local", { candidateType: "host" }], ["remote", { candidateType: "relay" }],
  ]);
  const result = collectReport(stats, new Peer(), new Video(), { state: "playing", error: null, dataReady: true });
  assert.equal(result.video_frames, 7);
  assert.equal(result.audio_samples, 19200);
  assert.equal(result.ice_path_kind, "relay");
  assert.equal(result.rtt_ms, 10);
  assert.equal(result.bytes_received, 1300);
  assert.equal(result.packets_lost, 1);
});
