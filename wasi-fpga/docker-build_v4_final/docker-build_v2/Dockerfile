FROM ubuntu:22.04

# Install dependencies
RUN apt-get update && apt-get install -y \
    ffmpeg \
    libopencl1 \
    ocl-icd-libopencl1 \
    ocl-icd-opencl-dev \
    curl \
    git \
    build-essential \
    python3 \
    python3-pip \
    libboost-filesystem1.74.0 \
    libboost-system1.74.0 \
    && rm -rf /var/lib/apt/lists/*

# Install WasmEdge
RUN curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- -p /usr/local

# --- FIX HERE ---
# Copy the actual plugin filename you have in your folder
COPY libwasi_fpga.so /usr/local/lib/libwasi_fpga.so

# Configure environment pointing to the correct plugin path
ENV WASMEDGE_PLUGIN_PATH=/usr/local/lib/libwasi_fpga.so
ENV LD_LIBRARY_PATH=/usr/local/lib:/opt/xilinx/xrt/lib:/usr/lib

WORKDIR /app

# --- ANOTHER FIX ---
# In your listing the wasm is at root, not inside 'wasm/'
COPY test_bcpnn_infer.wasm /app/