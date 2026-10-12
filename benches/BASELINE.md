# Benchmark Baseline — KynexComp

Measured on Linux x86_64 (Release profile, Criterion 0.5.1).
Date: 2026-10-12
Milestone: Phase 2F — Performance profiling, vblank pacing verification, and Phase 2 closure gate.

## Purpose and Regression Policy (Rule 06)

Per Rule 06:
> A change that regresses a benchmark by >10% needs justification in the PR/commit.

All benchmarks measure pure logic in the functional core and imperative shell state machines with allocation-free or bounded structures.

---

## 1. `kx-config` (Configuration Parsing & Validation)

Target: `crates/kx-config/benches/config_bench.rs`

| Benchmark Target | Mean Latency | 95% Confidence Interval | Notes |
|:---|:---|:---|:---|
| `config_parsing/minimal_config` | **443.69 ns** | [439.62 ns, 447.81 ns] | Minimal TOML parsing and defaults resolution |
| `config_parsing/full_config` | **2.0890 µs** | [2.0441 µs, 2.1456 µs] | Full section deserialization, validation, limits check |

### Budget Assessment
- Main thread budget: $\le 1$ ms.
- Actual parse duration: $\approx 2.1$ µs ($< 0.21\%$ of main thread work slice).
- Conforms to Rule 06 main-loop responsiveness constraints.

---

## 2. `kx-input` (Binding Matcher & Shortcut Dispatch)

Target: `crates/kx-input/benches/input_bench.rs`

| Benchmark Target | Mean Latency | 95% Confidence Interval | Notes |
|:---|:---|:---|:---|
| `input_binding_matching/default_emergency_exit_match` | **793.83 ps** | [792.70 ps, 795.05 ps] | Sub-nanosecond Ctrl+Shift+Esc emergency exit evaluation |
| `input_binding_matching/default_non_matching_key` | **2.7104 ns** | [2.6844 ns, 2.7359 ns] | Standard non-shortcut key event bypass |
| `input_binding_matching/thirty_bindings_hit_last` | **13.566 ns** | [13.545 ns, 13.590 ns] | Linear scan over 30 configured key bindings (worst-case match) |

### Budget Assessment
- Input handling latency budget: under one frame interval (16.6 ms at 60 Hz).
- Actual binding lookup: $\le 14$ ns per event.
- Zero allocation during lookup, satisfying Rule 06 hot path rules.

---

## 3. `kx-shell-core` (Frame Clock & Monotonic VBlank Pacing)

Target: `crates/kx-shell-core/benches/frame_pacing_bench.rs`

| Benchmark Target | Mean Latency | 95% Confidence Interval | Notes |
|:---|:---|:---|:---|
| `frame_clock_pacing/full_frame_cycle_transition` | **8.7326 ns** | [8.7036 ns, 8.7617 ns] | Complete cycle: `schedule_frame` $\to$ `on_render_begin` $\to$ `on_render_submitted` $\to$ `on_vblank` |
| `frame_clock_pacing/percentile_and_timing_retrieval` | **553.62 ps** | [552.59 ps, 554.71 ps] | O(1) stack ring-buffer p99 and last render timing query |

### Budget Assessment
- Rule 00 budget: frame production p99 CPU time $\le 4$ ms per frame at 60 Hz.
- Actual frame clock state machine and telemetry overhead: $< 9$ ns per frame ($< 0.00025\%$ of the 4 ms budget).
- Allocation-free ring-buffer (`[Duration; 64]`) on stack, satisfying Rule 00 and Rule 06.
