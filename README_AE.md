# FPL 2026 Artifact Evaluation README

This file gives artifact evaluators a compact entry point. The detailed experiment protocol is in `wasi-fpga/BENCHMARKING_GUIDE.md`.

## Artifact Summary

The artifact implements a WebAssembly-to-FPGA execution path for BCPNN neuromorphic inference on a Xilinx ZCU104 board:

1. A Rust WasmEdge plugin exposes 10 `fpga.*` WASI host functions.
2. The plugin maps those calls to OpenCL C API operations.
3. OpenCL/XRT/ZOCL execute the `BCPNN_infer_float` kernel on the ZCU104 FPGA.
4. WASM workloads run synthetic and representative inference experiments.
5. K3s manifests demonstrate deployment through an edge orchestration stack.

The artifact is intended to support the following evaluation claims:

| Claim | Evidence path |
| --- | --- |
| WASM can invoke FPGA kernels through a WASI extension | `wasi-fpga/src/`, `wasi-fpga/wasm/test_bcpnn_infer.rs` |
| The OpenCL backend matches the native BCPNN host execution model | `README.md`, `wasi-fpga/src/fpga_state.rs`, `wasi-fpga/src/opencl.rs` |
| The platform can collect latency, OpenCL event, energy, scalability, and stability metrics | `wasi-fpga/BENCHMARKING_GUIDE.md`, `wasi-fpga/wasm/bench_bcpnn_infer.rs` |
| The deployment path can run on ZCU104 with K3s/WasmEdge/XRT | `wasi-fpga/docker-build_v4_final/docker-build_v2/README_deploy.md` |

## Baseline Setup

| Component | Expected configuration |
| --- | --- |
| Target hardware | Xilinx ZCU104, Zynq UltraScale+ MPSoC |
| Operating system | Certified Ubuntu 22.04 for Xilinx devices |
| FPGA runtime | XRT 2023.1 or compatible ZCU104-supported XRT with ZOCL loaded |
| WebAssembly runtime | WasmEdge 0.13.5 or newer with plugin support |
| Build language | Rust 1.75 or newer with `wasm32-wasip1` target |
| Orchestration | K3s 1.28 or newer with containerd |
| Kernel artifact | `BCPNN_infer_float.xclbin` |
| Model data | `alvis_fullmnist_32x128_64x64_eps-4.bin` |

## Build

Build the WasmEdge plugin:

```bash
cd wasi-fpga
cargo build --release
sudo cp target/release/libwasi_fpga.so /usr/local/lib/wasmedge/
```

Build the benchmark WASM module:

```bash
rustc --target wasm32-wasip1 --edition 2021 \
  -C opt-level=z -C lto=yes \
  -o wasi-fpga/wasm/bench_bcpnn_infer.wasm \
  wasi-fpga/wasm/bench_bcpnn_infer.rs
```

Build the fixed smoke-test module:

```bash
rustc --target wasm32-wasip1 --edition 2021 \
  -o wasi-fpga/wasm/test_bcpnn_infer.wasm \
  wasi-fpga/wasm/test_bcpnn_infer.rs
```

## Smoke Test

Run this on the ZCU104 after XRT, ZOCL, WasmEdge, the plugin, the `.xclbin`, and the weights are installed:

```bash
wasmedge --dir /:/ wasi-fpga/wasm/test_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin
```

Expected behavior: the workload loads the bitstream, creates kernel `BCPNN_infer_float`, allocates 8 buffers, sets 17 kernel arguments, runs inference, and prints a predicted class without runtime errors.

## Main Benchmark

```bash
WASI_FPGA_BENCH=1 wasmedge --dir /:/ \
  wasi-fpga/wasm/bench_bcpnn_infer.wasm \
  ./BCPNN_infer_float.xclbin \
  ./alvis_fullmnist_32x128_64x64_eps-4.bin \
  --bench 1000 \
  > bench_results_D.csv 2> bench_log_D.txt
```

The benchmark CSV columns are:

```text
iteration,write_input_ns,run_ns,read_output_ns,total_ns,predicted_class
```

`[BENCH]` lines in stderr provide per-host-function wall-clock timings and OpenCL event timings for `migrate_in`, `kernel`, and `migrate_out`.

## Experiment Map

| Experiment | Description | Primary output |
| --- | --- | --- |
| A | WASM vs OCI startup, memory, and image size | `wasm_startup.txt`, `oci_startup.txt`, memory logs |
| B | WASI extension overhead | `[BENCH]` operation timings |
| C | FPGA kernel profiling | `fpga_profiling_C.csv` |
| D | End-to-end pipeline latency | `bench_results_D.csv`, `bench_log_D.txt` |
| E | Energy/power | INA226/hwmon power logs plus inference timing |
| F | Scalability | per-pod or per-process throughput CSV/logs |
| G | WASM runtime overhead vs native C++ | `security_wasm.csv`, native baseline logs |
| H | 24-hour stability | stability CSV, error log, temperature log, CMA log |

## Known Evaluation Constraints

The artifact is hardware-dependent. Full reproduction requires a ZCU104 board with the correct XRT/ZOCL stack and a compatible BCPNN `.xclbin`. Evaluators without this hardware can still inspect, build, and validate the WASM/plugin source, but cannot reproduce the hardware-timed results.

The BCPNN HLS kernel source, compiled bitstream, trained weights, representative input data, and raw result logs should be archived as versioned release assets or in the Zenodo deposit associated with the citable DOI.
