# Artifact Manifest

This repository is the public artifact package for the FPL 2026 paper on WASI-FPGA orchestration for neuromorphic inference. It is a curated snapshot of the implementation branch `origin/feature/benchmark-instrumentation` at commit `cf60e03`.

## Included in this Repository

| Path | Artifact type | Purpose |
| --- | --- | --- |
| `wasi-fpga/src/` | Software | Rust WasmEdge plugin implementing 10 `fpga.*` WASI host functions over OpenCL/XRT. |
| `wasi-fpga/wasm/` | Software | Rust source for WASM validation and benchmark workloads. |
| `workloads/wasm/poc-test/` | Software | Minimal proof-of-concept WASM workload and build script. |
| `wasi-fpga/docker-build_v4_final/docker-build_v2/` | Software/configuration | ZCU104 setup script, K3s manifests, Dockerfile, and run scripts for deployment. |
| `wasi-fpga/BENCHMARKING_GUIDE.md` | Documentation | Full experiment protocol for FPL artifact evaluation. |
| `install_xrt.md` | Documentation | XRT installation and ZCU104 setup notes. |
| `troubleshooting_zocl_no_devices.md` | Documentation | ZOCL/XRT troubleshooting guide. |
| `CoDesign_Report_Complete.md` | Documentation | System architecture and co-design context. |
| `README_AE.md` | Documentation | Artifact evaluator quick-start and form-aligned description. |

## Generated Binaries Not Tracked in Git

The following generated binaries were removed from Git history in this artifact repository and should be rebuilt or attached to a versioned release/Zenodo deposit:

| File pattern | How to reproduce or provide |
| --- | --- |
| `wasi-fpga/target/release/libwasi_fpga.so` | Build with `cd wasi-fpga && cargo build --release` on the target-compatible build host. |
| `wasi-fpga/wasm/*.wasm` | Build from the corresponding Rust source using `rustc --target wasm32-wasip1 --edition 2021`. |
| `wasi-fpga/docker-build_v4_final/docker-build_v2/avi_processor*` | Build from `wasi-fpga/src/bin/avi_processor.rs` if the video path is evaluated. |
| `BCPNN_infer_float.xclbin` | Build from the BCPNN HLS repository with the Vitis flow, or attach as a release asset. |
| `alvis_fullmnist_32x128_64x64_eps-4.bin` | Provide as a release asset if redistribution is permitted. |

## External Dependencies

The artifact depends on non-author-created proprietary or vendor-specific components:

| Dependency | Role | Availability note |
| --- | --- | --- |
| Xilinx ZCU104 board | Target FPGA platform | Commercial evaluation board. |
| AMD/Xilinx XRT and ZOCL | FPGA runtime and Linux driver | Vendor runtime required for hardware runs. |
| AMD/Xilinx Vitis HLS | Builds the BCPNN `.xclbin` | Proprietary vendor toolchain. |
| Certified Ubuntu 22.04 image for ZCU104 | Target operating system | Vendor-supported image. |

## Release Assets to Archive with Zenodo

For an artifact-evaluation release, attach these files or document why they cannot be redistributed:

| Asset | Required for |
| --- | --- |
| `environment_record.txt` | Baseline experimental setup and reproducibility fingerprint. |
| `checksums.sha256` | Integrity verification for source archive, plugin, WASM binaries, `.xclbin`, weights, and result logs. |
| `BCPNN_infer_float.xclbin` | Hardware execution on ZCU104. |
| `alvis_fullmnist_32x128_64x64_eps-4.bin` | BCPNN representative inference and benchmark runs. |
| `bench_results_*.csv` and `bench_log_*.txt` | Paper result validation. |
| `stability_results.csv`, `stability_log.txt`, `stability_temperature.log`, `stability_cma.log` | 24-hour stability validation, if claimed in the paper. |

## Artifact Status for the FPL Form

| Form topic | Recommended answer after Zenodo release |
| --- | --- |
| Software artifacts | Public repository under Apache-2.0. |
| Hardware artifacts | Mark all available only if the BCPNN HLS kernel source and bitstream are released under an open hardware-compatible license; otherwise mark some unavailable. |
| Data artifacts | Mark all maintained with stable identifier only after weights, representative inputs, and raw result logs are archived with the DOI. |
| Proprietary artifacts | No author-created artifacts are proprietary; associated vendor board/toolchain artifacts are proprietary. |
