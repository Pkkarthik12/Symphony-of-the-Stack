/**
 * Web Audio sonification engine for SymphonyFrame telemetry.
 */

export class SymphonyAudio {
  #ctx = null;
  #master = null;
  #carrier = null;
  #detune = null;
  #filter = null;
  #arpTimer = null;
  #sensitivity = 1;
  #running = false;

  async start() {
    if (this.#ctx) return;
    const ctx = new AudioContext();
    const master = ctx.createGain();
    master.gain.value = 0.35;
    master.connect(ctx.destination);

    const carrier = ctx.createOscillator();
    carrier.type = "sawtooth";
    carrier.frequency.value = 196;

    const detune = ctx.createOscillator();
    detune.type = "triangle";
    detune.frequency.value = 196.5;

    const filter = ctx.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = 1200;
    filter.Q.value = 4;

    const mix = ctx.createGain();
    mix.gain.value = 0.22;

    carrier.connect(filter);
    detune.connect(filter);
    filter.connect(mix);
    mix.connect(master);

    carrier.start();
    detune.start();

    this.#ctx = ctx;
    this.#master = master;
    this.#carrier = carrier;
    this.#detune = detune;
    this.#filter = filter;
    this.#running = true;
  }

  stop() {
    this.#running = false;
    if (this.#arpTimer) {
      clearInterval(this.#arpTimer);
      this.#arpTimer = null;
    }
    if (this.#ctx) {
      this.#ctx.close();
      this.#ctx = null;
    }
  }

  get running() {
    return this.#running;
  }

  setVolume(v) {
    if (this.#master) this.#master.gain.value = v;
  }

  setSensitivity(v) {
    this.#sensitivity = v;
  }

  /** @param {{ latency_p99_ms: number, error_rate: number, request_rate: number, anomaly_score: number }} frame */
  update(frame) {
    if (!this.#ctx || !this.#running) return;
    const s = this.#sensitivity;
    const baseHz = 130 + Math.min(frame.latency_p99_ms * 1.2 * s, 520);
    const detuneHz = baseHz + frame.error_rate * 800 * s;
    const tempo = 40 + Math.min(frame.request_rate / 25, 200);

    this.#carrier.frequency.setTargetAtTime(baseHz, this.#ctx.currentTime, 0.08);
    this.#detune.frequency.setTargetAtTime(detuneHz, this.#ctx.currentTime, 0.08);
    this.#filter.frequency.setTargetAtTime(
      400 + (5 - Math.min(frame.anomaly_score, 5)) * 400 * s,
      this.#ctx.currentTime,
      0.12
    );
    this.#filter.Q.setTargetAtTime(1 + frame.anomaly_score * s, this.#ctx.currentTime, 0.12);

    this.#scheduleArp(tempo, baseHz, frame.anomaly_score);
  }

  chaosStrike() {
    if (!this.#ctx) return;
    const t = this.#ctx.currentTime;
    const osc = this.#ctx.createOscillator();
    const gain = this.#ctx.createGain();
    osc.type = "square";
    osc.frequency.setValueAtTime(80, t);
    osc.frequency.exponentialRampToValueAtTime(40, t + 0.25);
    gain.gain.setValueAtTime(0.5, t);
    gain.gain.exponentialRampToValueAtTime(0.001, t + 0.35);
    osc.connect(gain);
    gain.connect(this.#master);
    osc.start(t);
    osc.stop(t + 0.4);
  }

  #scheduleArp(tempoBpm, rootHz, anomaly) {
    const intervalMs = Math.max(60, 60000 / tempoBpm / 2);
    if (this.#arpTimer) return;
    const scale = [0, 3, 5, 7, 10, 12];
    let i = 0;
    this.#arpTimer = setInterval(() => {
      if (!this.#ctx || !this.#running) return;
      const semi = scale[i % scale.length] + (anomaly > 3 ? 1 : 0);
      i += 1;
      this.#pluck(rootHz * 2 ** (semi / 12));
    }, intervalMs);
  }

  #pluck(freq) {
    const t = this.#ctx.currentTime;
    const osc = this.#ctx.createOscillator();
    const g = this.#ctx.createGain();
    osc.type = "sine";
    osc.frequency.value = freq;
    g.gain.setValueAtTime(0.12, t);
    g.gain.exponentialRampToValueAtTime(0.001, t + 0.18);
    osc.connect(g);
    g.connect(this.#master);
    osc.start(t);
    osc.stop(t + 0.2);
  }
}
