# WASI-FPGA Benchmarking Guide

Operational manual for collecting all quantitative data for the FPL 2026 paper.

---

## 1. Overview

This guide covers the complete procedure for running **8 experiments (A–H)** that evaluate the
WASI-FPGA neuromorphic inference platform on the Xilinx ZCU104. It is designed as a
self-contained manual: every command can be copy-pasted over SSH.

| Experiment | One-Line Description |
|------------|---------------------|
| **A** | WASM vs OCI container startup, memory, and image size |
| **B** | WASI extension overhead (per-function microbenchmarks) |
| **C** | FPGA kernel profiling via OpenCL hardware events |
| **D** | End-to-end pipeline latency (WASM → WASI → OpenCL → FPGA → WASM) |
| **E** | Energy / power measurement (WASM+FPGA vs native C++) |
| **F** | Scalability (multi-pod concurrent FPGA access) |
| **G** | WASM runtime overhead (WASM runtime-layer cost vs native C++ baseline) |
| **H** | 24-hour stability test |

### Benchmark Tracks

- **Track 1 — Overhead Isolation:** Synthetic repeated input (current `make_synthetic_digit_1()`). Measures runtime overhead, not neural network quality. Valid for experiments A–D, F, G.
- **Track 2 — Representative Inference:** Real MNIST test samples (requires `test_images.bin` or equivalent). Required for correctness claims and any NeuroBench-style reporting. Valid for experiment E (accuracy verification).

### Representative Inference Validation

The current benchmark module uses a synthetic digit '1' pattern repeated for all iterations. This is sufficient for latency/overhead isolation but NOT for inference quality claims. For publication, run at least one correctness-oriented test with representative MNIST inputs (e.g., all 10 digit classes) and verify predicted class matches expected. If citing NeuroBench [S23], must provide accuracy/correctness metrics — not just throughput. The existing `test_bcpnn_infer.wasm` can serve as the correctness validator (it checks prediction output).

**Estimated time:** ~2 days for experiments A–G, plus 24 hours dedicated for H.

**Audience:** artifact evaluators and FPGA engineers running the ZCU104 setup.

---

## 2. Prerequisites

| Requirement | Details |
|-------------|---------|
| **Hardware** | Xilinx ZCU104 with XRT 2023.1, ZOCL driver loaded |
| **PAC** | `mnist_float` installed (`sudo xlnx-config -a mnist_float`) |
| **WasmEdge** | v0.13.5 with WASI-FPGA plugin in `WASMEDGE_PLUGIN_PATH` |
| **Rust** | 1.75+ with `wasm32-wasip1` target (build host only) |
| **xclbin** | `BCPNN_infer_float.xclbin` on the board |
| **Weights** | `alvis_fullmnist_32x128_64x64_eps-4.bin` on the board |
| **CMA** | `cma=512M` in kernel cmdline (for DMA buffers) |

### Benchmark activation

- **`WASI_FPGA_BENCH=1`** env var activates `[BENCH]` output on stderr.
  The check is in `fpga_state.rs:64-66`: `std::env::var("WASI_FPGA_BENCH").map(|v| v == "1")`.
- **OpenCL queue profiling** is always enabled — `CL_QUEUE_PROFILING_ENABLE` is hardcoded in
  the command queue creation (`fpga_state.rs:173-177`). However, profiling events are only
  captured and emitted in `[BENCH]` output when `WASI_FPGA_BENCH=1` is set. Without that env
  var, the hardware timers are enabled in the queue but not surfaced in the logs.
- **All 10 WASI host functions** emit `[BENCH]` lines: `load_xclbin`, `create_kernel`,
  `alloc`, `write`, `read`, `set_arg`, `set_arg_int`, `set_arg_float`, `run`, `free`.

### Execution Mode

Record which WasmEdge execution mode is used for each benchmark:

- **Interpreter** (default): `wasmedge --dir /:/ module.wasm ...`
- **AOT-compiled**: `wasmedge compile module.wasm module_aot.wasm` then `wasmedge --dir /:/ module_aot.wasm ...`
- **K3s/runwasi**: OCI-wrapped via containerd-wasm-shim (different execution mode than standalone)

All benchmark commands in this guide use interpreter mode unless stated otherwise.
Experiment A below uses a **local OCI baseline via `podman run`**, not K3s/runwasi. Treat
K3s/runwasi as a separate deployment mode used for system-integration experiments, not for the
local process/container comparison in Experiment A.

---

## 3. Pre-Benchmark Verification Checklist

Run every check before starting any experiment. All commands are run on the ZCU104.

### 3.1 FPGA state

```bash
cat /sys/class/fpga_manager/fpga0/state
# Expected: "operating"
```

### 3.2 ZOCL driver

```bash
lsmod | grep zocl
# Expected: zocl module listed
# If missing: sudo modprobe zocl
```

### 3.3 Device node

```bash
ls -la /dev/dri/renderD128
# Expected: crw-rw---- ... /dev/dri/renderD128
```

### 3.4 CMA availability

```bash
cat /proc/meminfo | grep Cma
# Expected: CmaFree > 100 MB
# Each BCPNN inference allocates ~4.2 MB across 8 buffers
```

### 3.5 WasmEdge version

```bash
wasmedge --version
# Expected: wasmedge version 0.13.5
```

### 3.6 Plugin installed

```bash
ls -la /usr/local/lib/wasmedge/libwasi_fpga.so
# Expected: file exists with recent timestamp
```

### 3.7 Smoke test

```bash
wasmedge --dir /:/ test_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin
# Expected: valid prediction output, no errors
```

### 3.8 FPGA temperature baseline

```bash
cat /sys/class/hwmon/hwmon*/temp*_input 2>/dev/null
# Expected: < 60000 (millidegrees C = < 60 C)
# If > 70 C at idle, investigate cooling before benchmarking
```

### 3.9 No memmap bootarg

```bash
cat /proc/cmdline | grep memmap
# Expected: EMPTY output
# If memmap= present, it is a v1 leftover that causes DMA corruption.
# Remove it from boot args and reboot before benchmarking.
```

### 3.10 CPU frequency governor

```bash
cat /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor
# Recommended: set to 'performance' for reduced variance:
# for f in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
#   echo performance | sudo tee $f
# done
```

---

## 4. Environment Recording / Reproducibility

Run this block before your first benchmark session to capture a full environment fingerprint.
Save the output alongside your benchmark data for paper reproducibility.

```bash
{
echo "=== Environment Record ==="
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

echo "--- Board ---"
cat /proc/device-tree/model 2>/dev/null || echo "N/A"
uname -r

echo ""
echo "--- XRT ---"
xbutil examine 2>/dev/null | head -20 || echo "xbutil not available"

echo ""
echo "--- WasmEdge ---"
wasmedge --version

echo ""
echo "--- WASI Plugin ---"
md5sum /usr/local/lib/wasmedge/libwasi_fpga.so

echo ""
echo "--- WASM Module Checksums ---"
md5sum bench_bcpnn_infer.wasm 2>/dev/null
md5sum test_bcpnn_infer.wasm 2>/dev/null

echo ""
echo "--- xclbin + Weights Checksums ---"
md5sum BCPNN_infer_float.xclbin 2>/dev/null
md5sum alvis_fullmnist_32x128_64x64_eps-4.bin 2>/dev/null

echo ""
echo "--- CMA Config ---"
cat /proc/meminfo | grep -i cma

echo ""
echo "--- CPU Governor ---"
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null

echo ""
echo "--- FPGA Temperature (baseline) ---"
cat /sys/class/hwmon/hwmon*/temp*_input 2>/dev/null

echo ""
echo "--- Kernel cmdline ---"
cat /proc/cmdline
} > environment_record.txt

cat environment_record.txt
```

### Reproducibility Bundle Checklist

After each experiment, archive the following alongside your results:

- [ ] Exact command line used
- [ ] Parameter file (if native baseline)
- [ ] `xrt.ini` (if present, or note "absent")
- [ ] `environment_record.txt`
- [ ] All checksums (plugin `.so`, WASM `.wasm`, xclbin, weights)
- [ ] Raw stdout (CSV) and stderr (logs)
- [ ] Sensor mapping (which hwmon = which rail)
- [ ] Sensor `update_interval` value
- [ ] Temperature and CMA logs

---

## 5. Building

### 5.1 WASI Plugin (on build host or ZCU104)

```bash
cd wasi-fpga
cargo build --release
# Output: target/release/libwasi_fpga.so

# Copy to ZCU104 plugin directory
scp target/release/libwasi_fpga.so user@zcu104:/usr/local/lib/wasmedge/
```

### 5.2 WASM Benchmark Module

```bash
# On any machine with rustc + wasm32-wasip1 target
rustc --target wasm32-wasip1 --edition 2021 \
  -C opt-level=z -C lto=yes \
  -o wasm/bench_bcpnn_infer.wasm wasm/bench_bcpnn_infer.rs

# Optional: further size optimization
wasm-opt -Oz wasm/bench_bcpnn_infer.wasm -o wasm/bench_bcpnn_infer.wasm

# Copy to ZCU104
scp wasm/bench_bcpnn_infer.wasm user@zcu104:~/
```

### 5.3 Fixed Test Module

```bash
rustc --target wasm32-wasip1 --edition 2021 \
  -o wasm/test_bcpnn_infer.wasm wasm/test_bcpnn_infer.rs
```

### 5.4 Native ARM Baseline (for Experiments E and G)

The native C++ baseline comes from the `bcpnn_reference` repository. It uses the same
xclbin + weights, enabling apples-to-apples comparison.

**Source:** `bcpnn_reference/Application/MNIST/INFER_FLOAT/ZCU104_FPGA_HOST/mnistmain_FPGA_infer_float.cpp`

**Binary:** `mnistmain_FPGA_infer_float`

**Compile on ZCU104:**

```bash
cd bcpnn_reference/Application/MNIST/INFER_FLOAT

# The Makefile uses these -D defines from ModelSize.mk:
# -D H_IN=784 -D M_IN=2 -D H_HID=32 -D M_HID=128
# -D H_UT=1 -D M_UT=10 -D NACTHI=64 -D NSILHI=64

make host
# Or manually:
# g++ -std=c++17 -O3 -g \
#   -I ../../../../libsrc/include/ \
#   -I /opt/xilinx/xrt/include/xrt \
#   -D H_IN=784 -D M_IN=2 -D H_HID=32 -D M_HID=128 \
#   -D H_UT=1 -D M_UT=10 -D NACTHI=64 -D NSILHI=64 \
#   -L /opt/xilinx/xrt/lib -lxrt_core -lxrt++ \
#   -o mnistmain_FPGA_infer_float mnistmain_FPGA_infer_float.cpp
```

**CLI:** `./mnistmain_FPGA_infer_float [paramfile] [xclbin] [trained_data.bin]`
- In the upstream C++ program, the defaults are `mnistmain.par`, auto-detected xclbin, and
  `trained_data.bin`.
- For this project, pass explicit paths and use
  `alvis_fullmnist_32x128_64x64_eps-4.bin` as the trained-data binary.
- Iteration count is controlled by `tenpat` in the `.par` file (default: 10000), NOT a CLI argument

**Timing methodology difference:** The native baseline uses `gettimeofday()` (POSIX wall-clock,
not monotonic) whereas our WASM module uses `std::time::Instant` (monotonic). The native code
*does* set `CL_QUEUE_PROFILING_ENABLE` on the command queue, but it does not query OpenCL event
timing — it relies on `gettimeofday()` for all measurements. Document this difference in the paper.

**Per-iteration data:** The native binary does NOT emit per-iteration CSV. For paper-quality
comparison, either modify the native code to emit per-iteration timing, or measure total time
for N iterations and divide.

---

## 6. Statistical Methodology

### 6.1 Iteration Counts

| Experiment | Minimum Iterations | Rationale |
|------------|-------------------|-----------|
| A (WASM vs OCI) | 30 startup measurements | Startup variance is high |
| B (WASI overhead) | 1000 | Sub-millisecond functions need many samples |
| C (FPGA profiling) | 1000 | Same run as B; extract `op=run` lines |
| D (E2E pipeline) | 1000 | System-level latency distribution |
| E (Energy) | 100+ | Power averaging over multiple inferences |
| F (Scalability) | 100 per pod count | Throughput stabilization |
| G (Security overhead) | 100+ | Comparing WASM vs native medians |
| H (Stability) | 8,640,000 (~24h at ~10ms/iter) | Endurance test |

### 6.2 Warmup policy

`bench_bcpnn_infer.rs` performs 1 warmup iteration before the benchmark loop (line 194-201).
For paper-quality data, additionally discard the first 10 CSV rows during analysis (first
iterations may include JIT/cache effects).

### 6.3 Clock sources

| Layer | Clock | Source |
|-------|-------|--------|
| WASM-side | `std::time::Instant` | Monotonic (WASI clock_gettime) |
| Host-side wall | `std::time::Instant` | Monotonic (Rust host) |
| Host-side OpenCL | `clGetEventProfilingInfo` | FPGA hardware clock |
| Native baseline | `gettimeofday()` | POSIX wall-clock (NOT monotonic) |

### 6.4 Statistics to report

- **Primary:** Median (robust to outliers)
- **Required:** Mean, standard deviation, P95, P99, min, max
- The built-in WASM summary (`bench_bcpnn_infer.rs:278-289`) computes min/max/mean/median.
  Use the Python analysis scripts (Section 18) for P95/P99/std dev.

### 6.5 Outlier handling

Flag any iteration with latency > 3x the median. Do not discard outliers without explanation
(CMA fragmentation, temperature throttling, and background processes are common causes).

### 6.6 Independent repetitions

For paper tables, run the full benchmark 3–5 times as independent processes (not just iterations
within one process). Report across-run variance. This captures process-level variation (WASM
module loading, OpenCL context creation) that within-run iterations miss.

### 6.7 Confidence intervals

For key comparisons (WASM vs native, WASM vs OCI), compute 95% CI or effect-size intervals.
Reference: Kalibera & Jones, "Rigorous Benchmarking in Reasonable Time" (ISMM 2013).

### 6.8 Aggregation guidance

- **Median is primary, mean is secondary.** Don't report mean alone for skewed distributions.
- **Geometric mean:** Only use for aggregating ratios across heterogeneous sub-benchmarks
  (e.g., "average speedup"). Reference: Fleming & Wallace, "How Not to Lie with Statistics"
  (CACM 1986).

---

## 7. Test Matrix

| # | Experiment | What It Measures | Tool | Target Metric |
|---|-----------|-----------------|------|---------------|
| A | WASM vs OCI Comparison | Startup time, memory, image size | `time wasmedge` vs `time podman run` | WASM 50x faster startup |
| B | WASI Extension Overhead | Per-call latency of all 10 instrumented host functions | `WASI_FPGA_BENCH=1` + `[BENCH]` lines | < 1 ms per call |
| C | FPGA Kernel Profiling | OpenCL event durations (migrate_in, kernel, migrate_out) | `[BENCH] op=run` lines | Kernel < 10 ms |
| D | E2E Pipeline Latency | Total inference time (write + run + read) | `bench_bcpnn_infer.wasm --bench N` CSV | < 15 ms total |
| E | Energy/Power | Energy per inference (W, J) | INA226 hwmon + inference timing | Quantify per-inference energy |
| F | Scalability | Throughput under concurrent pod load | N=1,2,4,8 K3s pods | Throughput curve |
| G | WASM Runtime Overhead | WASM runtime-layer cost vs native C++ | WASM total vs native total | < 5% overhead |
| H | 24h Stability | Continuous error-free operation | `--bench 8640000` | Zero errors, < 10% drift |

---

## 8. Experiment A: WASM vs OCI Container Comparison

**Scope note:** This experiment compares standalone WasmEdge against a local OCI container
baseline (`podman run`). If you also want K3s RuntimeClass startup numbers, record them
separately as a system-level deployment measurement.

### 8.1 Startup time

```bash
# --- WASM startup (30 measurements) ---
for i in $(seq 1 30); do
  { time wasmedge --dir /:/ bench_bcpnn_infer.wasm \
    ./BCPNN_infer_float.xclbin \
    ./alvis_fullmnist_32x128_64x64_eps-4.bin \
    --bench 1 > /dev/null 2>&1 ; } 2>> wasm_startup.txt
done

# --- OCI container startup (30 measurements) ---
for i in $(seq 1 30); do
  { time podman run --rm --device /dev/dri/renderD128 \
    <oci-image>:<tag> \
    ./BCPNN_infer_float.xclbin \
    ./alvis_fullmnist_32x128_64x64_eps-4.bin ; } 2>> oci_startup.txt
done
```

### 8.2 Binary / image size

```bash
# WASM binary size
ls -lh bench_bcpnn_infer.wasm

# OCI image size
podman images <oci-image>:<tag> --format "{{.Size}}"
```

### 8.3 Memory usage

```bash
# WASM memory: launch in background, sample /proc
wasmedge --dir /:/ bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 100 > /dev/null 2>&1 &
WASM_PID=$!

# Sample memory every 100ms
while kill -0 $WASM_PID 2>/dev/null; do
  grep -E 'VmRSS|VmPeak' /proc/$WASM_PID/status 2>/dev/null
  sleep 0.1
done > wasm_memory.txt
wait $WASM_PID

# Peak and steady-state RSS
grep VmPeak wasm_memory.txt | tail -1
grep VmRSS wasm_memory.txt | sort -t: -k2 -n | tail -1
```

### 8.4 WasmEdge built-in statistics (for additional context)

```bash
# Only stdout to /dev/null, stderr to grep
wasmedge --enable-all-statistics --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 10 2>&1 >/dev/null | grep -i memory
```

---

## 9. Experiment B: WASI Extension Overhead

Measures individual host function call latency for all 10 instrumented WASI functions.

```bash
WASI_FPGA_BENCH=1 wasmedge --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 1000 \
  > /dev/null 2> bench_wasi_B.log

# Extract per-operation timings
grep '^\[BENCH\]' bench_wasi_B.log
```

### Expected `[BENCH]` output formats

Each instrumented function emits one line per call:

```
[BENCH] op=load_xclbin wall_ns=123456789
[BENCH] op=create_kernel wall_ns=1234567
[BENCH] op=alloc size=6272 wall_ns=123456
[BENCH] op=write buf_id=1 bytes=6272 wall_ns=123456
[BENCH] op=set_arg arg_idx=0 buf_id=1 wall_ns=1234
[BENCH] op=set_arg_int arg_idx=9 value=100 wall_ns=1234
[BENCH] op=set_arg_float arg_idx=8 value=0.001 wall_ns=1234
[BENCH] op=run migrate_in_ns=12345 kernel_ns=6789012 migrate_out_ns=1234 total_wall_ns=7000000
[BENCH] op=read buf_id=2 bytes=40 wall_ns=12345
[BENCH] op=free buf_id=1 wall_ns=12345
```

### Parsing per-operation statistics

```bash
# Count calls per operation type
grep '^\[BENCH\]' bench_wasi_B.log | sed 's/.*op=\([^ ]*\).*/\1/' | sort | uniq -c

# Extract wall_ns for non-run operations
for op in load_xclbin create_kernel alloc write read set_arg set_arg_int set_arg_float free; do
  echo "=== $op ==="
  grep "op=$op" bench_wasi_B.log | \
    grep -oP 'wall_ns=\K[0-9]+' | \
    awk '{ sum+=$1; n++; a[n]=$1 }
         END { asort(a); printf "  n=%d min=%d max=%d mean=%.0f median=%d\n",
               n, a[1], a[n], sum/n, a[int(n/2)+1] }'
done
```

---

## 10. Experiment C: FPGA Kernel Profiling (OpenCL Events)

Same data collection as Experiment B. Focus on the `op=run` lines which contain OpenCL
hardware-measured profiling data.

```bash
# Extract kernel profiling data from the same log
grep 'op=run' bench_wasi_B.log
```

### Understanding the `op=run` fields

| Field | Source | Meaning |
|-------|--------|---------|
| `migrate_in_ns` | `clGetEventProfilingInfo(evt_migrate_in)` | DMA host→device time (FPGA clock) |
| `kernel_ns` | `clGetEventProfilingInfo(evt_kernel)` | FPGA kernel execution time (FPGA clock) |
| `migrate_out_ns` | `clGetEventProfilingInfo(evt_migrate_out)` | DMA device→host time (FPGA clock) |
| `total_wall_ns` | `Instant::now().elapsed()` | Wall-clock time for entire `run_kernel()` call |

**`N/A` values:** When `input_buf_ids` or `output_buf_ids` arrays are empty, the corresponding
migrate event is not created. The field will show `N/A`. In the BCPNN benchmark, both arrays
are non-empty (7 input buffers, 1 output buffer), so `N/A` should not appear.

### Extracting CSV for analysis

```bash
grep 'op=run' bench_wasi_B.log | \
  sed 's/.*migrate_in_ns=\([^ ]*\) kernel_ns=\([^ ]*\) migrate_out_ns=\([^ ]*\) total_wall_ns=\([0-9]*\)/\1,\2,\3,\4/' \
  > fpga_profiling_C.csv
```

The OpenCL event timestamps come from the FPGA's internal clock — they are hardware-measured,
not wall-clock. This separates actual FPGA execution time from host overhead. The difference
`total_wall_ns - (migrate_in_ns + kernel_ns + migrate_out_ns)` represents the host-side
overhead (queue dispatch, event management, `clFinish` polling).

---

## 11. Experiment D: E2E Pipeline Latency

Measures total WASM-side inference time broken into write/run/read phases.

```bash
WASI_FPGA_BENCH=1 wasmedge --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 1000 \
  > bench_results_D.csv 2> bench_log_D.txt

# View summary (printed on stderr by bench_bcpnn_infer.wasm)
tail -15 bench_log_D.txt
```

### CSV columns

The CSV on stdout has the following header and format (`bench_bcpnn_infer.rs:209`):

```
iteration,write_input_ns,run_ns,read_output_ns,total_ns,predicted_class
```

| Column | What It Measures |
|--------|-----------------|
| `iteration` | 0-indexed iteration number |
| `write_input_ns` | Time to write input data + zero output buffer (WASM `Instant`) |
| `run_ns` | Time for `run()` WASI call (includes migrate+kernel+migrate+finish) |
| `read_output_ns` | Time to read output buffer back to WASM memory |
| `total_ns` | `write_input_ns + run_ns + read_output_ns` |
| `predicted_class` | Argmax of output vector (0-9 for MNIST) |

### Quick validation

```bash
# Check row count (should be 1001: 1 header + 1000 data rows)
wc -l bench_results_D.csv

# Verify predicted_class is consistent (same synthetic input each iteration)
cut -d, -f6 bench_results_D.csv | sort | uniq -c

# Spot-check first 5 rows
head -6 bench_results_D.csv
```

---

## 12. Experiment E: Energy / Power Measurement

Compares energy per inference across configurations:

| Config | Description |
|--------|-------------|
| **WASM+FPGA** | Our system (WasmEdge + WASI plugin + OpenCL + FPGA) |
| **Native C+++FPGA** | bcpnn_reference reference (C++ host + OpenCL + FPGA) |
| **ARM CPU only** | If available without FPGA; otherwise document as N/A |

### 12.1 Identify power sensors

```bash
# Find INA226 sensors on the ZCU104
for d in /sys/class/hwmon/hwmon*; do
  name=$(cat $d/name 2>/dev/null)
  if echo "$name" | grep -qi ina; then
    echo "$d: $name"
    ls $d/power*_input 2>/dev/null
    ls $d/in*_input 2>/dev/null
    ls $d/curr*_input 2>/dev/null
  fi
done
# Look for VCCINT (PL fabric power) — this is the FPGA-specific rail
```

### 12.2 Power logging script

```bash
# Start background power logger (1 Hz, writes to power_log.csv)
POWER_SENSOR="/sys/class/hwmon/hwmonX/power1_input"  # adjust X

echo "timestamp_s,power_uw" > power_log.csv
while true; do
  echo "$(date +%s.%N),$(cat $POWER_SENSOR 2>/dev/null || echo 0)"
  sleep 1
done >> power_log.csv &
POWER_PID=$!
echo "Power logger started: PID=$POWER_PID"
```

### 12.3 Run WASM+FPGA benchmark

```bash
# Start power logging (see 12.2)
BENCH_START=$(date +%s.%N)

WASI_FPGA_BENCH=1 wasmedge --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 100 \
  > energy_bench_wasm.csv 2> energy_log_wasm.txt

BENCH_END=$(date +%s.%N)
kill $POWER_PID

echo "Benchmark ran from $BENCH_START to $BENCH_END"
```

### 12.4 Run native C++ baseline

```bash
# Restart power logger
# ... (same as 12.2)

BENCH_START=$(date +%s.%N)

cd bcpnn_reference/Application/MNIST/INFER_FLOAT
# CLI: ./mnistmain_FPGA_infer_float [paramfile] [xclbin] [trained_data.bin]
# Iteration count set by `tenpat` in .par file, NOT a CLI argument
# Use the real trained-data artifact filename even though the C++ variable is named
# `trained_data_file`.
./mnistmain_FPGA_infer_float mnistmain.par \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin

BENCH_END=$(date +%s.%N)
kill $POWER_PID
```

### 12.5 Calculate energy per inference

```bash
# In Python:
# E_per_inference = P_avg_watts * t_avg_seconds
# where P_avg = mean(power_uw) / 1e6
# and t_avg = total_benchmark_seconds / N_iterations
```

**Note:** INA226 reports board-level power in microwatts. The VCCINT rail represents PL
(programmable logic) power. Board-level power includes PS (processor) overhead which is
constant across configurations — report both total and VCCINT if distinguishable.

### 12.6 Power sampling rate considerations

At ~10ms per inference, 1 Hz sampling (Section 12.2) captures 1 sample per ~100 inferences —
only valid for long-batch energy averages. For accurate energy measurement:

1. Identify the VCCINT rail specifically
2. Check the INA226 update interval: `cat /sys/class/hwmon/hwmonX/update_interval`
   (default is typically 1.1ms but may vary)
3. Run long continuous batches (1000+ iterations)
4. Compute `E = P_avg * t_total / N_iterations`

For per-inference power correlation, higher-rate sampling (100+ Hz) would require I2C direct
access or PYNQ. The 1 Hz approach is sufficient for average-energy-per-inference claims.

---

## 13. Experiment F: Scalability

Measures throughput under concurrent multi-pod access to a single FPGA.

### 13.1 Architecture context

- Each WasmEdge instance runs as a separate OS process.
- Within a single process, `FpgaState` uses `Arc<Mutex<Option<FpgaState>>>` (`lib.rs:38`),
  so concurrent WASI calls within one process are serialized.
- Multiple pods → multiple WasmEdge processes → multiple OpenCL contexts.
  OpenCL/XRT handles FPGA multiplexing at the driver level.
- The BCPNN kernel has a single compute unit (CU), so kernel execution is inherently
  sequential. Expected result: throughput plateaus as pod count increases.

### 13.2 Multi-pod deployment

**Manifest note:** The repo currently ships Job/Pod examples, not a ready-made scalable
Deployment named `bcpnn-bench`. Use this section only if you create a Deployment variant for
the benchmark. Otherwise, use the multi-process method in Section 13.4.

```bash
# Deploy N pods using K3s (requires K3s cluster with FPGA node)
for N in 1 2 4 8; do
  echo "=== Testing N=$N pods ==="

  # Create N replicas of the benchmark pod
  kubectl scale deployment bcpnn-bench --replicas=$N

  # Wait for all pods to be Running
  kubectl wait --for=condition=Ready pod -l app=bcpnn-bench --timeout=60s

  # Collect logs from all pods (each runs --bench 100)
  for pod in $(kubectl get pods -l app=bcpnn-bench -o name); do
    kubectl logs $pod > "scalability_N${N}_${pod##*/}.csv" &
  done
  wait

  # Cleanup
  kubectl scale deployment bcpnn-bench --replicas=0
  sleep 5
done
```

### 13.3 Scalability constraints

- All concurrent processes must use the same xclbin.
- XRT profiling/trace only works for the first process in multi-process mode (XRT limitation).
  Run Experiment F without `xrt.ini` and without `WASI_FPGA_BENCH=1` for clean throughput
  numbers. If detailed per-call logs are needed, collect them in a separate diagnostic run.
- **Expected result:** With single-CU kernel, per-pod latency increases linearly with pod count;
  aggregate throughput plateaus.

### 13.4 Alternative: multi-process without K3s

```bash
for N in 1 2 4 8; do
  echo "=== N=$N processes ==="
  START=$(date +%s.%N)

  for i in $(seq 1 $N); do
    wasmedge --dir /:/ \
      bench_bcpnn_infer.wasm \
      ./BCPNN_infer_float.xclbin \
      ./alvis_fullmnist_32x128_64x64_eps-4.bin \
      --bench 100 \
      > "scale_N${N}_p${i}.csv" 2> "scale_N${N}_p${i}.log" &
  done
  wait

  END=$(date +%s.%N)
  ELAPSED=$(echo "$END - $START" | bc)
  TOTAL_ITERS=$((N * 100))
  echo "  N=$N: $TOTAL_ITERS iterations in ${ELAPSED}s"
  echo "  Aggregate throughput: $(echo "$TOTAL_ITERS / $ELAPSED" | bc -l | head -c 8) infer/s"
done
```

---

## 14. Experiment G: WASM Runtime Overhead

> **Deployment note:** Current K3s manifests use `privileged: true` with `hostNetwork`, `hostIPC`,
> and `hostPID` access. This experiment measures intra-process WASM sandbox overhead (memory
> isolation, WASI dispatch), NOT full system-level container isolation. The security benefit is
> at the WASM runtime layer — the pod-level deployment is not yet sandboxed.

Measures the runtime-layer cost of the WASM path versus native C++ running the same FPGA kernel.

### 14.1 What constitutes "runtime overhead"

Both paths use the identical FPGA kernel through OpenCL. The WASM-specific overhead is:

1. **WASI function dispatch:** WasmEdge host function call mechanism
2. **`Arc<Mutex>` lock:** Global state mutex acquisition/release per WASI call
3. **WASM ↔ host memory copy:** `WasmEdge_MemoryInstanceGetData/SetData` for buffer transfers
4. **WASM instruction execution:** Any pure-WASM computation (argmax, timing, etc.)

### 14.2 Data collection

**WASM path:**

```bash
WASI_FPGA_BENCH=1 wasmedge --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 100 \
  > security_wasm.csv 2> security_wasm.log
```

**Native C++ path:**

```bash
cd bcpnn_reference/Application/MNIST/INFER_FLOAT
# Iteration count controlled by `tenpat` in .par file, NOT a CLI argument
# Use the real trained-data artifact filename even though the C++ variable is named
# `trained_data_file`.
./mnistmain_FPGA_infer_float mnistmain.par \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  > security_native.log 2>&1
```

### 14.3 Comparison

Compare WASM `total_ns` against the native baseline carefully:

- The WASM path emits per-iteration CSV, so median/P95/P99 are directly available.
- The current native binary emits aggregate timing (`gettimeofday()`-based average/last value),
  not a per-iteration distribution.
- For a strict median-to-median comparison, implement a native benchmark harness with
  per-iteration logging. Otherwise, compare repeated process-level native runs against repeated
  WASM runs and report confidence intervals on run means/medians.

### 14.4 Three-layer analysis with WasmEdge statistics

```bash
WASI_FPGA_BENCH=1 wasmedge --enable-all-statistics --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 100 \
  > /dev/null 2> security_stats.log
```

This combines three timing layers:
1. **WasmEdge stats:** Total execution time, WASM instruction count, host function call count/time
2. **`[BENCH]` per-function:** Wall-clock per WASI call + OpenCL profiling for `run`
3. **CSV E2E:** WASM-side end-to-end per-phase timing

Extract WasmEdge stats from the tail of the log. The host function time minus OpenCL kernel
time gives pure WASM overhead.

---

## 15. Experiment H: 24-Hour Stability Test

### 15.1 Launch

```bash
# ~8,640,000 iterations at ~10ms each = ~24 hours
WASI_FPGA_BENCH=1 nohup wasmedge --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 8640000 \
  > stability_results.csv 2> stability_log.txt &

BENCH_PID=$!
echo "Stability test PID: $BENCH_PID"
```

### 15.2 Background monitoring

Start these monitors in separate terminals or tmux panes:

```bash
# Temperature monitor (every 30s)
while true; do
  echo "$(date +%s) $(cat /sys/class/hwmon/hwmon*/temp*_input 2>/dev/null | head -1)"
  sleep 30
done > stability_temperature.log &
TEMP_PID=$!

# CMA monitor (every 60s)
while true; do
  echo "$(date +%s) $(grep CmaFree /proc/meminfo | awk '{print $2}')"
  sleep 60
done > stability_cma.log &
CMA_PID=$!

echo "Monitors: temp=$TEMP_PID cma=$CMA_PID"
```

### 15.3 Progress check (non-disruptive)

```bash
# How many iterations completed
wc -l stability_results.csv

# Check for errors
grep -c 'FAILED\|error\|Error' stability_log.txt

# Current FPGA temperature
cat /sys/class/hwmon/hwmon*/temp*_input 2>/dev/null

# Current CMA
grep CmaFree /proc/meminfo
```

### 15.4 Low-overhead 24h mode

The current approach writes one CSV row per iteration: ~8.6M rows × ~60 bytes ≈ 500 MB. This
is fine for SD card storage but operationally clumsy to analyze. Options:

- **Subsample during analysis:** `awk 'NR == 1 || NR % 1000 == 0'` to extract every 1000th row
- **Summary-only mode:** Redirect stdout to `/dev/null` and rely on the stderr summary at the
  end + background monitoring for drift detection
- **Future enhancement:** Modify `bench_bcpnn_infer.rs` to only emit CSV every N iterations
  (e.g., every 1000th) with a `--csv-every N` flag

### 15.5 Post-run validation

After the test completes (or is stopped), verify these **pass/fail criteria**:

```bash
echo "=== Stability Validation ==="

# 1. Zero errors
ERRORS=$(grep -c 'FAILED\|error\|Error' stability_log.txt)
echo "Errors: $ERRORS (pass: 0)"

# 2. Latency drift < 10%
#    Compare median of first 1000 vs last 1000 iterations
head -1001 stability_results.csv | tail -1000 | \
  cut -d, -f5 | awk '{ a[NR]=$1; s+=$1 } END { asort(a); printf "First 1000 median total_ns: %d\n", a[500] }'
tail -1000 stability_results.csv | \
  cut -d, -f5 | awk '{ a[NR]=$1; s+=$1 } END { asort(a); printf "Last 1000 median total_ns: %d\n", a[500] }'

# 3. CMA never exhausted
CMA_MIN=$(awk '{print $2}' stability_cma.log | sort -n | head -1)
echo "CMA min free (kB): $CMA_MIN (pass: > 0)"

# 4. FPGA temperature never > 85 C
TEMP_MAX=$(awk '{print $2}' stability_temperature.log | sort -n | tail -1)
echo "Temp max (millideg): $TEMP_MAX (pass: < 85000)"

# 5. Predicted class consistent
CLASSES=$(cut -d, -f6 stability_results.csv | tail -n +2 | sort | uniq -c)
echo "Predicted classes: $CLASSES (pass: single class)"

# Cleanup monitors
kill $TEMP_PID $CMA_PID 2>/dev/null
```

---

## 16. WasmEdge Built-in Statistics

WasmEdge provides three individual profiling flags that can be combined with
`WASI_FPGA_BENCH=1` for multi-layer analysis:

### 16.1 Individual flags

```bash
# Time measurement only
wasmedge --enable-time-measuring --dir /:/ bench_bcpnn_infer.wasm ...

# Instruction counting only
wasmedge --enable-instruction-count --dir /:/ bench_bcpnn_infer.wasm ...

# Gas metering only
wasmedge --enable-gas-measuring --dir /:/ bench_bcpnn_infer.wasm ...

# All statistics combined
wasmedge --enable-all-statistics --dir /:/ bench_bcpnn_infer.wasm ...
```

### 16.2 Expected output

At the end of execution, WasmEdge prints statistics to stderr:

```
[2024-01-01 00:00:00.000] [info] ====================  Statistics  ====================
[2024-01-01 00:00:00.000] [info]  Total execution time: XXXX us
[2024-01-01 00:00:00.000] [info]  Wasm instructions executed: XXXX
[2024-01-01 00:00:00.000] [info]  Host functions called: XXXX
[2024-01-01 00:00:00.000] [info]  Host functions execution time: XXXX us
[2024-01-01 00:00:00.000] [info] =======================================================
```

### 16.3 Three-layer analysis

Combine with `WASI_FPGA_BENCH=1` for a complete picture:

| Layer | Source | What It Shows |
|-------|--------|--------------|
| WasmEdge stats | `--enable-all-statistics` | Total time, WASM vs host split |
| `[BENCH]` lines | `WASI_FPGA_BENCH=1` | Per-WASI-function wall-clock + OpenCL profiling |
| CSV E2E | `bench_bcpnn_infer.wasm` stdout | Per-inference write/run/read timing |

```bash
WASI_FPGA_BENCH=1 wasmedge --enable-all-statistics --dir /:/ \
  bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 100 \
  > three_layer.csv 2> three_layer.log
```

Reference: [WasmEdge CLI documentation](https://wasmedge.org/docs/start/build-and-run/cli/)

---

## 17. XRT Timeline Trace Profiling

For detailed FPGA kernel profiling beyond OpenCL events, XRT can generate timeline traces.

### 17.1 Configuration

Create an `xrt.ini` file in the working directory:

```ini
[Debug]
opencl_trace = true
device_trace = fine
data_transfer_trace = fine

[Runtime]
runtime_log = console
```

### 17.2 Generated files

After running any benchmark with `xrt.ini` present:

| File | Content |
|------|---------|
| `opencl_trace.csv` | OpenCL API call trace with timestamps |
| `timeline_trace.csv` | Device-level kernel and DMA trace |
| `summary.csv` | Aggregated statistics per kernel/transfer |

**Caveat on ZCU104:** Files are written to the SD card's working directory. Ensure
sufficient free space (traces can grow to 100+ MB for long runs).

### 17.3 XRT hygiene for benchmarking

Any `xrt.ini` in the current directory or `XRT_INI_PATH` changes runtime behavior and adds
overhead. Before final latency runs:

1. **Remove `xrt.ini`** from the benchmark working directory
2. **Set `XRT_INI_PATH` explicitly** only for profile/trace runs
3. **Verify no stray `xrt.ini`:** `find . -name xrt.ini` (check working dir and parent dirs)
4. **Note:** The `bcpnn_reference` repo has its own `xrt.ini` at the repo root — do not
   copy it to the benchmark directory

### 17.4 Important: profiling overhead

XRT profiling adds overhead to every OpenCL call. This inflates `[BENCH] wall_ns` values
and distorts latency measurements. **Run XRT profiling separately from final latency
benchmarks.** Use the XRT data for architectural analysis (DMA bandwidth, kernel
utilization) and the `[BENCH]` data (without `xrt.ini`) for paper latency numbers.

### 17.5 References

- [xrt.ini — XRT documentation](https://xilinx.github.io/XRT/master/html/xrt_ini.html)
- [xrt.ini — Vitis UG1393](https://docs.amd.com/r/en-US/ug1393-vitis-application-acceleration/xrt.ini-File)
- [Vitis profiling tutorial](https://xilinx.github.io/Vitis-Tutorials/master/docs-jp/docs/Hardware_Acceleration/Design_Tutorials/03-rtl_stream_kernel_integration/doc/profile_tutorial.html)

---

## 18. Data Analysis & Validation

### 18.1 Parse CSV with Python/pandas

```python
import pandas as pd
import numpy as np

df = pd.read_csv("bench_results_C.csv")

# Convert ns to ms
for col in ["write_input_ns", "run_ns", "read_output_ns", "total_ns"]:
    df[col.replace("_ns", "_ms")] = df[col] / 1_000_000

# Discard first 10 rows (warmup transients)
df = df.iloc[10:].reset_index(drop=True)

# Summary statistics
print(df[["write_input_ms", "run_ms", "read_output_ms", "total_ms"]].describe())

# Percentiles (for paper tables)
for col in ["run_ms", "total_ms"]:
    print(f"\n{col}:")
    for p in [50, 90, 95, 99]:
        print(f"  P{p}: {np.percentile(df[col], p):.3f} ms")
    print(f"  Std dev: {df[col].std():.3f} ms")

# Throughput
total_time_s = df["total_ns"].sum() / 1e9
print(f"\nThroughput: {len(df) / total_time_s:.1f} inferences/sec")
```

### 18.2 Parse [BENCH] log lines

```python
import re

with open("bench_log_C.txt") as f:
    lines = [l for l in f if l.startswith("[BENCH]")]

# Parse key=value pairs
records = []
for line in lines:
    pairs = re.findall(r'(\w+)=(\S+)', line)
    records.append(dict(pairs))

df_bench = pd.DataFrame(records)

# Filter to just 'run' operations
df_run = df_bench[df_bench["op"] == "run"].copy()
for col in ["migrate_in_ns", "kernel_ns", "migrate_out_ns", "total_wall_ns"]:
    df_run[col] = pd.to_numeric(df_run[col], errors="coerce")

print(df_run[["kernel_ns", "migrate_in_ns", "migrate_out_ns", "total_wall_ns"]].describe())

# Percentiles for kernel execution
for p in [50, 90, 95, 99]:
    print(f"Kernel P{p}: {np.percentile(df_run['kernel_ns'].dropna(), p) / 1e6:.3f} ms")
```

### 18.3 Data validation checks

Run these checks before using data in the paper:

```python
# 1. predicted_class should be constant (same synthetic input)
assert df["predicted_class"].nunique() == 1, \
    f"ERROR: predicted_class varies: {df['predicted_class'].unique()}"

# 2. WASM-side run_ns should approximate host-side [BENCH] op=run total_wall_ns
#    (WASM-side includes WASI dispatch overhead, so it's slightly larger)
wasm_run_median = df["run_ns"].median()
host_run_median = df_run["total_wall_ns"].median()
ratio = wasm_run_median / host_run_median
assert 0.9 < ratio < 2.0, \
    f"WARNING: WASM/host run ratio={ratio:.2f} — investigate timing discrepancy"

# 3. OpenCL kernel_ns < total_wall_ns (kernel is subset of total)
assert (df_run["kernel_ns"] <= df_run["total_wall_ns"]).all(), \
    "ERROR: kernel_ns exceeds total_wall_ns"

# 4. No NaN/Inf in numeric columns
for col in ["write_input_ns", "run_ns", "read_output_ns", "total_ns"]:
    assert df[col].notna().all() and np.isfinite(df[col]).all(), \
        f"ERROR: {col} contains NaN or Inf"

print("All validation checks passed.")
```

---

## 19. CMA and Temperature Monitoring

### 19.1 CMA (Contiguous Memory Allocator)

CMA is the kernel allocator that provides DMA-capable contiguous buffers for OpenCL/XRT.

```bash
# Current CMA status
cat /proc/meminfo | grep -i cma
# CmaTotal:         524288 kB   (= 512 MB from cma=512M bootarg)
# CmaFree:          XXXXXX kB   (available for new allocations)
```

**DMA per inference:** Each BCPNN inference allocates 8 buffers totaling ~4.2 MB:

| Buffer | Size (bytes) | Calculation |
|--------|-------------|-------------|
| inputdata | 6,272 | 784 * 2 * 4 |
| outputdata | 40 | 1 * 10 * 4 |
| rndPoisson_hid | 16,384 | 32 * 128 * 4 |
| Hihjhi_ih | 16,384 | 32 * 128 * 4 |
| Bj_ih | 16,384 | 32 * 128 * 4 |
| Wji_ih | 4,194,304 | 4096 * 256 * 4 |
| Bj_hu | 40 | 1 * 10 * 4 |
| Wji_hu | 163,840 | 10 * 4096 * 4 |
| **Total** | **4,413,648** | **~4.2 MB** |

Buffers are allocated once and reused across iterations. CMA exhaustion only occurs if
buffers are leaked (not freed) or if multiple processes allocate simultaneously.

### 19.2 Continuous CMA monitoring

```bash
while true; do
  FREE_KB=$(grep CmaFree /proc/meminfo | awk '{print $2}')
  echo "$(date +%s) $FREE_KB"
  sleep 10
done > cma_monitor.log &
```

### 19.3 Temperature monitoring

```bash
# Find temperature sensors
for d in /sys/class/hwmon/hwmon*; do
  name=$(cat $d/name 2>/dev/null)
  for t in $d/temp*_input; do
    [ -f "$t" ] && echo "$d ($name): $(cat $t) millideg"
  done
done
```

**Thresholds:**

| Level | Temperature | Action |
|-------|------------|--------|
| Normal | < 70 C | OK |
| Warning | 70-85 C | Monitor closely, check cooling |
| Critical | 85-100 C | Stop benchmarks, check fan/heatsink |
| Shutdown | > 100 C | FPGA thermal protection activates |

### 19.4 Correlating temperature with benchmarks

If latency increases during a benchmark run, check whether temperature also increased.
High temperatures can cause FPGA clock throttling, which increases `kernel_ns` in the
`[BENCH]` output.

---

## 20. Troubleshooting Common Benchmark Issues

| Symptom | Cause | Fix |
|---------|-------|-----|
| `[BENCH]` lines missing | `WASI_FPGA_BENCH=1` not set | Set env var: `WASI_FPGA_BENCH=1 wasmedge ...` |
| `CL_PROFILING_INFO_NOT_AVAILABLE` | Should not happen | `CL_QUEUE_PROFILING_ENABLE` is hardcoded; report as bug |
| CMA allocation failure | Insufficient CMA or fragmentation | Increase `cma=` bootarg or reboot to defragment |
| High variance in timing | CPU governor, background processes | Set governor to `performance`; kill unnecessary processes |
| First iteration much slower | Expected (warmup) | The module does 1 warmup iteration; additionally discard first 10 CSV rows |
| `N/A` in migrate timing | Empty buffer ID arrays | Only happens if `run()` is called with empty input/output lists; not the case for BCPNN |
| Plugin not loaded | `WASMEDGE_PLUGIN_PATH` incorrect | Verify: `ls $WASMEDGE_PLUGIN_PATH/libwasi_fpga.so` |
| All zeros in output | PAC not installed or wrong xclbin | Verify: `sudo xlnx-config -a mnist_float`; check xclbin path |
| `predicted_class` varies | Data corruption or DMA issue | Check `memmap=` bootarg (Section 3.9); verify weights checksum |
| stderr flooding | `[wasi_fpga]` diagnostic lines always active | Filter: `grep '^\[BENCH\]' logfile.txt` for just benchmark lines |
| `run error: DeviceNotFound` | FPGA not initialized | Run pre-benchmark checklist (Section 3) |
| Latency drift in long runs | Temperature increase, CMA fragmentation | Correlate with temperature/CMA logs (Section 19) |
| WASM binary too large (>5MB) | Missing optimization | Compile with `-C opt-level=z -C lto=yes`; run `wasm-opt -Oz` |

---

## 21. References

### OpenCL Profiling
- [clGetEventProfilingInfo — Khronos OpenCL 3.0](https://registry.khronos.org/OpenCL/sdk/3.0/docs/man/html/clGetEventProfilingInfo.html)
- [clReleaseEvent — Khronos OpenCL 3.0](https://registry.khronos.org/OpenCL/sdk/3.0/docs/man/html/clReleaseEvent.html)

### WasmEdge Runtime
- [WasmEdge CLI (--enable-all-statistics)](https://wasmedge.org/docs/start/build-and-run/cli/)

### Xilinx / AMD FPGA
- [xrt.ini profiling — XRT docs](https://xilinx.github.io/XRT/master/html/xrt_ini.html)
- [xrt.ini — Vitis UG1393](https://docs.amd.com/r/en-US/ug1393-vitis-application-acceleration/xrt.ini-File)
- [ZCU104 Board User Guide UG1267](https://docs.amd.com/v/u/en-US/ug1267-zcu104-eval-bd)
- [Linux INA2xx hwmon driver](https://www.kernel.org/doc/html/v5.14/hwmon/ina2xx.html)
- [PYNQ ZCU104 PMBus notebook](https://github.com/Xilinx/PYNQ/blob/master/boards/ZCU104/notebooks/common/zcu104_pmbus.ipynb)
- [Linux CMA documentation](https://www.kernel.org/doc/html/latest/mm/cma.html)

### bcpnn_reference Reference
- `bcpnn_reference/Application/MNIST/INFER_FLOAT/` — Native C++ host application
- `bcpnn_reference/libsrc/include/BCPNN_Kernel.h` — Kernel constants and architecture
- `bcpnn_reference/Application/MNIST/INFER_FLOAT/ModelSize.mk` — Model dimension defines

### Source Code Cross-References (this repo)

| File | What It Provides |
|------|-----------------|
| `src/lib.rs:216-529` | 10 `[BENCH]`-instrumented host functions |
| `src/fpga_state.rs:50-66` | `RunTimings` struct, `bench_enabled()` |
| `src/fpga_state.rs:173-177` | `CL_QUEUE_PROFILING_ENABLE` (hardcoded) |
| `src/opencl.rs:289-314` | `get_event_duration_ns()` — OpenCL profiling extraction |
| `wasm/bench_bcpnn_infer.rs:130-137` | Buffer sizes (BCPNN architecture constants) |
| `wasm/bench_bcpnn_infer.rs:209` | CSV header definition |
| `wasm/bench_bcpnn_infer.rs:278-289` | Built-in summary statistics |

### Conference
- [FPL 2026 — International Conference on Field-Programmable Logic and Applications](https://fpl.org/)

### Literature
- Prior WASM-FPGA integration work (2025) — WASM-FPGA integration, [arXiv:2503.01561](https://arxiv.org/abs/2503.01561)
- Funky (SoCC'25) — Serverless FPGA, [arXiv:2510.15755](https://arxiv.org/abs/2510.15755)
- VU Amsterdam (2025) — WASM edge computing benchmarks
- ACM TOSEM (2024) — WebAssembly performance survey
