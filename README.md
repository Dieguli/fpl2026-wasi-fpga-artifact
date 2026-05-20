# Neuromorphic FPGA Orchestration Artifact

Hybrid WebAssembly + OCI container orchestration platform for neuromorphic AI workloads on FPGA hardware (Xilinx ZCU104) in edge-cloud continuum environments.

**Key innovation:** Custom WASI extensions expose FPGA operations (via OpenCL/XRT) to sandboxed WebAssembly modules, enabling secure neuromorphic inference at the edge.

## Architecture

```
WASM Module (sandboxed)                      BCPNN Native Host (C++)
    | WASI imports (fpga.*)                      | OpenCL C++ API
    v                                            v
WASI Host Plugin (Rust, cdylib)              xcl2.hpp wrappers
    | OpenCL C API                               | OpenCL C++ API
    v                                            v
clCreateProgramWithBinary (xclbin)           cl::Program(context, bins)
clCreateKernel("BCPNN_infer_float")          cl::Kernel(program, "BCPNN_infer_float")
clCreateBuffer (CL_MEM_USE_HOST_PTR)         cl::Buffer(context, CL_MEM_USE_HOST_PTR, ...)
clSetKernelArg (buffers + scalars)           kernel.setArg(narg++, buf)
clEnqueueMigrateMemObjects -> device         qq.enqueueMigrateMemObjects({...}, 0)
clEnqueueTask                                qq.enqueueTask(kernel)
clEnqueueMigrateMemObjects -> host           qq.enqueueMigrateMemObjects({...}, HOST)
clFinish                                     qq.finish()
    |                                            |
    v                                            v
Xilinx Runtime (XRT) --> ZOCL Driver --> FPGA (ZCU104)
```

The WASM path (left) replicates the exact same OpenCL call sequence as the native BCPNN host (right). Both paths share the same XRT/ZOCL/FPGA stack underneath.

Orchestrated by **K3s** with **containerd** (no Docker daemon), using **RuntimeClass** to route workloads to either the WasmEdge runtime (neuromorphic inference) or standard runc (infrastructure).

---

## The BCPNN Model

**BCPNN (Bayesian Confidence Propagation Neural Network)** is the neuromorphic ANN deployed on this platform. Its reference implementation is treated as the separate `bcpnn_reference` artifact package.

### Model Architecture

BCPNN is a neuromorphic learning algorithm with three neural populations:

| Population | Dimensions | Size | Role |
|-----------|-----------|------|------|
| Input | H_IN=784, M_IN=2 | 1568 neurons | MNIST 28x28 pixels (binarized complementary) |
| Hidden | H_HID=32, M_HID=128 | 4096 neurons | Sparse connectivity (64 active + 64 silent) |
| Output | H_UT=1, M_UT=10 | 10 neurons | One per class (digits 0-9) |

Two inter-layer projections (input->hidden, hidden->output) carry synaptic weights (`Wji`), biases (`Bj`), and connectivity indices (`Hihjhi`).

### FPGA Kernel Variants

The BCPNN kernels are written in **C++ with HLS pragmas** (Xilinx Vitis High-Level Synthesis):

| Kernel | File | Use Case |
|--------|------|----------|
| `BCPNN_Kernel` | `BCPNN_Kernel.cpp` | Full training + inference (77KB, streaming dataflow) |
| `BCPNN_infer_float` | `BCPNN_infer_float.cpp` | Float32 inference only (deployed here) |
| `BCPNN_infer_half` | `BCPNN_infer_half.cpp` | FP16 inference |
| `BCPNN_infer_fixed` | `BCPNN_infer_fixed.cpp` | Fixed-point inference (ap_fixed<16,4>) |

**Build flow** (Vitis): `C++ source --> v++ -c --> .xo --> v++ -l --> .link.xclbin --> v++ -p --> .xclbin`

The output `.xclbin` is an **OpenCL program binary**. This is the critical constraint that determines how the host must communicate with the kernel.

### Kernel Signature (BCPNN_infer_float)

```cpp
void BCPNN_infer_float(
    float *input_hbm,           // arg 0  - buffer: N_in floats (1568)
    float *output_hbm,          // arg 1  - buffer: N_ut floats (10)
    int   *rndPoisson_hid_hbm,  // arg 2  - buffer: N_hid ints (4096)
    int   *Hihjhi_ih_hbm,       // arg 3  - buffer: H_hid * denHi_ih ints (4096)
    float *Bj_ih_hbm,           // arg 4  - buffer: N_hid floats (4096)
    float *Wji_ih_hbm,          // arg 5  - buffer: N_hid * denNi_ih floats (1048576)
    float *Bj_hu_hbm,           // arg 6  - buffer: N_ut floats (10)
    float *Wji_hu_hbm,          // arg 7  - buffer: N_ut * denNi_hu floats (40960)
    float nampl,                // arg 8  - scalar float
    int   nfreq,                // arg 9  - scalar int
    float igain0,               // arg 10 - scalar float
    float igain2,               // arg 11 - scalar float
    float bwgain1,              // arg 12 - scalar float
    float bwgain2,              // arg 13 - scalar float
    float taumdt0,              // arg 14 - scalar float
    float taumdt1,              // arg 15 - scalar float
    float taumdt2               // arg 16 - scalar float
);
```

**17 arguments**: 8 buffer pointers + 9 scalar values. This is exactly what the WASI FPGA API was designed to support (buffer args via `set_arg`, scalars via `set_arg_int`/`set_arg_float`).

---

## Why OpenCL? Two Approaches Evaluated

### The Constraint: HLS Kernels Produce OpenCL Binaries

The BCPNN kernel is compiled using Vitis HLS. The `.xclbin` output is an **OpenCL program binary**, not a raw RTL design. The reference BCPNN host applications (in `bcpnn_reference/test/MNIST_ZCU104/`) all use the **OpenCL C++ API**:

```cpp
// Reference pattern from mnistmain_FPGA_infer_float.cpp
cl::Program program(context, devices, bins);                    // Load xclbin
cl::Kernel krnl = cl::Kernel(program, "BCPNN_infer_float");     // Create kernel
cl::Buffer buf(context, CL_MEM_USE_HOST_PTR | CL_MEM_READ_ONLY, size, ptr);  // Buffers
kernel.setArg(narg++, buf_inputdata);                           // Set args
qq.enqueueMigrateMemObjects({inputs...}, 0);                    // Host -> device
qq.enqueueTask(kernel);                                         // Execute
qq.enqueueMigrateMemObjects({buf_outputdata}, CL_MIGRATE_MEM_OBJECT_HOST);   // Device -> host
```

This API contract is the fundamental constraint that determines how any host — including our WASI plugin — must interact with BCPNN kernels.

### Approach 1: XRT-Direct (Original, Abandoned)

The initial WASI plugin attempted to use the low-level **XRT HAL2 C API**:

```
xclOpen() --> xclLoadXclBin() --> xclAllocBO() --> xclMapBO() --> xclSyncBO()
                                                                  xclRegWrite() (kernel control)
                                                                  xclRegRead()  (status polling)
```

**Why it failed:**

| Problem | Root Cause |
|---------|-----------|
| Kernel never writes output | `xclRegWrite` requires `xclOpenContext()` first on ZOCL/embedded — was missing |
| Buffer corruption | Manual bank selection (`HP0=4` vs default `0`) doesn't match xclbin connectivity |
| Multi-buffer DMA failures | `memmap=256M` bootarg (v1 leftover) poisoned physical memory region |
| 11 debugging iterations | Cascading bugs masked each other; each fix exposed the next |
| Wrong abstraction | XRT HAL2 is for **RTL-level register access**, not HLS kernel orchestration |

The XRT-direct API controls FPGA hardware at the register level. HLS kernels expect higher-level orchestration: program loading, kernel scheduling, automatic buffer migration. Using `xclRegWrite` to start an HLS kernel is like using assembly to call a function that expects a C calling convention.

**Code preserved** at `wasi-fpga/src/xrt.rs` (commented out) for reference.

### Approach 2: OpenCL (Current, Validated)

The current implementation uses the **OpenCL C API**, which mirrors the BCPNN host code:

```
clGetPlatformIDs() --> clGetDeviceIDs(CL_DEVICE_TYPE_ACCELERATOR)
  --> clCreateContext() --> clCreateCommandQueue()
  --> clCreateProgramWithBinary(xclbin) --> clCreateKernel("BCPNN_infer_float")
  --> clCreateBuffer(CL_MEM_USE_HOST_PTR) --> clSetKernelArg()
  --> clEnqueueMigrateMemObjects(inputs, 0)
  --> clEnqueueTask(kernel)
  --> clEnqueueMigrateMemObjects(outputs, CL_MIGRATE_MEM_OBJECT_HOST)
  --> clFinish()
```

**Why it works:**

| Aspect | Benefit |
|--------|---------|
| Same API as BCPNN host | Proven correct by years of BCPNN development and testing |
| Automatic context management | OpenCL handles CU context lifecycle (no missing `xclOpenContext`) |
| Automatic buffer migration | `clEnqueueMigrateMemObjects` handles DMA direction and bank selection |
| Kernel scheduling | `clEnqueueTask` schedules HLS kernels correctly (not raw register writes) |
| Type marshalling | `clSetKernelArg` handles buffer pointers vs scalar values automatically |
| Platform portability | Same code works on any Xilinx platform with OpenCL runtime |

### Side-by-Side Comparison

| Aspect | XRT-Direct (v1) | OpenCL (current) |
|--------|-----------------|------------------|
| Abstraction level | HAL (register I/O) | Runtime (kernel scheduling) |
| Context setup | Manual `xclOpenContext` (was missing) | Automatic via `clCreateContext` |
| Buffer allocation | `xclAllocBO` (manual bank flags) | `clCreateBuffer` (auto-mapped) |
| Data transfer | `xclMapBO` + `xclSyncBO` | `clEnqueueMigrateMemObjects` |
| Kernel execution | `xclRegWrite` (register poke) | `clEnqueueTask` (task scheduling) |
| Matches BCPNN host? | No | Yes (1:1 mapping) |
| Status | Abandoned (11 bugs found) | Validated on ZCU104 |

---

## Current Implementation

### WASI FPGA Plugin (Rust, OpenCL Backend)

**Location:** `wasi-fpga/src/`

| File | Lines | Purpose |
|------|-------|---------|
| `lib.rs` | 542 | Plugin entry point, 10 WASI host functions, WasmEdge C API |
| `fpga_state.rs` | 631 | OpenCL device/context/queue/kernel/buffer state management |
| `opencl.rs` | 262 | Manual OpenCL FFI bindings (no bindgen required) |
| `error.rs` | 45 | Error types (`DeviceNotFound`, `InvalidBufferId`, etc.) |
| `xrt.rs` | 312 | Legacy XRT-direct code (kept as reference, not compiled into plugin) |
| `buffer_manager.rs` | 100+ | Legacy buffer manager (kept as reference, not compiled) |

**Build output:** `target/release/libwasi_fpga.so` (deployed to WasmEdge plugin directory).

### WASI FPGA API

The plugin registers 10 host functions under the `fpga` import module:

| Function | Signature | OpenCL Equivalent |
|----------|-----------|-------------------|
| `load_xclbin` | `(path_ptr, path_len) -> i32` | `clCreateProgramWithBinary` |
| `create_kernel` | `(name_ptr, name_len) -> i32` | `clCreateKernel` |
| `alloc` | `(size) -> buf_id` | `clCreateBuffer` |
| `write` | `(buf_id, data_ptr, data_len) -> i32` | memcpy into host backing store |
| `read` | `(buf_id, data_ptr, data_len) -> i32` | memcpy from host backing store |
| `set_arg` | `(arg_idx, buf_id) -> i32` | `clSetKernelArg` (buffer) |
| `set_arg_int` | `(arg_idx, value) -> i32` | `clSetKernelArg` (int scalar) |
| `set_arg_float` | `(arg_idx, value_bits) -> i32` | `clSetKernelArg` (float scalar, via `f32::to_bits`) |
| `run` | `(in_ids, in_len, out_ids, out_len) -> i32` | `clEnqueueMigrateMemObjects` + `clEnqueueTask` + `clFinish` |
| `free` | `(buf_id) -> i32` | `clReleaseMemObject` |

### WASM Test Workloads

**Location:** `wasi-fpga/wasm/`

| Module | Purpose |
|--------|---------|
| `test_bcpnn_infer.rs` | BCPNN inference with synthetic MNIST digit, loads pre-trained `.bin` weights |
| `test_bcpnn_infer_video.rs` | BCPNN inference on video frames (AVI input) |
| `test_opencl_vadd.rs` | Vector addition validation (data integrity + kernel execution) |

The `test_bcpnn_infer.rs` workload replicates the exact flow of `bcpnn_reference/test/MNIST_ZCU104/mnistmain_FPGA_infer_float.cpp`:

1. Load pre-trained weights from `.bin` file (same format as `loadVectorsFromFile()`)
2. Load `BCPNN_infer_float.xclbin` via `load_xclbin`
3. Create kernel `"BCPNN_infer_float"` via `create_kernel`
4. Allocate 8 OpenCL buffers (same sizes as the C++ host)
5. Write weights + input data to buffers
6. Set 17 kernel arguments (8 buffers + 9 scalars) — matching the kernel signature exactly
7. Execute: migrate 7 inputs -> enqueueTask -> migrate 1 output
8. Read 10 output floats, compute argmax for predicted class

### PoC Workload

**Location:** `workloads/wasm/poc-test/`

Minimal WASM module validating the WASI -> OpenCL -> FPGA chain. Binary size: ~51KB (far under 5MB target).

---

## Repository Structure

```
wasi-fpga-artifact/
├── wasi-fpga/                              # WASI FPGA plugin (Rust, OpenCL backend)
│   ├── src/
│   │   ├── lib.rs                          # Plugin entry + 10 host functions
│   │   ├── opencl.rs                       # OpenCL FFI bindings (manual, no bindgen)
│   │   ├── fpga_state.rs                   # Device/kernel/buffer state management
│   │   ├── error.rs                        # Error types
│   │   ├── xrt.rs                          # Legacy XRT-direct (reference only)
│   │   └── buffer_manager.rs               # Legacy buffer manager (reference only)
│   ├── wasm/                               # Test WASM modules (.rs sources + .wasm)
│   │   ├── test_bcpnn_infer.rs             # BCPNN inference (synthetic MNIST)
│   │   ├── test_bcpnn_infer_video.rs       # BCPNN inference (video)
│   │   └── test_opencl_vadd.rs             # Vector addition validation
│   ├── docker-build_v4_final/              # Deployment package
│   │   └── docker-build_v2/
│   │       ├── README_deploy.md            # Deployment instructions
│   │       ├── Dockerfile                  # Ubuntu 22.04 + WasmEdge + OpenCL
│   │       ├── setup_zcu104_v2.sh          # Automated ZCU104 setup (8 steps)
│   │       ├── run_fpga_poc.sh             # Execution script (synthetic or video)
│   │       ├── fpga-poc.yaml               # K8s pod manifest
│   │       ├── fpga-flexible-poc.yaml      # Dynamic pod generation
│   │       ├── fpga-video-poc.yaml         # Video processing variant
│   │       └── k3s-bcpnn-job.yaml          # Job-based execution
│   ├── Cargo.toml
│   └── Cargo.lock
│
├── workloads/wasm/poc-test/                # PoC WASM workload (wasm32-wasi)
│   ├── src/main.rs
│   ├── Cargo.toml
│   └── build.sh
│
├── install_xrt.md                          # XRT installation guide
├── troubleshooting_zocl_no_devices.md      # ZOCL driver troubleshooting
├── wasm_workload_test.md                   # WASM workload testing notes
├── CoDesign_Report_Complete.md  # Co-design analysis report
├── README_AE.md                            # Artifact evaluation quick-start
└── ARTIFACT_MANIFEST.md                    # Artifact contents and external assets
```

---

## Quick Start

### Prerequisites

- **Rust 1.75+** with `wasm32-wasi` target: `rustup target add wasm32-wasi`
- **WasmEdge 0.13.5+**: see [WasmEdge installation](https://wasmedge.org/docs/)
- **XRT 2023.1** on FPGA nodes: see [install_xrt.md](install_xrt.md)
- **BCPNN bitstream + weights**: from the bcpnn_reference repository (pre-built `.xclbin` and trained `.bin` files)

### Build the WASI Plugin

```bash
cd wasi-fpga
cargo build --release
# Output: target/release/libwasi_fpga.so
```

### Build a WASM Workload

```bash
# BCPNN inference module
rustc --target wasm32-wasip1 -o wasm/test_bcpnn_infer.wasm wasm/test_bcpnn_infer.rs

# Or the PoC test
cd workloads/wasm/poc-test
cargo build --target wasm32-wasi --release
```

### Run Locally (on FPGA node)

```bash
# Copy plugin to WasmEdge directory
cp libwasi_fpga.so /usr/local/lib/wasmedge/

# Run BCPNN inference with pre-trained weights
wasmedge --dir /:/ test_bcpnn_infer.wasm \
  /path/to/BCPNN_infer_float.xclbin \
  /path/to/trained_weights.bin
```

### Deploy to K3s

See the [deployment guide](wasi-fpga/docker-build_v4_final/docker-build_v2/README_deploy.md) for:
1. ZCU104 OS installation (Ubuntu 22.04)
2. Automated setup script (XRT, WasmEdge, K3s)
3. Running inference pods (synthetic or video)
4. Adapting for custom models

---

## Deployment

### End-to-End Flow

```
1. Build BCPNN kernel       bcpnn_reference:  make build TARGET=hw
   (done once)              Output: BCPNN_infer_float.xclbin

2. Train model              bcpnn_reference:  ./mnistmain_FPGA <params>
   (done once)              Output: trained_weights.bin

3. Build WASI plugin        wasi-fpga-artifact:   cd wasi-fpga && cargo build --release
   (done once)              Output: libwasi_fpga.so

4. Build WASM workload      wasi-fpga-artifact:   rustc --target wasm32-wasip1 ...
   (per workload)           Output: test_bcpnn_infer.wasm

5. Setup ZCU104             On board:           sudo ./setup_zcu104_v2.sh
   (done once)              Installs: XRT, WasmEdge, K3s, plugin

6. Run inference            On board:           sudo ./run_fpga_poc.sh --synthetic
   (per execution)          Or: sudo ./run_fpga_poc.sh output.avi
```

### Required Artifacts on ZCU104

| Artifact | Source | Location on Board |
|----------|--------|-------------------|
| `BCPNN_infer_float.xclbin` | bcpnn_reference build | `/home/ubuntu/bcpnn_artifacts/` |
| `trained_weights.bin` | bcpnn_reference training | `/home/ubuntu/bcpnn_artifacts/` |
| `libwasi_fpga.so` | This repo, `cargo build --release` | `/usr/local/lib/wasmedge/` |
| `test_bcpnn_infer.wasm` | This repo, `rustc --target wasm32-wasip1` | `/home/ubuntu/bcpnn_artifacts/wasm/` |

### Hardware Requirements

- **Board:** Xilinx ZCU104 (Zynq UltraScale+ MPSoC)
- **OS:** Ubuntu 22.04 (certified Xilinx image)
- **Memory:** CMA set to 1024MB (`cma=1024M` in kernel cmdline)
- **Storage:** 32GB+ MicroSD (Class 10)
- **Network:** Ethernet for K3s and image pulls
- **Drivers:** XRT 2023.1 with ZOCL module loaded

---

## Docker-Free Design

This project uses **containerd** natively (embedded in K3s) with no Docker daemon:

- **Runtime:** K3s + containerd + containerd-wasm-shims (runwasi)
- **Image builds:** Buildah or Podman (Docker CLI compatible)
- **Why:** Lower resource footprint, better security, simpler architecture, K8s v1.24+ dropped dockershim

```
Kubernetes (K3s)
    | CRI gRPC
    v
containerd (/var/lib/rancher/k3s/agent/)
    |                       |
    v                       v
runc                    containerd-wasm-shim
(OCI containers)        (WASM modules)
    |                       |
    v                       v
Device Plugin           WasmEdge + libwasi_fpga.so
Monitoring              (BCPNN inference)
    |                       |
    +----------+------------+
               |
               v
         XRT -> ZOCL -> FPGA
```

---

## Documentation

| Document | Description |
|----------|-------------|
| [install_xrt.md](install_xrt.md) | XRT installation on ZCU104 |
| [troubleshooting_zocl_no_devices.md](troubleshooting_zocl_no_devices.md) | ZOCL driver troubleshooting |
| [wasm_workload_test.md](wasm_workload_test.md) | WASM workload testing guide |
| [Deployment README](wasi-fpga/docker-build_v4_final/docker-build_v2/README_deploy.md) | K3s deployment instructions |
| [PoC Test README](workloads/wasm/poc-test/README.md) | PoC validation details |
| [CoDesign Report](CoDesign_Report_Complete.md) | Full co-design analysis |
| [Artifact Evaluation README](README_AE.md) | FPL artifact evaluator quick-start |
| [Artifact Manifest](ARTIFACT_MANIFEST.md) | Included files, external assets, and form-aligned availability notes |

## Key Technologies

- **[WasmEdge](https://wasmedge.org/docs/)** -- WebAssembly runtime with plugin support
- **[WASI](https://wasi.dev/)** -- WebAssembly System Interface specification
- **[K3s](https://docs.k3s.io/)** -- Lightweight Kubernetes for edge
- **[containerd](https://containerd.io/docs/)** / **[runwasi](https://github.com/containerd/runwasi)** -- Container + WASM runtime integration
- **[Xilinx Runtime (XRT)](https://xilinx.github.io/XRT/)** -- FPGA driver and runtime
- **[OpenCL (Khronos)](https://www.khronos.org/opencl/)** -- Heterogeneous compute API (FPGA backend)
- **[Xilinx Vitis HLS](https://www.xilinx.com/products/design-tools/vitis.html)** -- High-Level Synthesis for FPGA kernels
- **[Kubernetes RuntimeClass](https://kubernetes.io/docs/concepts/containers/runtime-class/)** -- Workload routing
- **[Kubernetes Device Plugins](https://kubernetes.io/docs/concepts/extend-kubernetes/compute-storage-net/device-plugins/)** -- Hardware resource management
- **[Buildah](https://buildah.io/)** -- Daemonless OCI image builder

## Project Status

**Version:** 0.2.0 (OpenCL Backend)

| Milestone | Status |
|-----------|--------|
| WASI FPGA plugin (10 host functions, OpenCL backend) | Done |
| BCPNN inference WASM workloads (synthetic + video) | Done |
| Deployment package (setup script, K8s manifests, Dockerfile) | Done |
| PoC validation (WASM -> WASI -> OpenCL -> XRT -> FPGA) | Done |
| OpenCL consolidation (XRT-direct approach abandoned) | Done |
| Kubernetes device plugin (Go) | Planned |
| Full E2E Kubernetes tests | Planned |
| 24h stability validation | Planned |

## License

Apache-2.0

## Project Context

Prepared as an anonymized artifact package for FPGA orchestration evaluation.

### Related Repositories

- **bcpnn_reference** -- BCPNN model: HLS kernels, host applications, training, pre-trained weights
- **bcpnn_artifacts** -- Video demo application (camera/AVI input)

### Publications

Background publications should be cited in the anonymized paper bibliography rather than linked from this review artifact.
