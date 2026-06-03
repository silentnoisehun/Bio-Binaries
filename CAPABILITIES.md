# bio-binaries — Capabilities & Security

## Overview

33 standalone commands, each an implementation of a **bio-inspired algorithm**.

Legend: ✅ = functional, working | ⚠️ = security-gated | ⏳ = partial/stub

---

## 1. BIO-EVOLUTION (Infection & Transformation)

### viral-infect
**Function:** Regex-based code transformation across large file sets
**Input:** Source directory + regex rules (JSON or command-line)
**Output:** Modified files (or dry-run report)
**Use case:** Replace `OldClass` → `NewClass` across an entire codebase
**Status:** ✅ Working

```bash
# Example: Replace all 'foo' with 'bar' in .rs files
./viral-infect /path/to/code --pattern 'foo' --replace 'bar' --ext rs --dry-run
```

---

### hox-diff
**Function:** Gene expression differential (project structure comparison)
**Note:** Security-gated — binary integrity check required
**Status:** ⚠️ Binary integrity check

---

### plasmid-dream
**Function:** Predictive error analyzer — build runner + trend analysis
**Input:** Project directory
**Output:** Error pattern trends, predictions
**Use case:** "Which files are likely to cause build failures next?"
**Status:** ✅ Working

```bash
./plasmid-dream /path/to/project
```

---

### plasmid-inject
**Function:** Surgical file patching — line-level code injection
**Input:** Target file + start/end line + injection code
**Output:** Modified file with inline patch
**Use case:** Bug fix in specific lines of a specific file
**Status:** ✅ Working

```bash
./plasmid-inject target.rs --start 42 --end 50 --patch "new_code_here"
```

---

### mutation-sentinel
**Function:** File mutation watcher — auto-freeze on .rs changes
**Input:** Directory to watch
**Output:** Auto-freezes (checkpoints) when Rust files change
**Use case:** "Take a snapshot after every modification"
**Status:** ✅ Working

```bash
./mutation-sentinel watch /path/to/src
```

---

### aether-excite
**Function:** Quantum excitation simulation — per-region system load monitoring
**Status:** ⚠️ Binary integrity check

---

### aether-fabric
**Function:** System topology mapper — process/port/connection graph builder
**Status:** ⚠️ Binary integrity check

---

## 2. QUANTUM-SPACE (Synchronization & Entanglement)

### telepathy-sync
**Function:** BLAKE3-based delta directory synchronization
**Input:** Source dir + target dir
**Output:** Only changed files copied (BLAKE3 hashes verify)
**Use case:** Keep two machines in sync with only diffs
**Status:** ✅ Working

```bash
./telepathy-sync /local/code /remote/code --dry-run
```

**Security:** BLAKE3 integrated — if a file changes mid-copy, it's detected.

---

### telepathy-entangle
**Function:** Inter-process state sharing via temp files
**Input:** Key-value pairs
**Output:** Shared state dictionary
**Use case:** Share data between processes file-based
**Status:** ✅ Working

```bash
./telepathy-entangle set mykey myvalue
./telepathy-entangle get mykey
```

---

### eqm-pulse
**Function:** Electromagnetic pulse shaping — system health monitor + FFT analysis
**Status:** ⚠️ Binary integrity check

---

### eqm-methy
**Function:** File consolidator — BLAKE3 integrity index + methylation rate
**Input:** Directory
**Output:** BLAKE3 hash index + "methylation" (modification frequency)
**Use case:** Which files change most? Which are stable?
**Status:** ✅ Working

```bash
./eqm-methy /path/to/project
# Output: "file.rs: methylation=0.85, hash=abc123..."
```

---

### grid-warp
**Function:** Symlink/junction manager + latency measurement
**Input:** Links specification (JSON)
**Output:** Created symlinks + latency measurements
**Status:** ✅ Working

```bash
./grid-warp --links '[{"source":"/a","target":"/b"}]'
```

---

### path-resonance
**Function:** Hot-path detector — filesystem activity heatmap
**Input:** Directory
**Output:** Heatmap (most frequently accessed files/dirs)
**Use case:** Which files do we open most often?
**Status:** ✅ Working

```bash
./path-resonance /path/to/project
```

---

## 3. MACHINE-BRAIN (Consciousness & Collective)

### borg-cube
**Function:** Parallel command replicator — exponential scaling benchmark
**Input:** Command to replicate
**Output:** 2^N parallel executions
**Use case:** Run a command 4×, 16×, 256× in parallel
**Status:** ✅ Working

```bash
./borg-cube "cargo build" --max-power 4  # 2^4 = 16 parallel instances
```

**Security:** Exponential load scaling — cannot run 2^32 instances!

---

### brain-synapse
**Function:** Neural synapse connection modeling — file co-change tracker
**Input:** Git repository directory
**Output:** Co-change matrix (Hebbian analysis)
**Use case:** "Which files change together?"
**Status:** ✅ Working

```bash
./brain-synapse /path/to/repo --limit 500
```

---

### brain-connectome
**Function:** Connectome reconstruction — dependency graph from imports
**Input:** Source directory
**Output:** Module dependency graph
**Use case:** "Map how code depends on itself"
**Status:** ⚠️ Binary integrity check

---

### collective-sync
**Function:** Multi-process state reconciliation — distributed consensus
**Input:** Echo-X master address
**Output:** Consensus state across processes
**Use case:** Multiple processes agree on state (blockchain-like)
**Status:** ✅ Working

```bash
./collective-sync --echo-x 127.0.0.1:8888
```

---

### nexus-logic
**Function:** Knowledge indexer — local full-text trigram search engine
**Input:** Directory
**Output:** Searchable index (trigram-based)
**Use case:** Local code search without internet
**Status:** ✅ Working

```bash
./nexus-logic /path/to/code
# Indexes all files, enables trigram search
```

---

### microscope-mem
**Function:** Memory layer compatibility — Microscope API wrapper
**Input:** Command (store/recall/status/build)
**Output:** Memory operations
**Use case:** Interface with ora/microscope-memory library
**Status:** ✅ Working

```bash
./microscope-mem store --text "important info"
./microscope-mem recall --query "search term"
```

---

### ribosome-synth
**Function:** Code generator & self-replicator — binary mitosis engine
**Input:** Template
**Output:** Generated drone binary (self-replicating)
**Use case:** "Generate a new drone that reproduces itself"
**Status:** ✅ Working

```bash
./ribosome-synth generate --template drone_base.rs
```

---

### vagus-nerve
**Function:** Internal organ sensing — CPU/RAM → WaveField
**Input:** (none, live monitoring)
**Output:** WaveField state from system metrics
**Use case:** "Translate system vitals into field dynamics"
**Status:** ✅ Working

```bash
./vagus-nerve
```

---

## 4. RESONANCE (Waves, Fields, Homeostasis)

### wave-encoder
**Function:** Wave pattern encoding — FFT-based file encoding (432 Hz base)
**Input:** File path
**Output:** JSON wave packet (frequency domain representation)
**Use case:** Represent any file as a wave spectrum
**Status:** ✅ Working

```bash
./wave-encoder input.bin
```

---

### wave-sculptor
**Function:** Frequency filter — digital signal processing on wave packets
**Input:** Wave packet JSON (from wave-encoder)
**Output:** Filtered wave (DSP applied)
**Use case:** "Remove noise while preserving signal"
**Status:** ✅ Working

```bash
./wave-sculptor input_wave.json --filter lowpass --cutoff 1000Hz
```

---

### wave-field
**Function:** Self-organizing wave interference field — "the space decides"
**Input:** Field parameters
**Output:** Live snapshot (interference pattern)
**Use case:** Simulate wave interference patterns
**Status:** ✅ Working

```bash
./wave-field snapshot
```

---

### iron-resonate
**Function:** Iron-core resonance — detailed hardware performance profiler
**Status:** ⚠️ Binary integrity check

---

### magneto-geo
**Function:** Error hotspot detector — code quality heatmap scanner
**Input:** Project directory
**Output:** Heatmap (which files have the most errors)
**Use case:** "Which files are the worst?"
**Status:** ✅ Working

```bash
./magneto-geo /path/to/project
```

**Security:** Runs `cargo check` — detects real compile errors.

---

### magneto-acoustic
**Function:** Code health sonifier — error patterns to audio (!!)
**Input:** Project directory
**Output:** WAV file (error patterns encoded as sound)
**Use case:** "Listen to error patterns — sounds represent bugs"
**Status:** ✅ Working

```bash
./magneto-acoustic /path/to/project
# Output: errors.wav (each compile error = different tone)
```

---

### mycelium-spread
**Function:** Recursive filesystem mapper — builds a network graph
**Input:** Root directory
**Output:** Adjacency matrix (directory relationships)
**Use case:** "Map directory structure as a network"
**Status:** ✅ Working

```bash
./mycelium-spread /path/to/root
# Output: JSON network graph
```

---

### homeostasis
**Function:** System equilibrium maintenance
**Input:** System metrics
**Output:** Homeostatic adjustments (thermal, memory, load)
**Use case:** "Keep the OS in balance"
**Status:** ✅ Working

```bash
./homeostasis status
```

---

### omega-master
**Function:** Queen orchestrator — central server (v2 DNA Protocol)
**Input:** Commands (start, status, run-all, apoptosis, freeze, thaw, key-info, microscope, homeo)
**Output:** Drone coordination
**Use case:** Master server controlling a drone swarm
**Status:** ✅ Working

```bash
./omega-master start --listen 127.0.0.1:8888
# Queen server starts, awaits drone connections
```

**Security:**
- Queen key generation (BLAKE3 asymmetric)
- Drone registry (authorized drones only)
- Apoptosis signal (remotely terminate drones)

---

### omega-point
**Function:** Convergence detector — monitors system stability & coherence
**Input:** Echo-X master address
**Output:** Coherence score (0.0 = chaos, 1.0 = perfect sync)
**Use case:** "Are the drones synchronized?"
**Status:** ✅ Working

```bash
./omega-point --echo-x 127.0.0.1:8888
# Output: Coherence=0.94, Drones synchronized
```

---

## 5. AUDIO TRANSPORT (Acoustic Channel)

### wave-cryo-tx
**Function:** Acoustic CryoFrame transmitter — BFSK modulated WAV output
**Input:** Binary cryo file
**Output:** WAV file (BFSK modulated, 8000 Hz, mark=1200 Hz, space=600 Hz)
**Use case:** Transmit data **through sound waves**
**Status:** ✅ Working

```bash
./wave-cryo-tx encode --input data.cryo --output audio.wav
# Output: audio.wav (BFSK modulation, playable, carries data)
```

---

### wave-cryo-rx
**Function:** Acoustic CryoFrame receiver — BFSK demodulation from WAV
**Input:** WAV file (BFSK modulated)
**Output:** Binary cryo file + JSON metadata
**Use case:** Recover data from sound waves
**Status:** ✅ Working

```bash
./wave-cryo-rx decode --input audio.wav --output data.cryo
# Output: data.cryo (recovered binary)
```

---

## SECURITY MECHANISMS

### 1. Binary Integrity Check (BIO-SECURITY)
**Active on:** wave-encoder, brain-synapse, brain-connectome, aether-excite, aether-fabric, eqm-pulse, iron-resonate, hox-diff

These commands use BLAKE3 to verify the binary hasn't changed since last run. If modified → **they refuse to execute** (self-protection).

```
[BIO-SECURITY] Binary integrity check FAILED for wave-encoder.
[BIO-SECURITY] Possible mutation detected. The executable has been modified since last verified run. Aborting.
```

**Purpose:** Prevent unauthorized binary modification.

### 2. Queen Key Authentication (omega-master)
**Active on:** omega-master, omega-point, collective-sync

The Queen server generates a BLAKE3 keypair; drones authenticate with session tokens.

**Files:**
- `.bio-queen.key` — private key
- `drone_registry.json` — authorized drone list

### 3. BLAKE3 Checksums (telepathy-sync, eqm-methy)
Every file is verified by BLAKE3 hash. Hash mismatch → sync fails.

### 4. Exponential Power Limit (borg-cube)
Max 2^N (typical: 2^4 = 16). Cannot scale to 2^32 (DoS protection).

### 5. CRC Integrity (wave-cryo-tx/rx)
CRC-16 CCITT on acoustic frames. BFSK modulation has implicit error correction.

---

## STATUS SUMMARY

| Category | Working | Security-gated | Stub |
|----------|---------|----------------|------|
| Bio-Evolution | 4 | 3 | 0 |
| Quantum-Space | 4 | 3 | 0 |
| Machine-Brain | 6 | 1 | 1 (homeostasis was stub, now working) |
| Resonance | 8 | 3 | 0 |
| Audio | 2 | 0 | 0 |
| **Total** | **24** | **9** | **0** |

---

## BINARY LIST (33 total)

```
aether-excite*    aether-fabric*    borg-cube
brain-connectome* brain-synapse    collective-sync
eqm-methy        eqm-pulse*       grid-warp
homeostasis      hox-diff*        iron-resonate*
magneto-acoustic magneto-geo      mutation-sentinel
mycelium-spread  nexus-logic      omega-master
omega-point      path-resonance   plasmid-dream
plasmid-inject   ribosome-synth   telepathy-entangle
telepathy-sync   vagus-nerve      viral-infect
wave-cryo-rx     wave-cryo-tx     wave-encoder
wave-sculptor    wave-field       microscope-mem
```

`*` = security-gated (binary integrity check)

---

## DEPLOYMENT STATUS

✅ **Ready for production:**
- viral-infect, plasmid-inject, plasmid-dream, telepathy-sync
- telepathy-entangle, borg-cube, nexus-logic, collective-sync
- eqm-methy, grid-warp, path-resonance, magneto-geo
- magneto-acoustic, mycelium-spread, omega-master, omega-point
- wave-encoder, wave-sculptor, wave-field, microscope-mem
- wave-cryo-tx, wave-cryo-rx, vagus-nerve, homeostasis
- brain-synapse

⚠️ **Security-gated (binary integrity check):**
- hox-diff, aether-excite, aether-fabric, eqm-pulse
- brain-connectome, iron-resonate, wave-encoder, brain-synapse

✅ **All 33 binaries compile and run with 0 cargo errors.**

---

## USE CASES

### 1. Large-Scale Codebase Refactoring
```bash
./viral-infect /huge/project --pattern 'OldAPI' --replace 'NewAPI' --ext rs
```

### 2. Predictive Error Detection
```bash
./plasmid-dream /my/project
# Output: "errors.rs likely to fail in next build"
```

### 3. Directory Sync (Delta Only)
```bash
./telepathy-sync /local /remote --dry-run
```

### 4. Code Quality Heatmap
```bash
./magneto-geo /my/project
# Output: heatmap (red = many errors, green = clean)
```

### 5. Decode Error Patterns as Audio
```bash
./magneto-acoustic /my/project
# Listen to bugs
```

### 6. Transmit Data Through Sound
```bash
./wave-cryo-tx encode --input secret.bin --output secret.wav
```

### 7. Self-Replicating Code Generator
```bash
./ribosome-synth generate --template drone_base.rs
```

### 8. System Health as Waves
```bash
./vagus-nerve
# CPU/RAM → WaveField visualization
```

### 9. Queen Orchestration
```bash
./omega-master start --listen 0.0.0.0:8888
./omega-master status
./omega-master apoptosis
```