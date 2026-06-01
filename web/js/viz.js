/**
 * Canvas particle field driven by telemetry.
 */

export class SymphonyViz {
  #canvas;
  #ctx;
  #particles = [];
  #w = 0;
  #h = 0;

  constructor(canvas) {
    this.#canvas = canvas;
    this.#ctx = canvas.getContext("2d");
    window.addEventListener("resize", () => this.#resize());
    this.#resize();
    for (let i = 0; i < 120; i++) {
      this.#particles.push(this.#spawn());
    }
  }

  #resize() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    this.#w = window.innerWidth;
    this.#h = window.innerHeight;
    this.#canvas.width = this.#w * dpr;
    this.#canvas.height = this.#h * dpr;
    this.#canvas.style.width = `${this.#w}px`;
    this.#canvas.style.height = `${this.#h}px`;
    this.#ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  #spawn() {
    return {
      x: Math.random() * this.#w,
      y: Math.random() * this.#h,
      vx: (Math.random() - 0.5) * 0.6,
      vy: (Math.random() - 0.5) * 0.6,
      r: 1 + Math.random() * 2.5,
      hue: 220 + Math.random() * 80,
    };
  }

  /** @param {{ request_rate: number, latency_p99_ms: number, error_rate: number, anomaly_score: number, chaos: boolean }} frame */
  tick(frame) {
    const energy = Math.min(frame.request_rate / 2000, 1);
    const stress = Math.min(frame.latency_p99_ms / 300, 1);
    const err = Math.min(frame.error_rate * 20, 1);
    const anomaly = Math.min(frame.anomaly_score / 5, 1);

    const grd = this.#ctx.createRadialGradient(
      this.#w * 0.5,
      this.#h * 0.45,
      0,
      this.#w * 0.5,
      this.#h * 0.5,
      Math.max(this.#w, this.#h) * 0.65
    );
    grd.addColorStop(0, `hsla(${260 - stress * 80}, 80%, ${12 + energy * 8}%, 1)`);
    grd.addColorStop(1, "#06060c");
    this.#ctx.fillStyle = grd;
    this.#ctx.fillRect(0, 0, this.#w, this.#h);

    const speed = 0.4 + energy * 2.5 + stress * 2;
    for (const p of this.#particles) {
      p.vx += (Math.random() - 0.5) * 0.04 * (1 + anomaly);
      p.vy += (Math.random() - 0.5) * 0.04 * (1 + anomaly);
      const dir = frame.chaos ? 4 : 1;
      p.x += p.vx * speed * dir;
      p.y += p.vy * speed * dir;
      if (p.x < 0 || p.x > this.#w) p.vx *= -1;
      if (p.y < 0 || p.y > this.#h) p.vy *= -1;

      const alpha = 0.25 + energy * 0.45 + err * 0.3;
      this.#ctx.beginPath();
      this.#ctx.fillStyle = `hsla(${p.hue - stress * 60 + anomaly * 40}, 90%, ${55 + energy * 20}%, ${alpha})`;
      this.#ctx.arc(p.x, p.y, p.r * (1 + anomaly), 0, Math.PI * 2);
      this.#ctx.fill();
    }

    if (frame.chaos) {
      this.#ctx.strokeStyle = "rgba(255, 107, 74, 0.55)";
      this.#ctx.lineWidth = 2;
      this.#ctx.beginPath();
      this.#ctx.arc(this.#w / 2, this.#h / 2, 40 + stress * 80, 0, Math.PI * 2);
      this.#ctx.stroke();
    }
  }
}
