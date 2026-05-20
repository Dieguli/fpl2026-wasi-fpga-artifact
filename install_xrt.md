# Complete Guide: XRT Installation for ZCU104

**Project:** Neuromorphic FPGA Orchestration - Phase 4 FPGA Deployment
**Target Hardware:** Xilinx ZCU104 Evaluation Board (Zynq UltraScale+ MPSoC)
**Supported OS:** Ubuntu 22.04 (Recommended) or PetaLinux
**XRT Version:** 2023.2 (Ubuntu) or 2023.1 (PetaLinux)
**Last Updated:** December 2, 2025

---

## Table of Contents

1. [Quick Start: Ubuntu 22.04 + XRT 2023.2](#quick-start-ubuntu-2204--xrt-20232)
2. [Problem Context](#problem-context)
3. [System Architecture](#system-architecture)
4. [Option 1: Xilinx Meta-Layer (Recommended for PetaLinux)](#option-1-use-xilinx-meta-layer-recommended)
5. [Option 2: Manual XRT Compilation](#option-2-manual-xrt-compilation-for-arm)
6. [Option 3: Post-Boot Installation](#option-3-post-boot-installation-simpler-less-ideal)
7. [Installation Verification](#installation-verification)
8. [WASI FPGA Extension Build](#building-the-wasi-fpga-extension)
9. [PoC Test Execution](#poc-test-execution)
10. [Detailed Troubleshooting](#detailed-troubleshooting)
11. [Advanced Configuration](#advanced-configuration)
12. [Validation Checklist](#validation-checklist---phase-2)
13. [References and Resources](#additional-resources)

---

## Quick Start: Ubuntu 22.04 + XRT 2023.2

> **NEW in Phase 4:** Ubuntu 22.04 is now the recommended OS for development and
> testing. This provides a more familiar environment with standard package management.

### Why Ubuntu 22.04?

| Aspect | Ubuntu 22.04 | PetaLinux |
|--------|--------------|-----------|
| **Package Management** | APT (standard) | Yocto recipes |
| **XRT Installation** | Simple `.deb` package | Build from source |
| **Development Tools** | Pre-packaged | Manual installation |
| **Build Time** | Minutes | Hours |
| **Familiarity** | Standard Linux | Embedded Linux |

### Prerequisites

1. **ZCU104 with Ubuntu 22.04 SD card image**
   - Download from Xilinx/AMD or build with PYNQ
   - Or use the official Ubuntu image for Zynq UltraScale+

2. **Network access** (for package downloads)

3. **At least 4GB free disk space**

### Step 1: Download XRT 2023.2 for ARM64

```bash
# On ZCU104 running Ubuntu 22.04

# Create downloads directory
mkdir -p ~/downloads && cd ~/downloads

# Download XRT 2023.2 for ARM64 Ubuntu
# Option A: From AMD website (manual download)
# https://www.xilinx.com/support/download/index.html/content/xilinx/en/downloadNav/embedded-platforms.html
# Look for: XRT 2023.2 → Edge Platforms → Ubuntu 22.04 → ARM64

# Option B: Direct wget (if available)
wget https://www.xilinx.com/bin/public/openDownload?filename=xrt_202320.2.16.204_22.04-arm64-xrt.deb \
     -O xrt_202320.2.16.204_22.04-arm64-xrt.deb

# Verify download
ls -lh xrt_*.deb
# Expected: ~50-100MB .deb file
```

### Step 2: Install XRT Package

```bash
# Update package list
sudo apt update

# Install dependencies
sudo apt install -y \
    ocl-icd-opencl-dev \
    opencl-headers \
    libdrm-dev \
    libelf-dev \
    uuid-dev \
    libboost-all-dev \
    libprotobuf-dev \
    protobuf-compiler

# Install XRT
sudo apt install ./xrt_202320.2.16.204_22.04-arm64-xrt.deb

# If dependency errors occur:
sudo apt --fix-broken install
sudo apt install ./xrt_*.deb
```

### Step 3: Configure Environment

```bash
# Source XRT environment
source /opt/xilinx/xrt/setup.sh

# Add to .bashrc for persistence
echo 'source /opt/xilinx/xrt/setup.sh' >> ~/.bashrc

# Verify environment
echo $XILINX_XRT
# Expected: /opt/xilinx/xrt
```

### Step 4: Load ZOCL Driver

```bash
# Load the ZOCL driver (Zynq OpenCL)
sudo modprobe zocl

# Verify driver loaded
lsmod | grep zocl
# Expected output: zocl  xxxxx  0

# Check device created
ls -l /dev/dri/renderD128
# Expected: crw-rw-rw- 1 root render

# If permissions are wrong:
sudo chmod 666 /dev/dri/renderD128

# To auto-load on boot:
echo "zocl" | sudo tee /etc/modules-load.d/zocl.conf
```

### Step 5: Verify Installation

```bash
# Check XRT version
xbutil --version
# Expected: XRT Build Version: 2.16.204

# Examine devices
xbutil examine
# Should show device information without errors

# Quick API test
cat > /tmp/xrt_test.c << 'EOF'
#include <stdio.h>
#include <xrt/xrt_device.h>

int main() {
    xrtDeviceHandle device = xrtDeviceOpen(0);
    if (!device) {
        fprintf(stderr, "Failed to open device\n");
        return 1;
    }
    printf("SUCCESS: Device opened\n");
    xrtDeviceClose(device);
    return 0;
}
EOF

gcc -o /tmp/xrt_test /tmp/xrt_test.c \
    -I/opt/xilinx/xrt/include \
    -L/opt/xilinx/xrt/lib \
    -lxrt_coreutil

/tmp/xrt_test
# Expected: SUCCESS: Device opened
```

### Step 6: Install WasmEdge

```bash
# Install WasmEdge 0.13.5
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh \
    | bash -s -- -v 0.13.5

# Load into PATH
source ~/.wasmedge/env

# Verify
wasmedge --version
# Expected: WasmEdge version 0.13.5

# Add to .bashrc
echo 'source ~/.wasmedge/env' >> ~/.bashrc
```

### Step 7: Build WASI FPGA Extension

```bash
# Install Rust if not present
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Get the project
cd ~
git clone <repository-url> wasi-fpga-artifact
cd wasi-fpga-artifact/wasi-fpga

# Set XRT paths
export XRT_INCLUDE_PATH=/opt/xilinx/xrt/include
export XRT_LIB_PATH=/opt/xilinx/xrt/lib

# Build
cargo build --release

# Install
sudo cp target/release/libwasi_fpga_extensions.so /usr/local/lib/
sudo ldconfig

# Verify
ldconfig -p | grep wasi_fpga
```

### Step 8: Run Phase 4 Tests

```bash
# Set environment
export WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so
export FPGA_XCLBIN_PATH=/opt/xclbin/BCPNN_infer_float.xclbin
export FPGA_MOCK_EXECUTION=0  # Use real FPGA

# Copy xclbin to expected location
sudo mkdir -p /opt/xclbin
sudo cp /path/to/BCPNN_infer_float.xclbin /opt/xclbin/

# Run test workload
cd ~/wasi-fpga-artifact/workloads/wasm/poc-test
cargo build --target wasm32-wasip1 --release
wasmedge target/wasm32-wasip1/release/poc-test.wasm
```

### Ubuntu 22.04 Troubleshooting

| Issue | Cause | Solution |
|-------|-------|----------|
| `apt: unmet dependencies` | Missing repos | `sudo apt update && sudo apt --fix-broken install` |
| `modprobe: zocl not found` | Driver not in kernel | Rebuild kernel with ZOCL or use DKMS |
| `/dev/dri/renderD128 missing` | ZOCL not loaded | `sudo modprobe zocl` |
| `xbutil: command not found` | XRT not in PATH | `source /opt/xilinx/xrt/setup.sh` |
| `libxrt_coreutil.so not found` | Library path issue | `export LD_LIBRARY_PATH=/opt/xilinx/xrt/lib:$LD_LIBRARY_PATH` |

---

> **Note:** The following sections describe PetaLinux installation, which is still
> supported but more complex. For most development workflows, Ubuntu 22.04 is recommended.

---

## Problem Context

### The Challenge: PetaLinux vs Ubuntu

The original document [wasm_workload_test.md](wasm_workload_test.md#L186-L191) assumes a standard Ubuntu installation:

```bash
# THIS DOESN'T WORK ON PETALINUX
sudo apt install ./xrt_*.deb
```

**Why doesn't it work on PetaLinux?**

| Aspect | Ubuntu | PetaLinux |
|---------|--------|-----------|
| **Package system** | APT/dpkg | Yocto/OpenEmbedded recipes |
| **Binary installation** | .deb packages | Compiled into rootfs |
| **Dependency management** | Runtime (apt install) | Build-time (bitbake recipes) |
| **Architecture** | x86_64 (mostly) | ARM64 (aarch64) |
| **Kernel** | Generic Linux | Custom embedded kernel |

### ZCU104 Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Xilinx Zynq UltraScale+ MPSoC (ZCU104)                     │
├─────────────────────────────────────────────────────────────┤
│  Processing System (PS)                                      │
│  ├─ Quad-core ARM Cortex-A53 @ 1.2GHz                       │
│  ├─ Dual-core ARM Cortex-R5F (Real-time)                    │
│  ├─ ARM Mali-400 GPU                                         │
│  └─ DDR4 Memory Controller (2GB)                             │
├─────────────────────────────────────────────────────────────┤
│  Programmable Logic (PL/FPGA)                                │
│  ├─ 230,400 LUTs                                             │
│  ├─ 460,800 Flip-Flops                                       │
│  ├─ 663 DSP Slices                                           │
│  └─ 312 Block RAMs (11.1 Mb)                                 │
└─────────────────────────────────────────────────────────────┘
                         ↕ AXI Interconnect
┌─────────────────────────────────────────────────────────────┐
│  Software Stack                                              │
│  ├─ PetaLinux OS (Yocto-based)                               │
│  ├─ XRT (Xilinx Runtime) ← THIS IS WHAT WE INSTALL          │
│  ├─ ZOCL Driver (Zynq OpenCL)                                │
│  └─ WASI FPGA Extensions (Our code)                          │
└─────────────────────────────────────────────────────────────┘
```

**Official References:**
- **ZCU104 Board User Guide**: https://docs.amd.com/v/u/en-US/ug1267-zcu104-eval-bd
- **Zynq UltraScale+ Architecture**: https://docs.amd.com/r/en-US/ug1085-zynq-ultrascale-trm

---

## System Architecture

### Complete Software Stack

```
┌────────────────────────────────────────────────────────────┐
│  WASM Workload (poc-test.wasm)                             │
│  - Neuromorphic inference module                           │
│  - Imports WASI FPGA functions                             │
└────────────────────────────────────────────────────────────┘
                         ↓ WASI Imports
┌────────────────────────────────────────────────────────────┐
│  WasmEdge Runtime 0.13.5                                   │
│  - Loads WASI extension library                            │
│  - Manages WASM sandbox                                    │
└────────────────────────────────────────────────────────────┘
                         ↓ dlopen()
┌────────────────────────────────────────────────────────────┐
│  WASI FPGA Extensions (libwasi_fpga_extensions.so)        │
│  - 6 host functions: init, alloc, write, read, exec, free │
│  - Safe Rust wrappers                                      │
└────────────────────────────────────────────────────────────┘
                         ↓ FFI bindings
┌────────────────────────────────────────────────────────────┐
│  Xilinx Runtime (XRT) - libxrt_coreutil.so                │
│  - Device management (xrtDeviceOpen/Close)                 │
│  - Buffer operations (xrtBOAlloc/Free/Write/Read/Sync)     │
│  - Kernel execution (xrtKernelOpen/xrtRunStart)            │
└────────────────────────────────────────────────────────────┘
                         ↓ ioctl()
┌────────────────────────────────────────────────────────────┐
│  ZOCL Kernel Driver (zocl.ko)                              │
│  - Character device: /dev/dri/renderD128                   │
│  - DRM subsystem integration                               │
│  - DMA buffer management (CMA)                             │
└────────────────────────────────────────────────────────────┘
                         ↓ Hardware access
┌────────────────────────────────────────────────────────────┐
│  FPGA Hardware (ZCU104 Programmable Logic)                 │
│  - Neuromorphic accelerator kernels                        │
│  - AXI interfaces to PS                                    │
└────────────────────────────────────────────────────────────┘
```

### Components We Need to Install

| Component | Size | Location | Description |
|------------|--------|-----------|-------------|
| **libxrt_coreutil.so** | ~5MB | /usr/lib/ | XRT core library |
| **xrt headers** | ~2MB | /usr/include/xrt/ | C/C++ API headers |
| **zocl.ko** | ~500KB | /lib/modules/.../kernel/drivers/ | Kernel driver |
| **xbutil** | ~2MB | /usr/bin/ | CLI management tool |
| **xbmgmt** | ~2MB | /usr/bin/ | CLI configuration tool (optional) |

**Total:** ~12-15MB of rootfs space

---

## Option 1: Use Xilinx Meta-Layer (Recommended)

This is the **official and most robust** way to integrate XRT into PetaLinux.

### Advantages of This Method

- Officially maintained by Xilinx/AMD
- All dependencies resolved automatically
- Integrated into system build (not post-installation)
- Kernel and drivers synchronized
- Upgrades managed by meta-layer updates

### Prerequisites

- **PetaLinux Tools 2023.1** or higher installed
- **Disk space:** ~50GB free for build
- **RAM:** 16GB minimum (32GB recommended)
- **CPU:** Multicore recommended (build can take 1-3 hours)

### Step 1: Official Documentation

**Essential Reference Links:**

1. **XRT Embedded Build Guide**
   https://xilinx.github.io/XRT/master/html/embedded.html

2. **Meta-Xilinx GitHub Repository**
   https://github.com/Xilinx/meta-xilinx

3. **Meta-Xilinx-Tools (contains XRT recipes)**
   https://github.com/Xilinx/meta-xilinx-tools

4. **PetaLinux Tools Reference Guide**
   https://docs.amd.com/r/en-US/ug1144-petalinux-tools-reference-guide

5. **Yocto/OpenEmbedded Manual**
   https://docs.yoctoproject.org/

### Step 2: Configure PetaLinux Project

```bash
# 1. Navigate to your existing PetaLinux project
cd /path/to/your/petalinux-project

# 2. Verify PetaLinux version
petalinux-build --help | head -1
# Should show: PetaLinux Tools 2023.1 or higher

# 3. If you don't have a project, create a new one for ZCU104
petalinux-create --type project --template zynqMP --name zcu104-xrt-project
cd zcu104-xrt-project

# 4. Configure for ZCU104
petalinux-config --get-hw-description=/path/to/zcu104_base.xsa
# The .xsa file contains the hardware description (PL + PS)
```

**Note about Hardware Description (.xsa):**
- The `.xsa` file defines the FPGA configuration
- For ZCU104, you can use the base design or your own custom design
- Typical location: `vivado_project/project_name.sdk/system_top.xsa`

### Step 3: Add Meta-Xilinx-Tools to Build

```bash
# Edit layer configuration
nano project-spec/meta-user/conf/layer.conf

# Add at the end:
# Meta-layer dependencies
BBFILES += "${LAYERDIR}/recipes-*/*/*.bb \
            ${LAYERDIR}/recipes-*/*/*.bbappend"

BBFILE_COLLECTIONS += "meta-user"
BBFILE_PATTERN_meta-user = "^${LAYERDIR}/"
BBFILE_PRIORITY_meta-user = "7"
```

### Step 4: Configure RootFS for XRT

```bash
# Edit rootfs configuration
petalinux-config -c rootfs

# In the interactive menu, navigate to:
# Filesystem Packages → misc →
#   [*] packagegroup-petalinux-xrt
#   [*] xrt
#   [*] xrt-dev
#   [*] zocl
#   [*] opencl-headers
#   [*] opencl-clhpp-dev

# Alternatively, edit the file directly:
nano project-spec/configs/rootfs_config

# Add these lines:
CONFIG_packagegroup-petalinux-xrt=y
CONFIG_xrt=y
CONFIG_xrt-dev=y
CONFIG_zocl=y
CONFIG_opencl-headers=y
CONFIG_opencl-clhpp-dev=y

# If you need development tools on target:
CONFIG_packagegroup-petalinux-self-hosted=y
CONFIG_cmake=y
CONFIG_git=y
CONFIG_gcc=y
CONFIG_g++=y
CONFIG_make=y

# For debugging:
CONFIG_gdb=y
CONFIG_strace=y
CONFIG_ldd=y
```

**Package Explanation:**

| Package | Description | Needed for |
|---------|-------------|----------------|
| `xrt` | Runtime libraries | Application execution |
| `xrt-dev` | Headers and static libs | Native compilation |
| `zocl` | Kernel driver | FPGA access from userspace |
| `opencl-headers` | OpenCL API headers | OpenCL app compilation |
| `opencl-clhpp-dev` | C++ OpenCL bindings | C++ apps (optional) |
| `packagegroup-petalinux-xrt` | Meta-package | Installs all XRT |

### Step 5: Configure Kernel for ZOCL

```bash
# Configure Linux kernel
petalinux-config -c kernel

# In menuconfig, navigate and enable:
# Device Drivers --->
#   Accelerators --->
#     <M> Xilinx FPGA Device Drivers --->
#       <M> Xilinx ZOCL Driver Support
#       <M> Xilinx DRM ZoCL Driver

# DMA Engine support:
# Device Drivers --->
#   DMA Engine support --->
#     [*] DMA Engine support
#     <M> Xilinx AXI DMA Engine
#     <M> Xilinx VDMA Engine

# DRM support (needed for /dev/dri/renderD*):
# Device Drivers --->
#   Graphics support --->
#     <M> Direct Rendering Manager (XFree86 4.1.0 and higher DRI support)
#     [*] Enable legacy fbdev support for your modesetting driver
```

**Critical CMA (Contiguous Memory Allocator) Configuration:**

The ZOCL driver needs contiguous physical memory for DMA buffers.

```bash
# Configure CMA size
petalinux-config -c kernel

# Navigate to:
# Library routines --->
#   DMA Contiguous Memory Allocator --->
#     (512) Size in Mega Bytes
#     [*] Enable CMA

# Alternatively, edit device tree:
nano project-spec/meta-user/recipes-bsp/device-tree/files/system-user.dtsi

# Add:
/include/ "system-conf.dtsi"
/ {
    reserved-memory {
        #address-cells = <2>;
        #size-cells = <2>;
        ranges;

        /* Reserve 512MB for CMA */
        linux,cma {
            compatible = "shared-dma-pool";
            reusable;
            size = <0x0 0x20000000>; /* 512MB in hex */
            alignment = <0x0 0x2000>;
            linux,cma-default;
        };
    };
};
```

**CMA Documentation:**
https://xilinx-wiki.atlassian.net/wiki/spaces/A/pages/18841683/Linux+Reserved+Memory

### Step 6: Build System

```bash
# Clean previous build (optional but recommended)
petalinux-build -c rootfs -x mrproper

# Complete system build
# ⏱️ Estimated time: 1-3 hours on modern machine
petalinux-build

# Monitor progress (in another terminal)
tail -f build/build.log

# The build will:
# 1. Compile kernel with ZOCL driver
# 2. Cross-compile XRT for ARM64
# 3. Generate rootfs with all dependencies
# 4. Create bootable images
```

**Expected Output:**

```
INFO: build/tmp/deploy/images/zcu104-zynqmp/
  ├── Image                    (Kernel image ~15MB)
  ├── rootfs.tar.gz           (RootFS with XRT ~500MB)
  ├── rootfs.cpio.gz.u-boot   (RootFS for U-Boot)
  ├── boot.scr                (U-Boot script)
  └── system.dtb              (Device tree blob)
```

### Step 7: Generate Bootable Image

```bash
# Generate BOOT.BIN (contains FSBL, PMU FW, U-Boot, bitstream)
petalinux-package --boot \
  --fsbl images/linux/zynqmp_fsbl.elf \
  --u-boot images/linux/u-boot.elf \
  --pmufw images/linux/pmufw.elf \
  --fpga images/linux/system.bit \
  --force

# Output: images/linux/BOOT.BIN
```

**BOOT.BIN Components:**

| Component | Description |
|------------|-------------|
| **FSBL** (First Stage Boot Loader) | Initializes PS, DDR, clocks |
| **PMU FW** (Platform Management Unit) | Power management, reset control |
| **U-Boot** | Secondary bootloader, loads kernel |
| **system.bit** | FPGA bitstream (PL configuration) |

**Boot Process Guide:**
https://docs.amd.com/r/en-US/ug1137-zynq-ultrascale-mpsoc-swdev/Zynq-UltraScale-MPSoC-Boot-Sequence

### Step 8: Prepare SD Card

```bash
# Option A: Use WIC image (recommended)
petalinux-package --wic --images-dir images/linux/ \
  --bootfiles "BOOT.BIN boot.scr Image system.dtb" \
  --disk-name "sda"  # Change according to your SD card

# Write to SD card (⚠️ WARNING: destroys data on /dev/sdX)
sudo dd if=images/linux/petalinux-sdimage.wic of=/dev/sdX bs=4M status=progress
sync

# Option B: Manual partitioning
# Create 2 partitions:
sudo fdisk /dev/sdX
# Partition 1: 500MB, FAT32, bootable (BOOT)
# Partition 2: Remainder, ext4 (ROOTFS)

# Format
sudo mkfs.vfat -n BOOT /dev/sdX1
sudo mkfs.ext4 -L ROOTFS /dev/sdX2

# Mount
sudo mkdir -p /mnt/sd_{boot,root}
sudo mount /dev/sdX1 /mnt/sd_boot
sudo mount /dev/sdX2 /mnt/sd_root

# Copy boot files
sudo cp images/linux/BOOT.BIN /mnt/sd_boot/
sudo cp images/linux/boot.scr /mnt/sd_boot/
sudo cp images/linux/Image /mnt/sd_boot/
sudo cp images/linux/system.dtb /mnt/sd_boot/

# Extract rootfs
sudo tar xzf images/linux/rootfs.tar.gz -C /mnt/sd_root/

# Verify XRT is installed
ls /mnt/sd_root/usr/lib/libxrt_coreutil.so
ls /mnt/sd_root/usr/include/xrt/

# Unmount
sudo umount /mnt/sd_boot /mnt/sd_root
```

### Step 9: First Boot

```bash
# 1. Insert SD card into ZCU104
# 2. Connect UART (USB micro, 115200 8N1)
# 3. Open serial terminal:
sudo screen /dev/ttyUSB0 115200
# Or use minicom:
sudo minicom -D /dev/ttyUSB0 -b 115200

# 4. Power on board (SW6)
# 5. Observe boot sequence:
```

**Expected Boot Output:**

```
Xilinx Zynq MP First Stage Boot Loader
Release 2023.1   Nov 20 2025 - 10:30:00

U-Boot 2023.01 (Nov 20 2025 - 10:30:15 +0000)

Model: ZCU104 RevC
Board: Xilinx ZynqMP
DRAM:  2 GiB

...

Starting kernel ...

[    0.000000] Booting Linux on physical CPU 0x0000000000 [0x410fd034]
[    0.000000] Linux version 6.1.0-xilinx-v2023.1 (oe-user@oe-host)
...
[    2.345678] zocl: loading out-of-tree module taints kernel.
[    2.351234] zocl: Xilinx ZOCL Driver Version: 2023.1
[    2.356789] zocl 80000000.zyxclmm_drm: ZOCL device registered

...

PetaLinux 2023.1 zcu104-zynqmp /dev/ttyPS0

zcu104-zynqmp login: root
Password: root
root@zcu104-zynqmp:~#
```

---

## Option 2: Manual XRT Compilation for ARM

If you can't use meta-xilinx (e.g., highly customized PetaLinux), you can manually compile XRT.

### Advantages and Disadvantages

**Advantages:**
- Complete build control
- Doesn't require modifying meta-layers
- Useful for XRT debugging

**Disadvantages:**
- More complex to maintain
- Manual dependencies
- Potential kernel incompatibilities

### Step 1: Prepare Cross-Compilation Environment

```bash
# On your development machine (not on ZCU104)

# 1. Generate PetaLinux SDK
cd /path/to/petalinux-project
petalinux-build --sdk

# This generates SDK installer:
# images/linux/sdk.sh (~500MB)

# 2. Install SDK
./images/linux/sdk.sh -d /opt/petalinux-sdk-2023.1

# 3. Source environment
source /opt/petalinux-sdk-2023.1/environment-setup-cortexa72-cortexa53-xilinx-linux

# 4. Verify cross-compiler
$CC --version
# Should show: aarch64-xilinx-linux-gcc

echo $SDKTARGETSYSROOT
# Should show: /opt/petalinux-sdk-2023.1/sysroots/cortexa72-cortexa53-xilinx-linux
```

**SDK Generation Guide:**
https://docs.amd.com/r/en-US/ug1144-petalinux-tools-reference-guide/Building-SDK-Installer

### Step 2: Clone and Configure XRT

```bash
# 1. Clone XRT repository
git clone https://github.com/Xilinx/XRT.git
cd XRT

# 2. Checkout specific version
git checkout 2023.1
# ⚠️ IMPORTANT: Use same version as PetaLinux tools

# 3. Review dependencies
cat src/runtime_src/tools/scripts/aarch64.Dockerfile
# This shows required dependencies

# 4. Install dependencies in SDK sysroot
# (These should be in PetaLinux rootfs)
```

**XRT Dependencies:**

```bash
# In PetaLinux project, add to rootfs:
petalinux-config -c rootfs

# Enable:
CONFIG_boost=y
CONFIG_boost-dev=y
CONFIG_libdrm=y
CONFIG_libdrm-dev=y
CONFIG_opencl-headers=y
CONFIG_uuid=y
CONFIG_libuuid-dev=y
CONFIG_protobuf=y
CONFIG_protobuf-dev=y
CONFIG_libcurl=y
CONFIG_libcurl-dev=y
CONFIG_ncurses=y
CONFIG_ncurses-dev=y

# Rebuild rootfs to generate new SDK
petalinux-build -c rootfs
petalinux-build --sdk
```

### Step 3: Cross-Compile XRT

```bash
# Return to XRT directory
cd XRT/build

# Configure build for ARM64 embedded
./build.sh -opt -arm64 -j$(nproc)

# Important flags:
# -opt          : Optimized build (Release mode)
# -arm64        : Target ARM64 architecture
# -j$(nproc)    : Parallel build (uses all cores)

# ⏱️ Time: 30-60 minutes depending on your machine
```

**Build Troubleshooting:**

If you encounter dependency errors:

```bash
# Error: boost not found
# Solution: Install boost in sysroot
cd $SDKTARGETSYSROOT
wget https://boostorg.jfrog.io/artifactory/main/release/1.74.0/source/boost_1_74_0.tar.gz
tar xzf boost_1_74_0.tar.gz
cd boost_1_74_0
./bootstrap.sh --prefix=$SDKTARGETSYSROOT/usr
./b2 install

# Error: protobuf not found
# Solution: Cross-compile protobuf
git clone https://github.com/protocolbuffers/protobuf.git
cd protobuf
./autogen.sh
./configure --host=aarch64-xilinx-linux --prefix=$SDKTARGETSYSROOT/usr
make -j$(nproc)
make install
```

### Step 4: Package XRT for Target

```bash
# After successful build
cd XRT/build/Release

# Important files are in:
ls -lh
# xrt/
# ├── bin/           (xbutil, xbmgmt)
# ├── lib/           (libxrt_coreutil.so, etc.)
# ├── include/       (xrt/*.h headers)
# └── share/         (firmware, scripts)

# Create tarball to transfer to ZCU104
tar czf xrt-2023.1-zcu104-aarch64.tar.gz \
  --transform 's,^,xrt/,' \
  xrt/bin/* \
  xrt/lib/* \
  xrt/include/* \
  xrt/share/*

# Copy to target (via SCP or SD card)
scp xrt-2023.1-zcu104-aarch64.tar.gz root@<ZCU104-IP>:/tmp/
```

### Step 5: Install on ZCU104

```bash
# On ZCU104
cd /tmp
tar xzf xrt-2023.1-zcu104-aarch64.tar.gz -C /

# Verify installation
ls -lh /usr/local/lib/libxrt_coreutil.so
ls /usr/local/include/xrt/

# Configure library path
echo "/usr/local/lib" > /etc/ld.so.conf.d/xrt.conf
ldconfig

# Verify linking
ldconfig -p | grep xrt
# Should show: libxrt_coreutil.so => /usr/local/lib/libxrt_coreutil.so
```

### Step 6: Compile and Install ZOCL Driver

The ZOCL driver must be compiled against the ZCU104 kernel.

```bash
# On development machine
cd XRT/src/runtime_src/core/edge/drm/zocl

# Configure for PetaLinux kernel
export KERNEL_SRC=/path/to/petalinux-project/build/tmp/work-shared/zcu104-zynqmp/kernel-source
export ARCH=arm64
export CROSS_COMPILE=aarch64-xilinx-linux-

# Compile module
make -C $KERNEL_SRC M=$PWD modules

# Copy to target
scp zocl.ko root@<ZCU104-IP>:/lib/modules/$(uname -r)/kernel/drivers/gpu/drm/

# On ZCU104
depmod -a
modprobe zocl

# Verify load
lsmod | grep zocl
dmesg | tail -20 | grep zocl
```

---

## Option 3: Post-Boot Installation (Simpler, Less Ideal)

**WARNING:** This option is for **quick testing only**. Not recommended for production.

### Advantages and Disadvantages

**Advantages:**
- Faster for experimentation
- Doesn't require PetaLinux rebuild

**Disadvantages:**
- Consumes much disk space (~2GB)
- Very slow compilation on ARM (4-6 hours)
- Requires development tools on target
- Not persistent between rootfs rebuilds

### Step 1: Prepare Target with Build Tools

```bash
# In PetaLinux project, enable self-hosted development
petalinux-config -c rootfs

# Enable complete packagegroup:
CONFIG_packagegroup-petalinux-self-hosted=y

# Or enable individual packages:
CONFIG_git=y
CONFIG_cmake=y
CONFIG_gcc=y
CONFIG_g++=y
CONFIG_make=y
CONFIG_autoconf=y
CONFIG_automake=y
CONFIG_libtool=y
CONFIG_pkg-config=y

# XRT dependencies:
CONFIG_boost-dev=y
CONFIG_libdrm-dev=y
CONFIG_libuuid-dev=y
CONFIG_protobuf-dev=y
CONFIG_libcurl-dev=y
CONFIG_ncurses-dev=y

# Rebuild
petalinux-build -c rootfs
```

### Step 2: Expand RootFS Partition (if needed)

```bash
# On ZCU104, check space
df -h
# Filesystem      Size  Used Avail Use% Mounted on
# /dev/mmcblk0p2  7.2G  1.5G  5.4G  22% /

# If you need more space, expand SD card:
# (Do this BEFORE compiling XRT)

# 1. Expand partition (use fdisk/parted)
sudo fdisk /dev/mmcblk0
# d (delete partition 2)
# n (new partition, use all space)
# w (write)

# 2. Reboot
sudo reboot

# 3. Expand filesystem
sudo resize2fs /dev/mmcblk0p2

# 4. Verify
df -h
# Should show full SD card space
```

### Step 3: Clone and Compile XRT on Target

```bash
# ⚠️ WARNING: SLOW process (4-6 hours on ARM Cortex-A53)

# On ZCU104
cd /home/root

# 1. Clone XRT
git clone https://github.com/Xilinx/XRT.git
cd XRT
git checkout 2023.1

# 2. Configure build
cd build

# 3. Start compilation (uses all cores)
./build.sh -opt -j4

# Monitor temperature (board will heat up):
watch -n 5 'cat /sys/class/hwmon/hwmon0/temp1_input'
# If exceeds 85000 (85°C), reduce to -j2 or pause

# 4. Wait... (4-6 hours)
```

**Optimize ARM Compilation:**

```bash
# Use tmpfs for /tmp (reduces SD writes)
sudo mount -t tmpfs -o size=2G tmpfs /tmp

# Disable unnecessary features
cd XRT/build
# Edit: CMakeLists.txt
# Comment out: XRT_ENABLE_AI_DOMAIN, XRT_ENABLE_XDP, etc.

# Minimal build (runtime only, no debug tools)
./build.sh -opt -noert -j4
```

### Step 4: Install Compiled XRT

```bash
# After successful build
cd XRT/build/Release

# Install (as root)
sudo ./build.sh -opt -install

# Verify installation
ls -lh /opt/xilinx/xrt/lib/libxrt_coreutil.so
ls /opt/xilinx/xrt/include/xrt/

# Configure environment
echo "source /opt/xilinx/xrt/setup.sh" >> ~/.bashrc
source ~/.bashrc

# Verify
echo $XILINX_XRT
# Should show: /opt/xilinx/xrt
```

---

## Installation Verification

Regardless of installation method, follow these steps to verify.

### Verification Level 1: System Files

```bash
# 1. Verify main XRT library
ls -lh /usr/lib/libxrt_coreutil.so*
# Expected output:
# lrwxrwxrwx 1 root root   25 Nov 20 10:00 libxrt_coreutil.so -> libxrt_coreutil.so.2.16.0
# -rwxr-xr-x 1 root root 4.5M Nov 20 10:00 libxrt_coreutil.so.2.16.0

# 2. Verify XRT headers
ls /usr/include/xrt/
# Expected output:
# xrt_device.h  xrt_bo.h  xrt_kernel.h  xrt_aie.h  xrt_graph.h  xrt.h

# 3. Verify CLI tools
which xbutil xbmgmt
# Expected output:
# /usr/bin/xbutil
# /usr/bin/xbmgmt

# 4. Verify ZOCL driver
lsmod | grep zocl
# Expected output:
# zocl                  123456  0
# drm                   345678  1 zocl

# If not loaded:
sudo modprobe zocl

# Verify kernel messages
dmesg | grep -i zocl
# Expected output:
# [    2.345678] zocl: loading out-of-tree module taints kernel.
# [    2.351234] zocl: Xilinx ZOCL Driver Version: 2023.1
# [    2.356789] zocl 80000000.zyxclmm_drm: ZOCL device registered
# [    2.362345] [drm] Initialized zocl 2023.1.0 20230101 for 80000000.zyxclmm_drm on minor 0
```

### Verification Level 2: Hardware Devices

```bash
# 1. Verify DRM device (render node)
ls -l /dev/dri/
# Expected output:
# total 0
# drwxr-xr-x 2 root root       80 Nov 20 10:01 by-path
# crw-rw---- 1 root video 226, 0 Nov 20 10:01 card0
# crw-rw-rw- 1 root render 226, 128 Nov 20 10:01 renderD128

# IMPORTANT: renderD128 must have 0666 permissions (rw-rw-rw-)
# If not:
sudo chmod 666 /dev/dri/renderD128

# 2. Verify UIO devices (User I/O)
ls -l /dev/uio*
# May or may not exist depending on configuration

# 3. Verify sysfs device
ls /sys/class/drm/card0/device/
# Should contain files like: vendor, device, subsystem_vendor, etc.

cat /sys/class/drm/card0/device/vendor
# Expected output: 0x10ee (Xilinx vendor ID)

# 4. Verify PCI device (if applicable)
lspci -d 10ee:
# May show nothing on Zynq (not traditional PCI)
# On ZCU104, device is memory-mapped, not PCI
```

### Verification Level 3: XRT Utilities

```bash
# 1. Examine devices with xbutil
xbutil examine

# Expected output:
# -----------------------------------------------
#  System Configuration
# -----------------------------------------------
# OS Name              : Linux
# Release              : 6.1.0-xilinx-v2023.1
# ...
#
# Devices present
# [0] 0000:00:00.0 xilinx_zcu104_base Shell:<not supported>
#
# -----------------------------------------------
#  Device: [0]
# -----------------------------------------------
# ...
# Status               : Ready

# 2. Detailed device information
xbutil examine -d 0 -r platform

# 3. Verify memory
xbutil examine -d 0 -r memory

# Expected output:
# Memory Topology
#   Tag           Type    Temp    Size        Mem Usage    BO Count
#   bank0         DDR     N/A     2048 MB     0 MB         0
```

**xbutil Reference:**
https://xilinx.github.io/XRT/master/html/xbutil.html

### Verification Level 4: Basic XRT API Test

Create a simple test program:

```bash
# Create test file
cat > /tmp/xrt_test.c << 'EOF'
#include <stdio.h>
#include <stdlib.h>
#include <xrt/xrt_device.h>

int main() {
    printf("Testing XRT device access...\n");

    // Try to open device 0
    xrtDeviceHandle device = xrtDeviceOpen(0);
    if (!device) {
        fprintf(stderr, "ERROR: Failed to open device 0\n");
        return 1;
    }

    printf("SUCCESS: Device 0 opened successfully\n");

    // Close device
    xrtDeviceClose(device);
    printf("Device closed\n");

    return 0;
}
EOF

# Compile
gcc -o /tmp/xrt_test /tmp/xrt_test.c -I/usr/include -L/usr/lib -lxrt_coreutil

# Execute
/tmp/xrt_test

# Expected output:
# Testing XRT device access...
# SUCCESS: Device 0 opened successfully
# Device closed
```

### Verification Level 5: CMA (Contiguous Memory)

```bash
# Verify available CMA
cat /proc/meminfo | grep Cma
# Expected output:
# CmaTotal:         524288 kB  (512 MB)
# CmaFree:          520000 kB  (~508 MB free)

# Verify assigned DMA buffers
cat /sys/kernel/debug/dma_buf/bufinfo
# Shows active DMA buffers

# If CmaTotal is 0 or very small:
# You need to reconfigure device tree or kernel cmdline
# See "Advanced Configuration" section below
```

### Verification Level 6: Permissions and Udev Rules

```bash
# Verify user groups
groups
# Should include: video, render

# If not:
sudo usermod -a -G video,render $USER
# Logout and login again

# Create udev rules for automatic permissions
cat > /tmp/99-xrt.rules << 'EOF'
# XRT device permissions
SUBSYSTEM=="drm", KERNEL=="renderD*", MODE="0666"
SUBSYSTEM=="drm", KERNEL=="card*", MODE="0666"
SUBSYSTEM=="uio", MODE="0666"
EOF

sudo cp /tmp/99-xrt.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger
```

---

## Building the WASI FPGA Extension

Once XRT is installed and verified, compile the WASI extension.

### Step 1: Install Rust on ZCU104

```bash
# Download and install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Options during installation:
# 1) Proceed with installation (default)

# Load environment
source $HOME/.cargo/env

# Verify installation
rustc --version
# Output: rustc 1.75.0 (stable)

cargo --version
# Output: cargo 1.75.0

# Configure Rust to optimize for ARM64
cat >> ~/.cargo/config.toml << 'EOF'
[build]
rustflags = ["-C", "target-cpu=native"]

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
EOF
```

**Rust Installation Guide:**
https://doc.rust-lang.org/book/ch01-01-installation.html

### Step 2: Get Project Code

```bash
# Option A: Clone via Git (if ZCU104 has internet)
cd /home/root
git clone <artifact-repository-url> wasi-fpga-artifact
cd wasi-fpga-artifact

# Option B: Transfer via SCP
# On your development machine:
cd /path/to/wasi-fpga-artifact
tar czf wasi-fpga-artifact.tar.gz \
  --exclude=target \
  --exclude=.git \
  wasi-fpga/ workloads/ README_AE.md ARTIFACT_MANIFEST.md install_xrt.md

scp wasi-fpga-artifact.tar.gz root@<ZCU104-IP>:/home/root/

# On ZCU104:
cd /home/root
tar xzf wasi-fpga-artifact.tar.gz
cd wasi-fpga-artifact
```

### Step 3: Configure Environment Variables for XRT

```bash
# Configure XRT paths for bindgen
export XRT_INCLUDE_PATH=/usr/include
export XRT_LIB_PATH=/usr/lib
export LD_LIBRARY_PATH=/usr/lib:$LD_LIBRARY_PATH

# Optional: If XRT is in /opt/xilinx/xrt:
export XRT_INCLUDE_PATH=/opt/xilinx/xrt/include
export XRT_LIB_PATH=/opt/xilinx/xrt/lib
export LD_LIBRARY_PATH=/opt/xilinx/xrt/lib:$LD_LIBRARY_PATH
source /opt/xilinx/xrt/setup.sh

# Verify bindgen can find XRT
ls $XRT_INCLUDE_PATH/xrt/xrt_device.h
# Should exist

ls $XRT_LIB_PATH/libxrt_coreutil.so
# Should exist
```

### Step 4: Install Build Dependencies

```bash
# bindgen needs libclang
# In PetaLinux, this should be in rootfs:
petalinux-config -c rootfs
# Enable: CONFIG_clang=y CONFIG_llvm=y

# Or install manually if you have package manager:
# (Depends on your PetaLinux configuration)

# Verify clang is available
which clang
llvm-config --version
```

### Step 5: Compile WASI Extension

```bash
cd /home/root/wasi-fpga-artifact/wasi-fpga

# First compilation (downloads dependencies)
# ⏱️ Time: 10-20 minutes on ARM
cargo build --release

# Monitor progress
# In another terminal:
tail -f ~/.cargo/registry/CACHEDIR.TAG

# If build fails with "out of memory":
# Reduce parallelism
cargo build --release -j 1

# Or use swap file:
sudo dd if=/dev/zero of=/swapfile bs=1M count=2048
sudo mkswap /swapfile
sudo chmod 600 /swapfile
sudo swapon /swapfile
```

**Expected Output:**

```
   Compiling libc v0.2.150
   Compiling log v0.4.20
   Compiling thiserror v1.0.50
   ...
   Compiling wasmedge-sdk v0.13.0
   Compiling wasi-fpga v0.1.0 (/home/root/wasi-fpga-artifact/wasi-fpga)
    Finished release [optimized] target(s) in 12m 34s
```

### Step 6: Verify Compiled Library

```bash
# Verify output
ls -lh target/release/libwasi_fpga_extensions.so
# Expected output:
# -rwxr-xr-x 1 root root 3.2M Nov 20 15:30 target/release/libwasi_fpga_extensions.so

# Verify dependencies
ldd target/release/libwasi_fpga_extensions.so

# Expected output:
# linux-vdso.so.1 (0x0000ffff8e3f0000)
# libxrt_coreutil.so => /usr/lib/libxrt_coreutil.so (0x0000ffff8e100000)
# libgcc_s.so.1 => /lib/libgcc_s.so.1 (0x0000ffff8e0d0000)
# libc.so.6 => /lib/libc.so.6 (0x0000ffff8df50000)
# ...

# IMPORTANT: All dependencies must resolve (no "not found")

# Verify exported symbols
nm -D target/release/libwasi_fpga_extensions.so | grep fpga_
# Expected output:
# 00000000000ab123 T fpga_alloc_buffer
# 00000000000ab456 T fpga_execute_kernel
# 00000000000ab789 T fpga_free_buffer
# 00000000000ababc T fpga_init
# 00000000000abdef T fpga_read_buffer
# 00000000000ac012 T fpga_write_buffer
```

### Step 7: Install Library

```bash
# Copy to standard location
sudo cp target/release/libwasi_fpga_extensions.so /usr/local/lib/

# Update ldconfig cache
sudo ldconfig

# Verify it's registered
ldconfig -p | grep wasi_fpga
# Expected output:
# libwasi_fpga_extensions.so (libc6,AArch64) => /usr/local/lib/libwasi_fpga_extensions.so

# Verify permissions
ls -l /usr/local/lib/libwasi_fpga_extensions.so
# Should be: -rwxr-xr-x (executable and readable by all)
```

### Step 8: Quick Library Load Test

```bash
# Verify it can be loaded dynamically
cat > /tmp/test_load.c << 'EOF'
#include <stdio.h>
#include <dlfcn.h>

int main() {
    void *handle = dlopen("/usr/local/lib/libwasi_fpga_extensions.so", RTLD_LAZY);
    if (!handle) {
        fprintf(stderr, "Error: %s\n", dlerror());
        return 1;
    }
    printf("SUCCESS: Library loaded\n");
    dlclose(handle);
    return 0;
}
EOF

gcc -o /tmp/test_load /tmp/test_load.c -ldl
/tmp/test_load

# Expected output:
# SUCCESS: Library loaded
```

---

## PoC Test Execution

Now run the end-to-end validation test.

### Step 1: Install WasmEdge Runtime

```bash
# Automatic installation for ARM64
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -v 0.13.5

# ⏱️ Time: 2-5 minutes

# Load into PATH
source $HOME/.wasmedge/env

# Verify installation
wasmedge --version
# Expected output: WasmEdge version 0.13.5

# Verify location
which wasmedge
# Output: /home/root/.wasmedge/bin/wasmedge

# Basic test
wasmedge --help | head -10
```

**WasmEdge Installation:**
https://wasmedge.org/docs/start/install

**WasmEdge CLI Reference:**
https://wasmedge.org/docs/embed/cli/run

### Step 2: Compile WASM Module

**Option A: Compile on development machine (recommended)**

```bash
# On your x86_64 development machine (NOT on ZCU104)
cd /path/to/wasi-fpga-artifact/workloads/wasm/poc-test

# Install WASM target
rustup target add wasm32-wasip1

# Compile
cargo build --target wasm32-wasip1 --release

# Optimize with wasm-opt (optional but recommended)
# Install wasm-opt:
# Ubuntu: sudo apt install binaryen
# macOS: brew install binaryen
wasm-opt -Oz \
  target/wasm32-wasip1/release/poc-test.wasm \
  -o poc-test.wasm

# Verify size
ls -lh poc-test.wasm
# Should be ~30-50KB

# Transfer to ZCU104
scp poc-test.wasm root@<ZCU104-IP>:/home/root/
```

**Option B: Compile on ZCU104**

```bash
# On ZCU104
cd /home/root/wasi-fpga-artifact/workloads/wasm/poc-test

# Install WASM target
rustup target add wasm32-wasip1

# Compile
# ⏱️ Time: 2-5 minutes
cargo build --target wasm32-wasip1 --release

# wasm-opt probably not available on ARM
# You can skip optimization or install it manually

# Use compiled WASM directly
cp target/wasm32-wasip1/release/poc-test.wasm /home/root/
```

### Step 3: Prepare Execution Environment

```bash
# Verify all required components
cd /home/root

# 1. WASM module
ls -lh poc-test.wasm
# Should exist (~30-50KB)

# 2. WASI extension library
ls -lh /usr/local/lib/libwasi_fpga_extensions.so
# Should exist (~3-5MB)

# 3. XRT library
ls -lh /usr/lib/libxrt_coreutil.so
# Should exist (~4-5MB)

# 4. FPGA device
ls -l /dev/dri/renderD128
# Should exist with rw-rw-rw- permissions

# 5. ZOCL driver
lsmod | grep zocl
# Should be loaded
```

### Step 4: Run PoC Test

```bash
# Execute with verbose logging
RUST_LOG=debug \
  wasmedge \
  --env WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so \
  poc-test.wasm

# Or without debug logging:
wasmedge \
  --env WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so \
  poc-test.wasm
```

### Step 5: Interpret Results

**Expected Output (COMPLETE SUCCESS):**

```
========================================
  WASM FPGA PoC Test - Week 1 Day 3
========================================

[1/6] Testing fpga_init()...
PASSED: FPGA initialized

[2/6] Testing fpga_alloc_buffer(1024)...
PASSED: Buffer allocated (ID=1)

[3/6] Testing fpga_write_buffer()...
PASSED: Wrote 256 bytes to buffer

[4/6] Testing fpga_read_buffer()...
PASSED: Read 256 bytes from buffer

[5/6] Testing data integrity...
PASSED: Data integrity verified (256 bytes match)

[6/6] Testing fpga_execute_kernel() [placeholder]...
PASSED: Kernel execution placeholder returned success

Cleaning up...
Buffer freed

========================================
  ALL TESTS PASSED
========================================

Week 1 PoC SUCCESS:
  WASM → WASI → XRT → FPGA chain validated
  All 6 host functions operational
  DMA buffer round-trip verified
  Data integrity confirmed
```

**Interpretation:**

| Test | What it Validates | Components Involved |
|------|------------|--------------------------|
| **1. fpga_init()** | XRT can open device | WASM → WASI → XRT → /dev/dri/renderD128 |
| **2. fpga_alloc_buffer()** | DMA memory allocation | WASI → XRT → xrtBOAlloc → CMA |
| **3. fpga_write_buffer()** | Host → Device transfer | WASM memory → DMA buffer → xrtBOSync |
| **4. fpga_read_buffer()** | Device → Host transfer | DMA buffer → WASM memory |
| **5. Data integrity** | Round-trip without corruption | End-to-end data path |
| **6. fpga_execute_kernel()** | Functional API (placeholder) | WASI kernel invocation |

---

## Detailed Troubleshooting

### Problem 1: "wrapper.h: No such file or directory"

**Complete Error:**
```
error: failed to run custom build command for `wasi-fpga v0.1.0`
...
wrapper.h:9:10: fatal error: 'xrt/xrt_device.h' file not found
```

**Cause:** XRT headers not in expected location.

**Diagnosis:**
```bash
# Check if headers exist
find / -name xrt_device.h 2>/dev/null

# Check environment variable
echo $XRT_INCLUDE_PATH
```

**Solution:**
```bash
# Option A: Configure XRT_INCLUDE_PATH
export XRT_INCLUDE_PATH=/usr/include
# Or if in another location:
export XRT_INCLUDE_PATH=/opt/xilinx/xrt/include

# Option B: Create symlink
sudo mkdir -p /usr/include/xrt
sudo ln -s /opt/xilinx/xrt/include/xrt/* /usr/include/xrt/

# Re-run build
cd wasi-fpga
cargo clean
cargo build --release
```

### Problem 2: "libxrt_coreutil.so: cannot open shared object file"

**Complete Error:**
```
error while loading shared libraries: libxrt_coreutil.so: cannot open shared object file: No such file or directory
```

**Cause:** XRT library not in library path.

**Diagnosis:**
```bash
# Search for library
find / -name libxrt_coreutil.so 2>/dev/null

# Check LD_LIBRARY_PATH
echo $LD_LIBRARY_PATH

# Check ldconfig cache
ldconfig -p | grep xrt
```

**Solution:**
```bash
# Option A: Configure LD_LIBRARY_PATH (temporary)
export LD_LIBRARY_PATH=/usr/lib:$LD_LIBRARY_PATH
# Or:
export LD_LIBRARY_PATH=/opt/xilinx/xrt/lib:$LD_LIBRARY_PATH

# Option B: Configure ldconfig (permanent)
echo "/usr/lib" | sudo tee /etc/ld.so.conf.d/xrt.conf
# Or:
echo "/opt/xilinx/xrt/lib" | sudo tee /etc/ld.so.conf.d/xrt.conf

sudo ldconfig

# Verify
ldd /usr/local/lib/libwasi_fpga_extensions.so
# All dependencies should resolve
```

### Problem 3: "zocl driver not loaded"

**Error in dmesg:**
```
[   10.123456] xrtDeviceOpen: No such device
```

**Diagnosis:**
```bash
# Check if module is loaded
lsmod | grep zocl

# Check if module exists
find /lib/modules -name zocl.ko

# Check device tree
ls /sys/bus/platform/devices/ | grep zyxclmm
```

**Solution:**
```bash
# Load module manually
sudo modprobe zocl

# If it fails, check dmesg
dmesg | grep -i zocl

# If module doesn't exist, you need to:
# 1. Rebuild kernel with ZOCL enabled
# 2. Or compile out-of-tree module (see Option 2)

# To load automatically on boot:
echo "zocl" | sudo tee -a /etc/modules

# Or create systemd service:
cat > /tmp/zocl-load.service << 'EOF'
[Unit]
Description=Load ZOCL driver
After=systemd-modules-load.service

[Service]
Type=oneshot
ExecStart=/sbin/modprobe zocl
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
EOF

sudo cp /tmp/zocl-load.service /etc/systemd/system/
sudo systemctl enable zocl-load.service
sudo systemctl start zocl-load.service
```

### Problem 4: "Permission denied" accessing /dev/dri/renderD128

**Error:**
```
ERROR: Failed to open device /dev/dri/renderD128: Permission denied
```

**Diagnosis:**
```bash
# Check permissions
ls -l /dev/dri/renderD128
# May show: crw-rw---- 1 root video (INCORRECT)

# Check user groups
groups
# Should include: video, render
```

**Solution:**
```bash
# Option A: Add user to groups (permanent)
sudo usermod -a -G video,render $USER
# Logout and login again

# Option B: Change permissions (temporary)
sudo chmod 666 /dev/dri/renderD128

# Option C: Create udev rule (permanent and automatic)
cat > /tmp/99-xrt.rules << 'EOF'
# XRT and FPGA device permissions
SUBSYSTEM=="drm", KERNEL=="renderD*", MODE="0666", GROUP="render"
SUBSYSTEM=="drm", KERNEL=="card*", MODE="0666", GROUP="video"
SUBSYSTEM=="uio", MODE="0666"

# Xilinx FPGA devices
ATTR{vendor}=="0x10ee", MODE="0666"
EOF

sudo cp /tmp/99-xrt.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules
sudo udevadm trigger

# Verify change
ls -l /dev/dri/renderD128
# Should show: crw-rw-rw- 1 root render
```

### Problem 5: "CMA allocation failed"

**Error in dmesg:**
```
[  100.123456] xrt_bo_alloc: cma_alloc failed, size=1048576
```

**Diagnosis:**
```bash
# Check available CMA
cat /proc/meminfo | grep Cma
# CmaTotal:      65536 kB  (64 MB - INSUFFICIENT)
# CmaFree:       32768 kB

# Check fragmentation
cat /sys/kernel/debug/cma/cma-reserved/
```

**Solution (requires reboot):**

**Method A: Device Tree (recommended for PetaLinux)**

```bash
# In PetaLinux project
cd /path/to/petalinux-project

# Edit device tree
nano project-spec/meta-user/recipes-bsp/device-tree/files/system-user.dtsi

# Add/modify:
/include/ "system-conf.dtsi"
/ {
    reserved-memory {
        #address-cells = <2>;
        #size-cells = <2>;
        ranges;

        /* Reserve 512MB for CMA */
        linux,cma {
            compatible = "shared-dma-pool";
            reusable;
            size = <0x0 0x20000000>; /* 512MB */
            alignment = <0x0 0x2000>;
            linux,cma-default;
        };
    };
};

# Rebuild device tree
petalinux-build -c device-tree

# Copy new DTB to SD card
sudo cp build/tmp/deploy/images/zcu104-zynqmp/system.dtb /mnt/sd_boot/

# Reboot ZCU104
```

**Method B: Kernel Command Line (alternative)**

```bash
# Edit U-Boot bootargs
# On ZCU104, interrupt U-Boot:
# Press any key during boot

# At U-Boot prompt:
setenv bootargs "console=ttyPS0,115200 earlycon root=/dev/mmcblk0p2 rw rootwait cma=512M"
saveenv
boot

# To make permanent, edit boot.scr:
# In PetaLinux project:
nano project-spec/meta-user/recipes-bsp/u-boot/files/boot.cmd

# Add to bootargs:
# cma=512M

# Rebuild
petalinux-build -c u-boot
```

### Problem 6: "WasmEdge: import module 'fpga' not found"

**Error:**
```
[error] WasmEdge runtime failed: link failed: unknown import, Code: 0x63
  When linking module: "poc-test" , function name: "fpga_init"
```

**Cause:** WasmEdge can't find WASI extension library.

**Diagnosis:**
```bash
# Check environment variable
echo $WASMEDGE_PLUGIN_PATH

# Verify library exists
ls -l /usr/local/lib/libwasi_fpga_extensions.so

# Check WasmEdge recognizes path
strace wasmedge poc-test.wasm 2>&1 | grep wasi_fpga
```

**Solution:**
```bash
# Make sure to pass variable when executing
WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so \
  wasmedge poc-test.wasm

# Or export permanently
export WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so
echo 'export WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga_extensions.so' >> ~/.bashrc

# Verify library is valid
file /usr/local/lib/libwasi_fpga_extensions.so
# Should show: ELF 64-bit LSB shared object, ARM aarch64

# Verify symbols
nm -D /usr/local/lib/libwasi_fpga_extensions.so | grep create_fpga_import
# Should show WasmEdge function
```

### Problem 7: "Data integrity check failed"

**Error:**
```
[5/6] Testing data integrity...
❌ FAILED: Data mismatch!
  Expected first 16 bytes: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]
  Got first 16 bytes:      [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,  0,  0,  0,  0,  0]
```

**Possible Cause 1:** Incorrect DMA sync.

```bash
# Verify WASI extension logs
RUST_LOG=debug wasmedge --env WASMEDGE_PLUGIN_PATH=... poc-test.wasm

# Look in output for:
# "xrtBOSync TO_DEVICE" before write
# "xrtBOSync FROM_DEVICE" before read
```

**Possible Cause 2:** Cache coherency issues.

```bash
# In WASI code (wasi-fpga/src/xrt.rs), ensure:
# - xrtBOSync with correct direction
# - Memory barriers if needed
```

**Solution:**

Review code in [wasi-fpga/src/xrt.rs](wasi-fpga/src/xrt.rs#L86-L115):

```rust
// Must have xrtBOSync before each operation
pub fn write(&self, data: &[u8], offset: usize) -> Result<()> {
    unsafe {
        xrtBOWrite(self.handle, data.as_ptr(), data.len(), offset);

        // CRITICAL: Sync TO device
        xrtBOSync(self.handle, XCL_BO_SYNC_BO_TO_DEVICE, ...);
    }
}

pub fn read(&self, length: usize, offset: usize) -> Result<Vec<u8>> {
    unsafe {
        // CRITICAL: Sync FROM device BEFORE reading
        xrtBOSync(self.handle, XCL_BO_SYNC_BO_FROM_DEVICE, ...);

        xrtBORead(self.handle, buffer.as_mut_ptr(), length, offset);
    }
}
```

### Problem 8: Segfault or Kernel Panic

**Symptoms:**
- `Segmentation fault (core dumped)`
- System freezes
- Kernel panic in dmesg

**Diagnosis:**
```bash
# Enable core dumps
ulimit -c unlimited

# Run test
wasmedge ... poc-test.wasm
# If it crashes: core dump in ./core

# Analyze with gdb
gdb /home/root/.wasmedge/bin/wasmedge core
(gdb) bt
(gdb) info registers

# Review dmesg for kernel panic
dmesg | tail -50
```

**Common Causes:**

1. **Null pointer dereference in WASI extension**
   - Verify all `unwrap()` in Rust code
   - Verify buffer ID validation

2. **Invalid memory access in XRT**
   - Verify bounds checking in write/read
   - Verify buffer size is correct

3. **Corrupt ZOCL driver**
   - Reload driver: `sudo rmmod zocl && sudo modprobe zocl`
   - Check dmesg for hardware errors

---

## Advanced Configuration

### Performance Configuration

#### CPU Governor

```bash
# Change to performance governor (maximum speed)
for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do
    echo performance | sudo tee $cpu
done

# Verify
cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
# Should show: performance

# To make permanent:
cat > /tmp/cpu-performance.service << 'EOF'
[Unit]
Description=Set CPU to performance governor

[Service]
Type=oneshot
ExecStart=/bin/bash -c 'for cpu in /sys/devices/system/cpu/cpu*/cpufreq/scaling_governor; do echo performance > $cpu; done'

[Install]
WantedBy=multi-user.target
EOF

sudo cp /tmp/cpu-performance.service /etc/systemd/system/
sudo systemctl enable cpu-performance.service
```

#### Huge Pages (for better DMA performance)

```bash
# Enable huge pages
echo 128 | sudo tee /proc/sys/vm/nr_hugepages

# Verify
cat /proc/meminfo | grep Huge
# HugePages_Total:     128
# HugePages_Free:      128
# Hugepagesize:       2048 kB

# To make permanent:
echo "vm.nr_hugepages=128" | sudo tee -a /etc/sysctl.conf
```

#### IRQ Affinity (interrupt pinning)

```bash
# Identify FPGA IRQs
cat /proc/interrupts | grep -i zynq

# Pin IRQ to specific CPU (e.g., CPU 3)
echo 8 | sudo tee /proc/irq/<IRQ_NUMBER>/smp_affinity
# 8 = binary 1000 = CPU 3
```

### Monitoring and Profiling

#### FPGA Monitoring

```bash
# Continuous monitoring script
cat > /tmp/fpga_monitor.sh << 'EOF'
#!/bin/bash
while true; do
    clear
    echo "=== FPGA Status ==="
    date
    echo ""

    echo "--- XRT Devices ---"
    xbutil examine -r platform 2>/dev/null || echo "xbutil failed"
    echo ""

    echo "--- CMA Memory ---"
    grep Cma /proc/meminfo
    echo ""

    echo "--- DMA Buffers ---"
    cat /sys/kernel/debug/dma_buf/bufinfo 2>/dev/null | head -20
    echo ""

    echo "--- Temperature (if available) ---"
    cat /sys/class/hwmon/hwmon*/temp*_input 2>/dev/null | awk '{print $1/1000 "°C"}'
    echo ""

    sleep 2
done
EOF

chmod +x /tmp/fpga_monitor.sh
/tmp/fpga_monitor.sh
```

#### Profiling with perf

```bash
# Enable perf (must be compiled in kernel)
petalinux-config -c kernel
# Enable: Kernel hacking → Tracers → Enable perf

# Profile WASM test
perf record -g wasmedge --env WASMEDGE_PLUGIN_PATH=... poc-test.wasm

# Analyze results
perf report

# Profile specifically WASI calls
perf record -e probe:fpga_* wasmedge ... poc-test.wasm
```

### Detailed Logging

#### XRT Logging

```bash
# Enable XRT debug logging
export XRT_LOG_LEVEL=debug
export XRT_LOG_FILE=/tmp/xrt.log

# Run test
wasmedge ... poc-test.wasm

# Review logs
tail -f /tmp/xrt.log
```

#### WASI Extension Logging

```bash
# Enable Rust logging
export RUST_LOG=wasi_fpga=trace
export RUST_BACKTRACE=1

# Run test with detailed logging
wasmedge ... poc-test.wasm 2>&1 | tee /tmp/wasi.log
```

#### Kernel Logging (ZOCL)

```bash
# Enable debug in ZOCL driver
echo 8 > /proc/sys/kernel/printk  # Increase console log level

# Reload driver with debugging
sudo rmmod zocl
sudo modprobe zocl dyndbg='+p'

# Monitor kernel messages
dmesg -w | grep -i zocl
```

---

## Validation Checklist - Phase 2

Use this systematic checklist to validate each component:

### Hardware and Kernel

- [ ] **Board powered correctly**
  - [ ] LED D12 (PGOOD) green on
  - [ ] LED D13 (DONE) green on after boot

- [ ] **Successful boot from SD card**
  - [ ] FSBL executes correctly
  - [ ] U-Boot loads kernel
  - [ ] Linux boots completely
  - [ ] Login prompt available

- [ ] **Kernel with ZOCL support**
  - [ ] Command: `grep ZOCL /boot/config-$(uname -r)` → `CONFIG_ZOCL=m`
  - [ ] Driver compiled: `find /lib/modules -name zocl.ko` → file exists

- [ ] **CMA configured correctly**
  - [ ] Command: `cat /proc/meminfo | grep Cma` → shows at least 256MB
  - [ ] `CmaFree` should be >200MB after boot

### XRT Installation

- [ ] **XRT library installed**
  - [ ] `ls /usr/lib/libxrt_coreutil.so` → file exists (~4-5MB)
  - [ ] `ldd /usr/lib/libxrt_coreutil.so` → all dependencies resolved

- [ ] **XRT headers available**
  - [ ] `ls /usr/include/xrt/xrt_device.h` → file exists
  - [ ] `ls /usr/include/xrt/xrt_bo.h` → file exists

- [ ] **CLI tools installed**
  - [ ] `which xbutil` → `/usr/bin/xbutil`
  - [ ] `xbutil --version` → shows version 2023.1

- [ ] **Library path configured**
  - [ ] `ldconfig -p | grep xrt` → libxrt_coreutil.so listed
  - [ ] `echo $LD_LIBRARY_PATH` → includes `/usr/lib` or `/opt/xilinx/xrt/lib`

### ZOCL Driver

- [ ] **Driver loaded in kernel**
  - [ ] `lsmod | grep zocl` → module listed
  - [ ] `dmesg | grep zocl` → no errors, shows "device registered"

- [ ] **DRM device created**
  - [ ] `ls /dev/dri/renderD128` → device exists
  - [ ] `ls -l /dev/dri/renderD128` → permissions `crw-rw-rw-`

- [ ] **Sysfs entries present**
  - [ ] `ls /sys/class/drm/card0/device/vendor` → file exists
  - [ ] `cat /sys/class/drm/card0/device/vendor` → `0x10ee`

### XRT Runtime Validation

- [ ] **xbutil works**
  - [ ] `xbutil examine` → lists device without errors
  - [ ] `xbutil examine -r platform` → shows device info

- [ ] **Basic XRT API test**
  - [ ] Compile and execute [XRT test](#verification-level-4-basic-xrt-api-test)
  - [ ] Test returns "SUCCESS: Device 0 opened successfully"

- [ ] **Buffer allocation works**
  - [ ] Test with xrtBOAlloc of 1MB → successful
  - [ ] `cat /proc/meminfo | grep Cma` → CmaFree decreased

### Rust Toolchain

- [ ] **Rust installed**
  - [ ] `rustc --version` → shows version 1.75.0 or higher
  - [ ] `cargo --version` → shows corresponding version

- [ ] **Target wasm32-wasip1 available**
  - [ ] `rustup target list --installed | grep wasm32-wasip1` → listed

- [ ] **Compilation works**
  - [ ] `cargo build` in simple project → successful
  - [ ] Generated binary executes correctly

### WASI Extension Build

- [ ] **Environment variables configured**
  - [ ] `echo $XRT_INCLUDE_PATH` → shows correct path
  - [ ] `echo $XRT_LIB_PATH` → shows correct path
  - [ ] `ls $XRT_INCLUDE_PATH/xrt/xrt_device.h` → file exists

- [ ] **Bindgen dependencies**
  - [ ] `which clang` → `/usr/bin/clang` or similar
  - [ ] `llvm-config --version` → shows LLVM version

- [ ] **Successful WASI extension build**
  - [ ] `cd wasi-fpga && cargo build --release` → no errors
  - [ ] `ls target/release/libwasi_fpga_extensions.so` → file ~3-5MB

- [ ] **Correct exported symbols**
  - [ ] `nm -D target/release/libwasi_fpga_extensions.so | grep fpga_init` → symbol present
  - [ ] All 6 fpga_* symbols present

- [ ] **Library dependencies resolved**
  - [ ] `ldd target/release/libwasi_fpga_extensions.so` → no "not found"
  - [ ] `libxrt_coreutil.so` resolved correctly

- [ ] **Library installed**
  - [ ] `ls /usr/local/lib/libwasi_fpga_extensions.so` → file exists
  - [ ] `ldconfig -p | grep wasi_fpga` → library registered

### WasmEdge Runtime

- [ ] **WasmEdge installed**
  - [ ] `wasmedge --version` → `WasmEdge version 0.13.5`
  - [ ] `which wasmedge` → `/home/root/.wasmedge/bin/wasmedge`

- [ ] **Path configured**
  - [ ] `echo $PATH | grep wasmedge` → includes `~/.wasmedge/bin`
  - [ ] `source ~/.wasmedge/env` → no errors

- [ ] **Basic WasmEdge test**
  - [ ] Execute simple hello-world WASM → works
  - [ ] WasmEdge can load basic modules

### WASM Module

- [ ] **Module compiled**
  - [ ] `ls poc-test.wasm` → file exists
  - [ ] `file poc-test.wasm` → `WebAssembly (wasm) binary module version 0x1`

- [ ] **Size optimized**
  - [ ] `ls -lh poc-test.wasm` → size <100KB (target: <5MB)

- [ ] **Correct imports**
  - [ ] `wasm-objdump -x poc-test.wasm | grep import` → shows "fpga" imports

### End-to-End Test

- [ ] **Execution without crash**
  - [ ] `wasmedge --env WASMEDGE_PLUGIN_PATH=... poc-test.wasm` → no segfault
  - [ ] Process ends with exit code 0 or 1 (not killed)

- [ ] **Test 1: fpga_init() passes**
  - [ ] Output shows: `PASSED: FPGA initialized`

- [ ] **Test 2: fpga_alloc_buffer() passes**
  - [ ] Output shows: `PASSED: Buffer allocated (ID=1)`

- [ ] **Test 3: fpga_write_buffer() passes**
  - [ ] Output shows: `PASSED: Wrote 256 bytes to buffer`

- [ ] **Test 4: fpga_read_buffer() passes**
  - [ ] Output shows: `PASSED: Read 256 bytes from buffer`

- [ ] **Test 5: Data integrity passes**
  - [ ] Output shows: `PASSED: Data integrity verified`

- [ ] **Test 6: fpga_execute_kernel() passes**
  - [ ] Output shows: `PASSED: Kernel execution placeholder`

- [ ] **Successful cleanup**
  - [ ] Output shows: `Buffer freed`

- [ ] **Final summary**
  - [ ] Output shows: `ALL TESTS PASSED`

### Performance Validation

- [ ] **Startup time**
  - [ ] `time wasmedge ... poc-test.wasm` → total <2s
  - [ ] WASM cold start <100ms (instrument if needed)

- [ ] **Memory usage**
  - [ ] `top` during execution → RSS <512MB
  - [ ] No memory leaks (run multiple times, stable memory)

- [ ] **WASI call latency**
  - [ ] Instrument individual calls → <10ms each
  - [ ] fpga_init() <50ms, fpga_alloc() <5ms

### System Stability

- [ ] **Test repeated 10 times**
  - [ ] `for i in {1..10}; do wasmedge ... poc-test.wasm; done` → all pass

- [ ] **No kernel errors**
  - [ ] `dmesg` after tests → no panics, no OOMs

- [ ] **CMA not fragmented**
  - [ ] `cat /proc/meminfo | grep Cma` → CmaFree consistent

- [ ] **No file descriptor leaks**
  - [ ] `lsof | wc -l` before and after tests → no significant increase

---

## Additional Resources

### Official Xilinx/AMD Documentation

| Document | URL | Description |
|-----------|-----|-------------|
| **XRT Documentation** | https://xilinx.github.io/XRT/ | Complete XRT documentation |
| **XRT GitHub** | https://github.com/Xilinx/XRT | Source code and issues |
| **XRT Embedded Guide** | https://xilinx.github.io/XRT/master/html/embedded.html | Build for embedded ARM |
| **xbutil Reference** | https://xilinx.github.io/XRT/master/html/xbutil.html | CLI tool documentation |
| **XRT Native API** | https://xilinx.github.io/XRT/master/html/xrt_native_apis.html | C/C++ API reference |
| **Buffer APIs** | https://xilinx.github.io/XRT/master/html/BO.main.html | xrtBO* functions |
| **ZCU104 User Guide** | https://docs.amd.com/v/u/en-US/ug1267-zcu104-eval-bd | Hardware documentation |
| **Zynq MPSoC TRM** | https://docs.amd.com/r/en-US/ug1085-zynq-ultrascale-trm | Technical reference |
| **PetaLinux Tools Guide** | https://docs.amd.com/r/en-US/ug1144-petalinux-tools-reference-guide | PetaLinux reference |
| **Linux Boot Sequence** | https://docs.amd.com/r/en-US/ug1137-zynq-ultrascale-mpsoc-swdev/Zynq-UltraScale-MPSoC-Boot-Sequence | Boot process |
| **CMA Documentation** | https://xilinx-wiki.atlassian.net/wiki/spaces/A/pages/18841683/Linux+Reserved+Memory | Memory management |

### Yocto/OpenEmbedded

| Resource | URL | Description |
|---------|-----|-------------|
| **Yocto Project** | https://www.yoctoproject.org/ | Official site |
| **Yocto Manual** | https://docs.yoctoproject.org/ | Complete documentation |
| **Meta-Xilinx** | https://github.com/Xilinx/meta-xilinx | Xilinx BSP layer |
| **Meta-Xilinx-Tools** | https://github.com/Xilinx/meta-xilinx-tools | XRT recipes |
| **Bitbake User Manual** | https://docs.yoctoproject.org/bitbake/ | Build system |

### WebAssembly and WASI

| Resource | URL | Description |
|---------|-----|-------------|
| **WasmEdge** | https://wasmedge.org/docs/ | Runtime documentation |
| **WasmEdge GitHub** | https://github.com/WasmEdge/WasmEdge | Source code |
| **WasmEdge Rust SDK** | https://wasmedge.org/docs/sdk/rust | Rust embedding |
| **Host Functions** | https://wasmedge.org/docs/develop/rust/host_function | Custom host functions |
| **WASI Spec** | https://wasi.dev/ | WASI specification |
| **WebAssembly Spec** | https://webassembly.github.io/spec/core/ | Core specification |
| **wasm32-wasip1** | https://doc.rust-lang.org/rustc/platform-support/wasm32-wasip1.html | Rust WASI target |

### Rust

| Resource | URL | Description |
|---------|-----|-------------|
| **Rust Book** | https://doc.rust-lang.org/book/ | Official learning resource |
| **Rust Installation** | https://doc.rust-lang.org/book/ch01-01-installation.html | Install guide |
| **Rust FFI** | https://doc.rust-lang.org/nomicon/ffi.html | Foreign function interface |
| **bindgen** | https://rust-lang.github.io/rust-bindgen/ | C bindings generator |
| **thiserror** | https://docs.rs/thiserror/latest/thiserror/ | Error handling |
| **Embedded Rust** | https://docs.rust-embedded.org/book/ | Embedded development |

### Kubernetes and Orchestration

| Resource | URL | Description |
|---------|-----|-------------|
| **Device Plugin API** | https://kubernetes.io/docs/concepts/extend-kubernetes/compute-storage-net/device-plugins/ | K8s device plugins |
| **RuntimeClass** | https://kubernetes.io/docs/concepts/containers/runtime-class/ | Runtime selection |
| **K3s** | https://docs.k3s.io/ | Lightweight Kubernetes |

### Community and Support

| Resource | URL | Description |
|---------|-----|-------------|
| **Xilinx Forums** | https://support.xilinx.com/s/topic/0TO2E000000YKYAWA4/ | Official support |
| **XRT GitHub Issues** | https://github.com/Xilinx/XRT/issues | Bug reports, questions |
| **WasmEdge Discord** | https://discord.gg/U4B5sFTkFc | Community chat |
| **Rust Community** | https://www.rust-lang.org/community | Rust help |

### Papers and Academic References

| Resource | Description |
|---------|-------------|
| **Anonymized project context** | Neuromorphic AI orchestration research context |
| **Co-design report** | [CoDesign_Report_Complete.md](CoDesign_Report_Complete.md) |
| **WebAssembly for Edge** | Research on WASM in edge computing |

---

## Final Notes

### Key Differences: PetaLinux vs Ubuntu

| Aspect | Ubuntu (x86_64) | PetaLinux (ARM64) |
|---------|-----------------|-------------------|
| Package Management | APT/dpkg | Yocto/BitBake recipes |
| Install Time | Runtime (apt install) | Build-time (petalinux-build) |
| Dependencies | Resolved at runtime | Resolved at build-time |
| Kernel Config | Generic | Custom for ZCU104 |
| XRT Install | .deb package | meta-xilinx layer |
| Update Process | apt update/upgrade | Rebuild PetaLinux image |
| Disk Space | Minimal impact | Requires full rebuild (~50GB) |
| Build Time | Seconds (install) | Hours (compile) |

### When to Use Each Installation Option

| Option | Best For | Time | Complexity |
|--------|-----------|--------|-------------|
| **Meta-Xilinx** | Production, reproducible image | High (1-3h) | Medium |
| **Cross-compile** | Complete control, XRT debugging | Medium (30-60min) | High |
| **Post-boot** | Quick testing, development | High (4-6h) | Low |

**Recommendation:**
- **Development:** Option 3 (post-boot) for rapid iteration
- **Production:** Option 1 (meta-xilinx) for reproducible image

### Next Steps After Phase 2

1. **Validate Performance Targets**
   - WASM startup: <100ms
   - WASI call latency: <1ms
   - Memory usage: <512MB
   - Binary size: <5MB

2. **Implement Phase 3: Full WASI API**
   - Real kernel execution (xclbin loading)
   - Multi-FPGA support
   - Async operations

3. **Develop Phase 4: Neuromorphic Workload**
   - SNN inference module
   - Bitstream for neuromorphic accelerator
   - <10ms inference latency

4. **Integrate Phase 5: Kubernetes**
   - Go device plugin
   - Helm charts
   - E2E testing

---

## Changelog

| Version | Date | Changes |
|---------|-------|---------|
| 1.0 | Nov 20, 2025 | Complete initial version |

---

**Author:** Anonymous artifact authors
**Project:** WASM-Integrated FPGA Orchestration
**Contact:** Use the artifact repository issue tracker or the submission contact listed in the FPL artifact form.

---

Good luck with the installation! If you encounter specific problems, consult the [Troubleshooting](#detailed-troubleshooting) section or review logs carefully.
