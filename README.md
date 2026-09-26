<!-- badges:start -->
[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22983057.svg)](https://doi.org/10.5281/zenodo.22983057)
[![ORCID iD](https://img.shields.io/badge/ORCID-0009--0003--3986--6039-green)](https://orcid.org/0009-0003-3986-6039)
<!-- badges:end -->
<img width="704" height="397" alt="bio-binaries-header" src="https://github.com/user-attachments/assets/b99de88c-760d-414c-bb9f-651749f2b929" />

Bio-binaries v0.2.1

**33 bio-inspired system utilities** — Binary protocol orchestration, quantum-space simulation, machine-brain inference.

A modular ecosystem where each command represents a biological principle: viral infection, plasmid injection, neural synchronization, resonance fields, homeostasis. Pure Rust, 100% binary protocol.

---

## Architecture

### Layer 1: Bio-Evolution
- `viral-infect` — infection cascade, signal propagation
- `hox-diff` — differential activation, gene expression
- `plasmid-dream` — dream sequence generation
- `plasmid-inject` — vector delivery system
- `mutation-sentinel` — file mutation monitoring

### Layer 2: Quantum-Space
- `telepathy-sync` — entanglement synchronization
- `telepathy-entangle` — quantum correlation
- `eqm-pulse` — electromagnetic pulse shaping
- `eqm-methy` — methylation state encoding
- `grid-warp` — spatial distortion fields
- `path-resonance` — resonance path finding
- `aether-fabric` — topological field mapping
- `aether-excite` — quantum field excitation

### Layer 3: Machine-Brain
- `borg-cube` — collective consciousness node
- `brain-synapse` — synaptic connection modeling
- `brain-connectome` — connectome reconstruction
- `collective-sync` — hive mind synchronization
- `nexus-logic` — logic gate fusion
- `microscope-mem` — memory layer interface
- `ribosome-synth` — protein synthesis (binary mitosis)
- `vagus-nerve` — internal organ sensing (CPU/RAM → WaveField)

### Layer 4: Resonance & Homeostasis
- `wave-encoder` — wave pattern encoding (432 Hz base)
- `wave-sculptor` — wave shaping filter (DSP)
- `wave-field` — self-organizing interference field
- `iron-resonate` — iron-core resonance profiling
- `magneto-acoustic` — acoustic-magnetic coupling (code → audio)
- `magneto-geo` — geomagnetic field interaction (code heatmap)
- `mycelium-spread` — fungal network propagation (filesystem mapping)
- `homeostasis` — system equilibrium maintenance

### Control & Audio
- `omega-master` — Queen orchestrator (central server)
- `omega-point` — singularity convergence detector
- `wave-cryo-tx` — acoustic cryo-transmitter (BFSK modem)
- `wave-cryo-rx` — acoustic cryo-receiver (BFSK demodulation)

### All Binaries (33)

| Binary | Layer | Description | Status |
|--------|-------|-------------|--------|
| `omega-master` | Control | Queen orchestrator — drone registration, task dispatch, BioMessage protocol | ✅ |
| `omega-point` | Control | Convergence detector — system coherence monitoring | ✅ |
| `viral-infect` | Bio-Evolution | Regex-based code transformation at scale | ✅ |
| `hox-diff` | Bio-Evolution | Project structure differentiator | ✅ |
| `plasmid-dream` | Bio-Evolution | Predictive error analyzer — build runner + trend analysis | ✅ |
| `plasmid-inject` | Bio-Evolution | Surgical file patching at line level | ✅ |
| `mutation-sentinel` | Bio-Evolution | File mutation watcher — auto-freeze on changes | ✅ |
| `telepathy-sync` | Quantum-Space | BLAKE3 delta directory synchronization | ✅ |
| `telepathy-entangle` | Quantum-Space | Inter-process shared state via temp files | ✅ |
| `eqm-pulse` | Quantum-Space | System health monitor + FFT frequency analysis | ✅ |
| `eqm-methy` | Quantum-Space | File consolidator — BLAKE3 integrity index with methylation rate | ✅ |
| `aether-excite` | Quantum-Space | Resource excitation monitor — per-region system load | ✅ |
| `aether-fabric` | Quantum-Space | System topology mapper — process/port/connection graph | ✅ |
| `borg-cube` | Machine-Brain | Parallel command replicator — exponential scaling | ✅ |
| `brain-synapse` | Machine-Brain | File co-change tracker — Hebbian git analysis | ✅ |
| `brain-connectome` | Machine-Brain | Dependency graph builder — static import analysis | ✅ |
| `collective-sync` | Machine-Brain | Multi-process state reconciliation — distributed consensus | ✅ |
| `nexus-logic` | Machine-Brain | Knowledge indexer — local full-text trigram search engine | ✅ |
| `ribosome-synth` | Machine-Brain | Code generator & self-replicator — binary mitosis engine | ✅ |
| `microscope-mem` | Machine-Brain | Memory layer interface (Microscope API compatibility) | ✅ |
| `vagus-nerve` | Machine-Brain | Internal organ sensing — CPU/RAM → WaveField | ✅ |
| `wave-encoder` | Resonance | Data→wave encoder — FFT-based file encoding (432 Hz base) | ✅ |
| `wave-sculptor` | Resonance | Frequency filter — digital signal processing on wave packets | ✅ |
| `wave-field` | Resonance | Self-organizing wave interference field | ✅ |
| `iron-resonate` | Resonance | Hardware resonance monitor — HW performance profiler | ✅ |
| `path-resonance` | Resonance | Hot-path detector — filesystem activity heatmap | ✅ |
| `grid-warp` | Resonance | Symlink/junction manager + latency measurement | ✅ |
| `magneto-geo` | Resonance | Error hotspot detector — code quality heatmap scanner | ✅ |
| `magneto-acoustic` | Resonance | Code health sonifier — error patterns to audio (WAV) | ✅ |
| `mycelium-spread` | Resonance | Recursive filesystem mapper — network graph builder | ✅ |
| `homeostasis` | Resonance | System equilibrium maintenance | ✅ |
| `wave-cryo-tx` | Audio | Acoustic CryoFrame transmitter — BFSK modulated WAV output | ✅ |
| `wave-cryo-rx` | Audio | Acoustic CryoFrame receiver — BFSK demodulation from WAV | ✅ |

---

## Protocol

**v2 BioMessage** (Primary — 100% binary)
- Header: 60 bytes (BLAKE3 auth, nonce replay protection)
- Payload: TLV-style field encoding
- 22 opcodes: JOIN, TASK, RESULT, HEARTBEAT, CLONE, GENOME, APOPTOSIS, FREEZE, THAW, CRISPR_PATCH, IMMUNE_ALERT, HOMEO_SYNC, MICRO_QUERY, and more
- Drones use `bio_client::DroneClient` over UDP
- Stateless, concurrent, replay-protected via NonceWindow

**v1 Echo-X** (Legacy — deprecated)

---

## Storage (100% Binary)

| Module | Format | Purpose |
|--------|--------|---------|
| wave_store | Custom TLV | Wave packet archive |
| cryo | bincode + zlib | Cryogenic snapshots (FFT spectral state) |
| microscope_mem | bincode index | Memory layer |
| eqm_methy | bincode | Methylation state |
| telepathy_entangle | bincode | Entanglement records |

No JSON in production storage. External APIs (Ollama, dream_loop) use JSON as bridge only.

---

## Build & Run

```bash
# Dev build (fast check)
cargo check

# Release build
cargo build --release

# Test single drone
./target/release/omega-master status

# Queen server (orchestrator)
./target/release/omega-master start --listen 127.0.0.1:8888

# List all available binaries
ls target/release/*.exe | grep -v deps | sed 's|target/release/||;s|\.exe$||'
```

---

## Key Components

| Module | Lines | Description |
|--------|-------|-------------|
| `bio_protocol.rs` | 553 | Binary protocol: encode/decode, TLV fields, CRC64, replay protection |
| `cryo.rs` | 867 | FFT-based spectral snapshots (CryoFrame), freeze/thaw engine |
| `acoustic.rs` | 675 | BFSK modem: Goertzel demodulation, hand-written WAV I/O |
| `auth.rs` | 253 | QueenKey (BLAKE3 keyed), DroneToken, BinaryIntegrity gate macro |
| `bio_client.rs` | 63 | UDP DroneClient: JOIN, heartbeat, result send |
| `system.rs` | 129 | sysinfo-based system snapshot (CPU, RAM, disk, processes) |
| `omega_master.rs` | 667 | Queen orchestrator: drone registry, task dispatch, CLI |

Project stats: ~17,300 lines of Rust, 92 source files, 33 binary targets.

---

## Dependencies (Minimal, Focused)

```toml
sysinfo = "0.30"           # System info
blake3 = "1.5"             # BLAKE3 hashing
clap = "4"                 # CLI parsing
serde = "1.0"              # Serialization
bincode = "1.3"            # Binary encoding
serde_json = "1.0"         # JSON (external APIs only)
tokio = "1"                # Async runtime
colored = "2"              # Terminal colors
memmap2 = "0.9"            # Memory mapping
chrono = "0.4"             # Timestamps
notify = "6"               # File watching
rustfft = "6"              # FFT spectral analysis
flate2 = "1.0"             # Zlib compression
qrcode = "0.14"            # QR code generation
reqwest = "0.13.2"         # HTTP (external API bridge)
```

---

## Project Structure

```
src/
├── lib.rs                  # Module declarations
├── commands/               # 33 command modules
│   ├── mod.rs
│   ├── omega_master.rs     # Queen orchestrator
│   ├── viral_infect.rs     # Code transformer
│   ├── telepathy_sync.rs   # Delta sync
│   ├── ...                 # 30 more command modules
├── bin/                    # 33 binary entry points
│   ├── omega_master.rs
│   ├── viral_infect.rs
│   └── ...                 # 31 more entry points
├── bio_protocol.rs         # v2 BioMessage format
├── bio_client.rs           # Drone client protocol
├── auth.rs                 # BLAKE3 authentication
├── cryo.rs                 # Cryogenic snapshots (FFT)
├── acoustic.rs             # BFSK modem
├── wave_store.rs           # Wave packet storage
├── system.rs               # System monitoring
├── wave_field.rs           # Wave field simulation
├── magneto.rs              # Magnetic field utilities
├── mitosis.rs              # Binary replication
├── leash.rs                # Metabolic token management
└── ...

Cargo.toml                  # Dependencies & metadata
Cargo.lock                  # Locked versions
```

---

## Security Mechanisms

1. **Binary Integrity Check** — BLAKE3 self-hash at startup; if the binary changed since last verified run, exits with code 77
2. **Queen Key Authentication** — Ed25519-style BLAKE3 keypair, drone session tokens (1-hour expiry)
3. **Nonce Replay Protection** — rolling window of seen nonces in BioMessage protocol
4. **CRC64 Checksums** — every BioMessage has a checksum covering header + payload
5. **Exponential Power Limit** — borg-cube caps at 2^N (typical: 2^4 = 16) to prevent DoS
6. **CRC-16 CCITT** — acoustic frames have CRC-16 error detection
7. **Constant-Time Comparison** — auth tag verification is timing-safe

---

## Author

**Máté Róbert (Silent)** — bio-binaries orchestration system

A research project exploring bio-inspired computing principles through a pure Rust ecosystem. Each module represents a biological metaphor: viral propagation, quantum entanglement, neural connectivity, resonance fields, homeostatic balance.

Not a toy. Deployment-ready code.

---

## Quick Start

```bash
# Check compilation
cargo check

# Run the Queen server
cargo run --release --bin omega-master -- status

# Try code transformation (dry run)
cargo run --release --bin viral-infect -- . --pattern "TODO" --replace "DONE" --dry-run

# Acoustic transmission test
cargo run --release --bin wave-cryo-tx -- test

# FFT spectral snapshot
cargo run --release --bin cryo-freeze
```

**Status**: 🎯 **Ready for deployment** — All 33 binaries compile and run, binary protocol verified.

Last verified: 2026-06-03 | Cargo check: 0 errors