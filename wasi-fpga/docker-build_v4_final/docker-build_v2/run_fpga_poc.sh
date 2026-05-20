#!/bin/bash

# ===============================================================================
# SCRIPT V5: STRICT VOLUMES (FIX INVALID PATH WASM)
# ===============================================================================

export KUBECONFIG=/etc/rancher/k3s/k3s.yaml

POD_NAME="fpga-flexible-poc"
DATA_DIR="/home/ubuntu/bcpnn_artifacts"
ORCH_DIR="/home/ubuntu/docker-build_v2"
PLUGIN_SRC="/home/ubuntu/docker-build_v2/libwasi_fpga_buena.so"

XCLBIN="BCPNN_infer_float.xclbin"
WEIGHTS="alvis_fullmnist_32x128_64x64_eps-4.bin"

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# ===============================================================================
# 0. K3S CHECK
# ===============================================================================
if ! systemctl is-active --quiet k3s; then
  echo -e "${YELLOW} K3s was stopped. Starting service...${NC}"
  systemctl start k3s
  sleep 5
fi

# ===============================================================================
# 1. ARGUMENT HANDLING AND COMMAND DECISION
# ===============================================================================
MODE="synthetic"
VIDEO_FILE="none"

CTR_XCLBIN="/data/PAC_container/hwconfig/mnist_float/zcu104/\$XCLBIN_NAME"
CTR_WEIGHTS="/data/TrainedWeight/\$WEIGHTS_NAME"
CTR_VIDEO="/data/video_input/\$VIDEO_NAME"

if [ -z "$1" ]; then
  echo -e "${YELLOW}Usage: sudo ./run_fpga_poc.sh [video_name.avi | --synthetic]${NC}"
  exit 1
fi

if [ "$1" == "--synthetic" ]; then
  MODE="synthetic"
  echo -e "${GREEN} Selected mode: SYNTHETIC${NC}"
  POD_COMMAND="echo ' RUNNING SYNTHETIC MODE'; /usr/local/bin/avi_processor --synthetic \"$CTR_XCLBIN\" \"$CTR_WEIGHTS\""
else
  MODE="video"
  VIDEO_FILE="$1"
  if [ ! -f "$DATA_DIR/video_input/$VIDEO_FILE" ]; then
    echo -e "${RED} Error: Video not found at: $DATA_DIR/video_input/$VIDEO_FILE${NC}"
    exit 1
  fi
  echo -e "${GREEN} Selected mode: VIDEO ($VIDEO_FILE)${NC}"
  POD_COMMAND="echo ' RUNNING VIDEO MODE'; /usr/local/bin/avi_processor \"$CTR_VIDEO\" \"$CTR_XCLBIN\" \"$CTR_WEIGHTS\""
fi

# ===============================================================================
# 2. MEMORY CLEANUP
# ===============================================================================
echo -e "\n${YELLOW}[1/4] Preparing hardware and CMA memory...${NC}"
k3s kubectl delete pod $POD_NAME --force --grace-period=0 2>/dev/null

systemctl stop gdm3
sync
echo 3 > /proc/sys/vm/drop_caches

CMA_FREE=$(cat /proc/meminfo | grep CmaFree | awk '{print $2}')
echo -e "CMA Free Memory: ${GREEN}$((CMA_FREE / 1024)) MB${NC}"

if [ "$CMA_FREE" -lt 150000 ]; then
  echo -e "${RED}  WARNING: Low memory (<150MB).${NC}"
fi

# ===============================================================================
# 3. PREPARING LIBRARIES
# ===============================================================================
mkdir -p /usr/local/lib/wasmedge
cp "$PLUGIN_SRC" /usr/local/lib/wasmedge/libwasi_fpga.so 2>/dev/null

# ===============================================================================
# 4. YAML GENERATION (STRICT VOLUMES)
# ===============================================================================
echo -e "${YELLOW}[2/4] Generating Pod definition (Image v2)...${NC}"

cat <<EOF > generated_pod.yaml
apiVersion: v1
kind: Pod
metadata:
  name: $POD_NAME
spec:
  hostNetwork: true
  containers:
  - name: bcpnn-engine
    image: wasi-fpga:v2
    imagePullPolicy: Never
    securityContext:
      privileged: true
    workingDir: /app
    env:
      - name: VIDEO_NAME
        value: "$VIDEO_FILE"
      - name: XCLBIN_NAME
        value: "$XCLBIN"
      - name: WEIGHTS_NAME
        value: "$WEIGHTS"
      - name: LD_LIBRARY_PATH
        value: "/lib:/usr/lib/aarch64-linux-gnu:/usr/local/lib"
      - name: WASMEDGE_PLUGIN_PATH
        value: "/opt/wasmedge/plugins/libwasi_fpga.so"
    
    command: ["/bin/bash", "-c"]
    args: 
      - |
        $POD_COMMAND

    volumeMounts:
    - mountPath: /dev
      name: dev-dir
    - mountPath: /data
      name: firmware-data
    - mountPath: /usr/local/bin/avi_processor
      name: rust-binary
    - mountPath: /app/wasm
      name: wasm-dir
    - mountPath: /opt/wasmedge/plugins/libwasi_fpga.so
      name: good-plugin
    - mountPath: /lib
      name: host-lib
    - mountPath: /usr/lib/aarch64-linux-gnu
      name: host-usr-lib-aarch
    - mountPath: /etc/OpenCL
      name: host-etc-opencl

  volumes:
  - name: dev-dir
    hostPath: 
      path: /dev
      type: Directory
  - name: firmware-data
    hostPath: 
      path: $DATA_DIR
      type: DirectoryOrCreate
  - name: rust-binary
    hostPath: 
      path: $ORCH_DIR/avi_processor
      type: File
  - name: wasm-dir
    hostPath: 
      path: $ORCH_DIR/wasm
      type: Directory
  - name: good-plugin
    hostPath: 
      path: /usr/local/lib/wasmedge/libwasi_fpga.so
      type: File
  - name: host-lib
    hostPath: 
      path: /lib
      type: Directory
  - name: host-usr-lib-aarch
    hostPath: 
      path: /usr/lib/aarch64-linux-gnu
      type: Directory
  - name: host-etc-opencl
    hostPath: 
      path: /etc/OpenCL
      type: DirectoryOrCreate

  restartPolicy: Never
EOF

# ===============================================================================
# 5. EXECUTION
# ===============================================================================
echo -e "${YELLOW}[3/4] Launching Pod in K3s...${NC}"
k3s kubectl apply -f generated_pod.yaml

echo -e "${YELLOW}[4/4] Connecting to logs...${NC}"
sleep 3
k3s kubectl logs $POD_NAME -f
