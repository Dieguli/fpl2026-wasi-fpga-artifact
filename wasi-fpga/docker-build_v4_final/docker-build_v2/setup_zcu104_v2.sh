#!/bin/bash

# =============================================================================
# SETUP ZCU104 V12 - SAFE XRT + PRE-SWAP + HYBRID DOCKER (K8S NAMESPACE FIX)
# =============================================================================

set -e
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# --- OPTIONAL REPOSITORY CONFIGURATION ---
# For anonymous artifact review, large BCPNN assets should come from the
# versioned release/DOI bundle. Set these variables only if evaluators have
# access to separate non-identifying mirrors.
REPO_PAC_URL="${REPO_PAC_URL:-}"
REPO_DATA_URL="${REPO_DATA_URL:-}"

# --- DOCKER CONFIGURATION ---
# Local image tar filename
LOCAL_IMAGE_TAR="wasi-fpga_v2.tar"
# Nombre de la imagen en Docker Hub
HUB_IMAGE_URL="wasi-fpga:v2"
# Nombre que espera el Pod de Kubernetes
TARGET_IMAGE_TAG="wasi-fpga:v2"

# --- PATHS ---
HOME_DIR="/home/ubuntu"
DIR_PAC_REPO="$HOME_DIR/bcpnn_reference"
DIR_DATA_REPO="$HOME_DIR/bcpnn_artifacts"

echo -e "${GREEN} STARTING HYBRID ZCU104 DEPLOYMENT (V12)${NC}"

# 0. Root check
if [ "$EUID" -ne 0 ]; then
    echo -e "${RED} Run as root: sudo ./setup_zcu104_final.sh${NC}"
    exit 1
fi

# =============================================================================
# 1. SWAP CREATION (CRITICAL: MUST RUN BEFORE ANY INSTALLATION)
# =============================================================================
echo -e "${YELLOW}[1/8] Checking Swap memory (RAM lifeline)...${NC}"
if [ ! -f /swapfile ]; then
    echo "    Creating 4GB Swap to avoid system crashes..."
    fallocate -l 4G /swapfile && chmod 600 /swapfile && mkswap /swapfile && swapon /swapfile
    echo '/swapfile none swap sw 0 0' >> /etc/fstab
    echo -e "${GREEN}     Swap created successfully.${NC}"
else
    echo -e "${GREEN}     Swap already configured.${NC}"
fi

# =============================================================================
# 1.5 SYSTEM DEPENDENCIES AND SMART XRT
# =============================================================================
echo -e "${YELLOW}[1.5/8] Installing base tools (Git, FFmpeg)...${NC}"
apt-get update
# NOTE: xrt-dkms removed from this line
apt-get install -y curl git htop wget ffmpeg build-essential

echo -e "${YELLOW}      Checking XRT driver (zocl)...${NC}"
if lsmod | grep -q zocl || modinfo zocl &> /dev/null; then
    echo -e "${GREEN}     Driver 'zocl' detected. Skipping build.${NC}"
else
    echo -e "${RED}      Driver 'zocl' not found.${NC}"
    echo "       Starting heavy xrt-dkms build..."
    echo "       (This will take ~15 minutes. With Swap it should not hang!)"
    apt-get install -y xrt-dkms
fi

# =============================================================================
# 2. AUTOMATIC GIT REPOSITORY DOWNLOAD
# =============================================================================
echo -e "${YELLOW}[2/8] Synchronizing GitHub repositories...${NC}"

# 2.1 Hardware repo (PAC)
if [ ! -d "$DIR_PAC_REPO" ]; then
    if [ -n "$REPO_PAC_URL" ]; then
        echo "      Cloning BCPNN reference package..."
        git clone "$REPO_PAC_URL" "$DIR_PAC_REPO"
        chown -R ubuntu:ubuntu "$DIR_PAC_REPO"
    else
        echo "      BCPNN reference package not found at $DIR_PAC_REPO."
        echo "      Place the release/DOI BCPNN reference files there or set REPO_PAC_URL."
    fi
else
    echo "     BCPNN reference package present."
    if [ -d "$DIR_PAC_REPO/.git" ] && [ -n "$REPO_PAC_URL" ]; then
        cd "$DIR_PAC_REPO" && git pull && cd - > /dev/null
    fi
fi

# 2.2 Data repo (Videos/Weights)
if [ ! -d "$DIR_DATA_REPO" ]; then
    if [ -n "$REPO_DATA_URL" ]; then
        echo "      Cloning BCPNN artifact data..."
        git clone "$REPO_DATA_URL" "$DIR_DATA_REPO"
        chown -R ubuntu:ubuntu "$DIR_DATA_REPO"
    else
        echo "      BCPNN artifact data not found at $DIR_DATA_REPO."
        echo "      Place the release/DOI xclbin, weights, and video inputs there or set REPO_DATA_URL."
    fi
else
    echo "      BCPNN artifact data present."
    if [ -d "$DIR_DATA_REPO/.git" ] && [ -n "$REPO_DATA_URL" ]; then
        cd "$DIR_DATA_REPO" && git pull && cd - > /dev/null
    fi
fi

# =============================================================================
# 3. KERNEL CONFIGURATION (CMA 1GB)
# =============================================================================
echo -e "${YELLOW}[3/8] Checking CMA Memory (1024M)...${NC}"
FLASH_CFG="/etc/default/flash-kernel"

if ! grep -q "cma=1024M" "$FLASH_CFG"; then
    echo "      Adding cma=1024M..."
    sed -i 's/LINUX_KERNEL_CMDLINE="/LINUX_KERNEL_CMDLINE="cma=1024M /' "$FLASH_CFG"
    echo "     Regenerating boot image (flash-kernel)..."
    flash-kernel
else
    echo -e "${GREEN}    ✅ CMA configuration OK.${NC}"
fi

# =============================================================================
# 4. AUTOMATIC PAC INSTALLATION
# =============================================================================
echo -e "${YELLOW}[4/8] Installing Bitstreams (PAC)...${NC}"
PAC_SOURCE="$DIR_PAC_REPO/PAC_container"

if [ -d "$PAC_SOURCE" ]; then
    mkdir -p /boot/firmware/xlnx-config/
    mkdir -p /usr/local/share/xlnx-config/

    cp -rf "$PAC_SOURCE" /boot/firmware/xlnx-config/
    cp -rf "$PAC_SOURCE" /usr/local/share/xlnx-config/
    
    echo -e "${GREEN}     PAC installed in firmware.${NC}"
else
    echo -e "${RED}     ERROR: 'PAC_container' not found in repo.${NC}"
    exit 1
fi

# =============================================================================
# 5. XILINX HARDWARE ACTIVATION
# =============================================================================
echo -e "${YELLOW}[5/8] Activating Platform: stream_32x128_SP...${NC}"
if xlnx-config -a stream_32x128_SP; then
    echo -e "${GREEN}     Platform activated.${NC}"
else
    echo -e "${YELLOW}      Reboot pending to apply.${NC}"
fi

# =============================================================================
# 6. WASMEDGE INSTALLATION
# =============================================================================
echo -e "${YELLOW}[6/8] Installing WasmEdge Runtime...${NC}"
if ! command -v wasmedge &> /dev/null; then
    curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -p /usr/local
fi
mkdir -p /usr/local/lib/wasmedge

# =============================================================================
# 7. K3S INSTALLATION
# =============================================================================
echo -e "${YELLOW}[7/8] Configuring Kubernetes (K3s)...${NC}"
if ! command -v k3s &> /dev/null; then
    curl -sfL https://get.k3s.io | sh -
    sleep 10
fi

if [ -f /etc/rancher/k3s/k3s.yaml ]; then
    mkdir -p /home/ubuntu/.kube
    cp /etc/rancher/k3s/k3s.yaml /home/ubuntu/.kube/config
    chown -R ubuntu:ubuntu /home/ubuntu/.kube
    chmod 600 /home/ubuntu/.kube/config
fi

# =============================================================================
# 8. DOCKER IMAGE IMPORT (HYBRID: LOCAL -> HUB)
# =============================================================================
echo -e "${YELLOW}[8/8] Managing Docker Image...${NC}"

# A. Try local first
POSSIBLE_LOCATIONS=(
    "./$LOCAL_IMAGE_TAR"
    "$HOME_DIR/$LOCAL_IMAGE_TAR"
)
IMAGE_FOUND=0

echo "    🔎 Searching for local file: $LOCAL_IMAGE_TAR..."
for TAR_PATH in "${POSSIBLE_LOCATIONS[@]}"; do
    if [ -f "$TAR_PATH" ]; then
        echo "       -> Found at: $TAR_PATH"
        # CRITICAL: Import using k8s.io namespace
        k3s ctr -n k8s.io images import "$TAR_PATH"
        echo -e "${GREEN}     Image imported from local.${NC}"
        IMAGE_FOUND=1
        break
    fi
done

# B. Try Docker Hub (Fallback)
if [ $IMAGE_FOUND -eq 0 ]; then
    echo -e "${YELLOW}      Not found locally. Attempting to download from Docker Hub...${NC}"

    # CRITICAL: Force docker.io prefixes so containerd (ctr) doesn't get confused
    FULL_HUB_URL="docker.io/$HUB_IMAGE_URL"
    FULL_TARGET_URL="docker.io/library/$TARGET_IMAGE_TAG"

    echo "       Target: $FULL_HUB_URL"

    if k3s crictl pull "$FULL_HUB_URL"; then
        echo -e "${GREEN}     Image downloaded from Docker Hub.${NC}"

        # CRITICAL: Retag explicitly indicating Kubernetes namespace
        k3s ctr -n k8s.io images tag "$FULL_HUB_URL" "$FULL_TARGET_URL"
        echo "       Retagged locally as: $FULL_TARGET_URL"
    else
        echo -e "${RED}     TOTAL FAILURE: Not found locally and could not download.${NC}"
        echo "       - Check your internet connection."
        echo "       - Or upload the file $LOCAL_IMAGE_TAR to /home/ubuntu/"
    fi
fi

# =============================================================================
# REINICIO AUTOMÁTICO
# =============================================================================
echo -e "\n========================================================"
echo -e "${GREEN} INSTALLATION COMPLETED.${NC}"
echo -e "${YELLOW} REBOOTING IN 5 SECONDS TO APPLY CHANGES...${NC}"
echo -e "========================================================"

for i in {5..1}; do
    echo "Rebooting in $i..."
    sleep 1
done

reboot
