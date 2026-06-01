# Symphony of the Stack

**Turn your infrastructure into music and motion.**

Request rate drives tempo. Latency shapes pitch. Errors add dissonance. Anomalies shift timbre. Chaos experiments hit like a drum.

```
  Metrics (demo / NATS / HTTP)  →  symphony-bridge  →  WebSocket  →  Browser
                                        │                              │
                                        └──────── static UI ───────────┘
                                              Web Audio + canvas
```

## Quick start

### Docker (recommended)

```bash
docker compose up -d
```

Open **http://localhost:8765** → click **Start audio**.

### Local (Rust 1.75+)

```bash
cargo run -p symphony-bridge --release
# → http://localhost:8765
```

On Windows, install [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) with the **C++** workload if `cargo build` reports a missing `link.exe`.

## Configuration

| Variable | Default | Description |
|----------|---------|-------------|
| `SYMPHONY_HTTP_ADDR` | `0.0.0.0:8765` | HTTP + WebSocket listen address |
| `SYMPHONY_WEB_DIR` | `web` | Static UI directory |
| `SYMPHONY_DEMO` | `true` | Synthetic metrics when no live publishers |
| `NATS_URL` | — | Subscribe to event subjects (e.g. Nexus Orchestrator) |
| `SYMPHONY_NATS_SUBJECTS` | `nexus.anomalies.>,nexus.chaos.>,nexus.feedback.>` | Comma-separated NATS subjects |
| `SYMPHONY_ANOMALY_URL` | — | Poll an HTTP anomaly API (e.g. `http://host:8090`) |
| `SYMPHONY_TICK_MS` | `100` | Frame broadcast interval (ms) |

## Sonification map

| Signal | Sound | Visual |
|--------|-------|--------|
| Request rate | Arpeggio tempo | Particle speed |
| p99 latency | Root pitch | Hue / stress |
| Error rate | Detuned layer | Particle glow |
| Anomaly score | Filter brightness & Q | Color shift |
| Chaos event | Square drum hit | Expanding ring |

## WebSocket protocol

Connect to `ws://localhost:8765/ws`. Each message is JSON:

```json
{
  "ts_ms": 1717200000123,
  "target": "production/api",
  "request_rate": 1240.5,
  "latency_p99_ms": 87.2,
  "error_rate": 0.004,
  "anomaly_score": 1.8,
  "chaos": false,
  "event": null
}
```

## Integrate with [Nexus Orchestrator](https://github.com/Pkkarthik12/Nexus-autonomous-infrastructure-orchestrator)

Point `NATS_URL` at your Nexus NATS bus and optionally `SYMPHONY_ANOMALY_URL` at the anomaly scorer. Publish chaos or anomaly events and hear your cluster react in real time.

## Repository layout

```
├── crates/symphony-bridge/   # Rust WebSocket + HTTP server
├── web/                      # Browser UI (Web Audio + canvas)
├── Dockerfile
├── docker-compose.yml
└── scripts/
```

## License

MIT — see [LICENSE](LICENSE).
