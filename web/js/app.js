import { SymphonyAudio } from "./audio.js";
import { SymphonyViz } from "./viz.js";

/** @typedef {Object} SymphonyFrame
 *  @property {number} ts_ms
 *  @property {string} target
 *  @property {number} request_rate
 *  @property {number} latency_p99_ms
 *  @property {number} error_rate
 *  @property {number} anomaly_score
 *  @property {boolean} chaos
 *  @property {string} [event]
 */

const $ = (id) => document.getElementById(id);

const statusEl = $("connection-status");
const audio = new SymphonyAudio();
const viz = new SymphonyViz($("viz"));

let lastChaos = false;

function wsUrl() {
  const proto = location.protocol === "https:" ? "wss:" : "ws:";
  return `${proto}//${location.host}/ws`;
}

function formatRate(n) {
  return n >= 1000 ? `${(n / 1000).toFixed(1)}k` : Math.round(n).toString();
}

/** @param {SymphonyFrame} frame */
function renderHud(frame) {
  $("m-target").textContent = frame.target;
  $("m-rate").textContent = formatRate(frame.request_rate);
  $("m-latency").textContent = frame.latency_p99_ms.toFixed(0);
  $("m-errors").textContent = `${(frame.error_rate * 100).toFixed(2)}%`;
  $("m-anomaly").textContent = frame.anomaly_score.toFixed(2);
  $("m-event").textContent = frame.event ? `⚡ ${frame.event}` : "";
}

/** @param {SymphonyFrame} frame */
function onFrame(frame) {
  renderHud(frame);
  viz.tick(frame);
  if (audio.running) {
    audio.update(frame);
    if (frame.chaos && !lastChaos) audio.chaosStrike();
  }
  lastChaos = frame.chaos;
}

function connect() {
  const ws = new WebSocket(wsUrl());

  ws.addEventListener("open", () => {
    statusEl.textContent = "● Live";
    statusEl.classList.add("live");
    statusEl.classList.remove("error");
  });

  ws.addEventListener("message", (ev) => {
    try {
      const frame = JSON.parse(ev.data);
      onFrame(frame);
    } catch {
      /* ignore */
    }
  });

  ws.addEventListener("close", () => {
    statusEl.textContent = "Reconnecting…";
    statusEl.classList.remove("live");
    setTimeout(connect, 1500);
  });

  ws.addEventListener("error", () => {
    statusEl.textContent = "Connection error";
    statusEl.classList.add("error");
  });
}

$("btn-audio").addEventListener("click", async () => {
  const btn = $("btn-audio");
  if (!audio.running) {
    await audio.start();
    btn.textContent = "Audio on";
    btn.classList.add("active");
  } else {
    audio.stop();
    btn.textContent = "Start audio";
    btn.classList.remove("active");
  }
});

$("volume").addEventListener("input", (e) => {
  audio.setVolume(Number(e.target.value));
});

$("sensitivity").addEventListener("input", (e) => {
  audio.setSensitivity(Number(e.target.value));
});

connect();
