# ZCU104 Kubernetes Edge AI & WasmEdge FPGA Deployment Guide

This directory contains the automated infrastructure to deploy BCPNN (Bayesian Confidence Propagation Neural Network) inference workloads at the edge using a **Xilinx ZCU104** board, **Kubernetes (K3s)**, **WasmEdge**, and FPGA hardware acceleration via **OpenCL/XRT**.

The environment is designed as a boilerplate (reference template). The underlying infrastructure is fully transparent; users only need to provide their own model (`.xclbin`), weights (`.bin`), and WebAssembly application (`.wasm`).

## How It Works

```
run_fpga_poc.sh
    |
    v
K3s creates Pod (from generated YAML)
    |
    v
containerd pulls image --> starts container
    |
    v
wasmedge --dir /:/ test_bcpnn_infer.wasm <xclbin> <weights>
    |
    v
WASM module imports fpga.* functions from libwasi_fpga.so
    |
    v
Plugin calls OpenCL API:
  1. clCreateProgramWithBinary(xclbin)    --> loads FPGA bitstream
  2. clCreateKernel("BCPNN_infer_float")  --> creates kernel handle
  3. clCreateBuffer (8 buffers)           --> allocates DMA memory
  4. clSetKernelArg (17 args)             --> binds buffers + scalars
  5. clEnqueueMigrateMemObjects (inputs)  --> host -> FPGA
  6. clEnqueueTask                        --> runs inference
  7. clEnqueueMigrateMemObjects (output)  --> FPGA -> host
  8. clFinish                             --> wait for completion
    |
    v
WASM reads 10 output floats --> argmax = predicted class
```

## Prerequisites

- Xilinx ZCU104 board and its power supply
- MicroSD card (32GB minimum, Class 10 or higher)
- Network connection on the board (Ethernet)
- BCPNN artifacts from the bcpnn_reference repository:
  - `BCPNN_infer_float.xclbin` (FPGA bitstream, built with `make build TARGET=hw`)
  - Trained weights `.bin` file (from `TrainedWeight/` or training output)

## Directory Structure

```text
docker-build_v2/
├── Dockerfile                      # Base image: Ubuntu 22.04 + WasmEdge + OpenCL
├── setup_zcu104_v2.sh              # Zero-touch installation script (Step 2)
├── run_fpga_poc.sh                 # Main execution script (Step 3)
├── libwasi_fpga.so                 # Compiled WASI FPGA plugin (OpenCL backend)
├── fpga-poc.yaml                   # K8s pod manifest (basic)
├── fpga-flexible-poc.yaml          # K8s pod manifest (dynamic generation)
├── fpga-video-poc.yaml             # K8s pod manifest (video processing)
├── k3s-bcpnn-job.yaml              # K8s Job manifest
├── debug-pod.yaml                  # Debugging pod (optional)
└── wasm/                           # WebAssembly modules
    ├── test_bcpnn_infer.wasm       # BCPNN inference (synthetic data)
    └── test_bcpnn_infer_video.wasm # BCPNN inference (video input)
```

---

## Step 1: Operating System Installation

The environment is built on the officially certified Ubuntu image for Xilinx.

1. Download the **Ubuntu 22.04** image for ZCU104
2. Flash the image to your MicroSD card (BalenaEtcher, Rufus, or `dd`)
3. Follow the official Xilinx instructions:
   [Getting Started with Certified Ubuntu 22.04 LTS for Xilinx Devices](https://xilinx-wiki.atlassian.net/wiki/spaces/A/pages/2057043969/Snaps+-+xlnx-config+Snap+for+Certified+Ubuntu+on+Xilinx+Devices)

Once installed, insert the SD card, power on the board, and connect via SSH or serial using the default user `ubuntu` / password `ubuntu`.

---

## Step 2: Automatic Configuration (Zero-Touch Setup)

The setup script consolidates all low-level configuration into a single command. It will automatically:

1. Create 4GB of swap memory (required for XRT compilation on-board)
2. Install system dependencies + build XRT driver (Xilinx Runtime)
3. Download GitHub repositories and sample models
4. Configure kernel memory: **CMA to 1024M** in `/etc/default/flash-kernel`
5. Install PAC firmware and enable hardware platform
6. Install **WasmEdge** 0.13.5 runtime
7. Install **K3s** (lightweight Kubernetes with embedded containerd)
8. Pull and re-tag the base container image

**Instructions:**

```bash
# Upload the setup script to the ZCU104 board
scp setup_zcu104_v2.sh ubuntu@<board-ip>:~/

# SSH into the board and run
ssh ubuntu@<board-ip>
chmod +x setup_zcu104_v2.sh
sudo ./setup_zcu104_v2.sh
```

> **Note:** The script takes 15-20 minutes (depending on whether XRT needs to be compiled). It will **reboot the board automatically** at the end to apply kernel and hardware changes.

### What Gets Installed

| Component | Version | Purpose |
|-----------|---------|---------|
| XRT | 2023.1 | FPGA runtime + ZOCL driver |
| WasmEdge | 0.13.5 | WebAssembly runtime |
| K3s | 1.28+ | Lightweight Kubernetes (includes containerd) |
| CMA | 1024MB | Contiguous Memory Allocator for DMA buffers |

### Post-Reboot Verification

After the board reboots, verify the setup:

```bash
# Check ZOCL driver is loaded
lsmod | grep zocl

# Check XRT installation
xbutil examine 2>/dev/null || echo "XRT installed (edge mode)"

# Check K3s is running
sudo k3s kubectl get nodes

# Check WasmEdge
wasmedge --version

# Check CMA allocation
dmesg | grep -i cma
```

---

## Step 3: Run the Inference Pod (Smoke Test)

After the board reboots, the environment is ready. Upload the run script and WASM modules to the board.

### Option A: Synthetic Data Test

Run BCPNN inference with automatically generated synthetic MNIST-like noise patterns:

```bash
sudo ./run_fpga_poc.sh --synthetic
```

### Option B: Video Inference

Run inference on an `.avi` video file:

```bash
# Place video in /bcpnn_artifacts/video_input/
sudo ./run_fpga_poc.sh output.avi
```

### What Happens

1. The script checks/starts K3s service
2. Drops filesystem caches and stops unnecessary services (gdm3) to maximize CMA memory
3. Copies `libwasi_fpga.so` to `/usr/local/lib/wasmedge/`
4. Generates a K8s pod YAML with correct volume mounts
5. Creates the pod via `kubectl apply`
6. The pod runs WasmEdge with the BCPNN WASM module
7. Output shows predicted class (0-9) for each input frame
8. Pod is cleaned up after execution

### Expected Output

```
=== WASI-FPGA BCPNN Inference Test ===
Kernel: BCPNN_infer_float
Network: 784x2 -> 32x128 -> 1x10
...
[8] Running kernel (migrate 7 inputs -> execute -> migrate output)...
...
=== RESULTADO DE INFERENCIA ===
  Clase 0:     -0.234500
  Clase 1:      0.891200  ████████████████████████
  ...
  Predicted class: 1 (value: 0.891200)
```

---

## Step 4: Adapt for Your Own Models

To deploy your own FPGA-accelerated AI model, you need three artifacts:

### Required Artifacts

| Artifact | How to Produce | What It Contains |
|----------|---------------|-----------------|
| `.xclbin` | Build with Vitis HLS (`make build TARGET=hw`) | FPGA bitstream with your kernel |
| `.bin` | Train your model, save weights | Serialized weight vectors |
| `.wasm` | Compile Rust with `--target wasm32-wasip1` | Your inference logic calling WASI FPGA API |

### Writing a Custom WASM Module

Your WASM module must import the `fpga` module and follow this pattern:

```rust
#[link(wasm_import_module = "fpga")]
extern "C" {
    fn load_xclbin(path_ptr: *const u8, path_len: i32) -> i32;
    fn create_kernel(name_ptr: *const u8, name_len: i32) -> i32;
    fn alloc(size: i32) -> i32;        // returns buffer ID
    fn write(buf_id: i32, data_ptr: *const u8, data_len: i32) -> i32;
    fn read(buf_id: i32, data_ptr: *mut u8, data_len: i32) -> i32;
    fn set_arg(arg_idx: i32, buf_id: i32) -> i32;
    fn set_arg_int(arg_idx: i32, value: i32) -> i32;
    fn set_arg_float(arg_idx: i32, value_bits: i32) -> i32;
    fn run(in_ids_ptr: *const i32, in_ids_len: i32,
           out_ids_ptr: *const i32, out_ids_len: i32) -> i32;
    fn free(buf_id: i32) -> i32;
}

fn main() {
    // 1. Load bitstream
    load_xclbin(path_ptr, path_len);

    // 2. Create kernel (name must match kernel in xclbin)
    create_kernel(name_ptr, name_len);

    // 3. Allocate buffers for each kernel argument that is a pointer
    let buf_in = alloc(input_size_bytes);
    let buf_out = alloc(output_size_bytes);

    // 4. Write data into input buffers
    write(buf_in, data_ptr, data_len);

    // 5. Set kernel arguments (match your kernel's signature)
    set_arg(0, buf_in);       // buffer argument
    set_arg(1, buf_out);      // buffer argument
    set_arg_int(2, value);    // scalar int argument
    set_arg_float(3, f32::to_bits(value) as i32);  // scalar float argument

    // 6. Run kernel (specify which buffers are inputs vs outputs)
    let inputs = [buf_in];
    let outputs = [buf_out];
    run(inputs.as_ptr(), inputs.len() as i32,
        outputs.as_ptr(), outputs.len() as i32);

    // 7. Read results
    read(buf_out, result_ptr, result_len);

    // 8. Cleanup
    free(buf_in);
    free(buf_out);
}
```

See `test_bcpnn_infer.rs` in the `wasm/` directory for a complete working example.

### Updating the Run Script

Edit `run_fpga_poc.sh` to point to your files:

```bash
XCLBIN="my_custom_model.xclbin"
WEIGHTS="my_weights.bin"
```

---

## Troubleshooting

### ZOCL Driver Not Loaded

```bash
# Symptom: OpenCL can't find FPGA device
sudo modprobe zocl
# If that fails, check XRT installation:
ls /lib/modules/$(uname -r)/extra/zocl.ko
```

### CMA Memory Insufficient

```bash
# Symptom: Buffer allocation fails
# Check current CMA:
dmesg | grep -i cma
cat /proc/meminfo | grep Cma

# Fix: Ensure cma=1024M in kernel cmdline
sudo nano /etc/default/flash-kernel
# Add: LINUX_KERNEL_CMDLINE_DEFAULTS="cma=1024M"
sudo flash-kernel
sudo reboot
```

### memmap Bootarg Poisoning

```bash
# Symptom: Multi-buffer DMA corruption, buffers contain 0x7FC00000 (NaN)
# Cause: Legacy memmap=256M$0x70000000 in bootargs reserves memory that XRT allocates

# Check:
cat /proc/cmdline | grep memmap

# Fix: Remove any memmap= entries from bootargs
# Edit boot configuration and reboot
```

### K3s Service Not Running

```bash
sudo systemctl start k3s
sudo k3s kubectl get nodes
# Node should show "Ready" status
```

### Pod Stuck in Pending

```bash
# Check pod events
sudo k3s kubectl describe pod <pod-name>

# Common causes:
# - Image not available locally (check: sudo k3s ctr images list)
# - Volume mount paths don't exist on host
# - Insufficient CMA memory
```

### OpenCL Device Not Found

```bash
# Verify device exists
ls -la /dev/dri/renderD128
ls -la /dev/uio0

# Check OpenCL ICD
cat /etc/OpenCL/vendors/*.icd

# Check XRT OpenCL library
ls -la /opt/xilinx/xrt/lib/libxilinxopencl.so
```

---

## Architecture Details

### Pod Volume Mounts

The K8s pod requires these host paths mounted:

| Host Path | Container Path | Purpose |
|-----------|---------------|---------|
| `/dev` | `/dev` | FPGA device files (uio0, dri/renderD128) |
| `/lib` | `/lib` | System libraries |
| `/usr/lib/aarch64-linux-gnu` | `/usr/lib/aarch64-linux-gnu` | ARM64 libraries |
| `/etc/OpenCL` | `/etc/OpenCL` | OpenCL ICD configuration |
| `/opt/xilinx/xrt/lib` | `/opt/xilinx/xrt/lib` | XRT runtime libraries |
| `/home/ubuntu/bcpnn_artifacts` | `/data` | xclbin, weights, WASM modules |
| `libwasi_fpga.so` | `/usr/local/lib/wasmedge/` | WASI FPGA plugin |

### Security Note

The pod runs with `privileged: true` and `hostNetwork: true` because:
- Direct FPGA device access requires privileged mode (`/dev/uio0`, `/dev/dri/renderD128`)
- XRT/ZOCL driver communication needs host device access
- The Kubernetes device plugin (planned) will eliminate the need for privileged mode

---

*Developed for Edge Computing and Hybrid AI environments (WebAssembly + FPGA + Kubernetes).*
*Prepared as an anonymized artifact package for FPGA orchestration evaluation.*
