#!/bin/bash
# Build script for PoC test WASM module

set -e

echo "=== Building PoC Test WASM Module ==="

# Ensure wasm32-wasi target is installed
if ! rustup target list --installed | grep -q wasm32-wasi; then
    echo "Installing wasm32-wasi target..."
    rustup target add wasm32-wasi
fi

# Build to WASM
echo "Building to wasm32-wasi..."
cargo build --target wasm32-wasi --release

# Optimize for size
if command -v wasm-opt &> /dev/null; then
    echo "Optimizing with wasm-opt..."
    wasm-opt -Oz \
        target/wasm32-wasi/release/poc-test.wasm \
        -o target/wasm32-wasi/release/poc-test-opt.wasm

    SIZE_BEFORE=$(stat -f%z target/wasm32-wasi/release/poc-test.wasm 2>/dev/null || stat -c%s target/wasm32-wasi/release/poc-test.wasm)
    SIZE_AFTER=$(stat -f%z target/wasm32-wasi/release/poc-test-opt.wasm 2>/dev/null || stat -c%s target/wasm32-wasi/release/poc-test-opt.wasm)

    echo "Size before optimization: $(numfmt --to=iec-i --suffix=B $SIZE_BEFORE 2>/dev/null || echo $SIZE_BEFORE bytes)"
    echo "Size after optimization:  $(numfmt --to=iec-i --suffix=B $SIZE_AFTER 2>/dev/null || echo $SIZE_AFTER bytes)"

    mv target/wasm32-wasi/release/poc-test-opt.wasm target/wasm32-wasi/release/poc-test.wasm
else
    echo "WARNING: wasm-opt not found - skipping optimization"
    echo "Install with: cargo install wasm-opt"
fi

echo ""
echo "✅ Build complete!"
echo "Output: target/wasm32-wasi/release/poc-test.wasm"
echo ""
echo "To run (requires WasmEdge and FPGA hardware):"
echo "  wasmedge --env WASMEDGE_PLUGIN_PATH=../../../wasi-fpga/target/release/libwasi_fpga_extensions.so \\"
echo "           target/wasm32-wasi/release/poc-test.wasm"
