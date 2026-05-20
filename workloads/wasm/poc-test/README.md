# PoC Test - WASM FPGA Integration Validation

Week 1 Day 3 Proof-of-Concept test that validates the complete WASM → WASI → XRT → FPGA chain.

## Purpose

This minimal WASM module tests all 6 WASI FPGA host functions:

1. **`fpga_init()`** - Initialize FPGA device
2. **`fpga_alloc_buffer(1024)`** - Allocate 1KB DMA buffer
3. **`fpga_write_buffer()`** - Write 256 bytes of test data
4. **`fpga_read_buffer()`** - Read 256 bytes back
5. **`fpga_execute_kernel()`** - Execute placeholder kernel (v1.0)
6. **`fpga_free_buffer()`** - Free DMA buffer

## Building

### Prerequisites
- Rust 1.75+ with `wasm32-wasi` target
- `wasm-opt` (optional, for size optimization)

### Build Commands

```bash
# Automated build
./build.sh

# Or manual build
cargo build --target wasm32-wasi --release

# Optimize (optional)
wasm-opt -Oz target/wasm32-wasi/release/poc-test.wasm -o poc-test.wasm
```

## Running

### On FPGA Node (Real Hardware)

```bash
# Ensure WASI extension library is built
cd ../../../wasi-fpga
cargo build --release

# Run PoC test
cd ../workloads/wasm/poc-test
wasmedge \
  --env WASMEDGE_PLUGIN_PATH=../../../wasi-fpga/target/release/libwasi_fpga_extensions.so \
  target/wasm32-wasi/release/poc-test.wasm
```

### Expected Output (Success)

```
========================================
  WASM FPGA PoC Test - Week 1 Day 3
========================================

[1/6] Testing fpga_init()...
✅ PASSED: FPGA initialized

[2/6] Testing fpga_alloc_buffer(1024)...
✅ PASSED: Buffer allocated (ID=1)

[3/6] Testing fpga_write_buffer()...
✅ PASSED: Wrote 256 bytes to buffer

[4/6] Testing fpga_read_buffer()...
✅ PASSED: Read 256 bytes from buffer

[5/6] Testing data integrity...
✅ PASSED: Data integrity verified (256 bytes match)

[6/6] Testing fpga_execute_kernel() [placeholder]...
✅ PASSED: Kernel execution placeholder returned success

Cleaning up...
✅ Buffer freed

========================================
  ✅ ALL TESTS PASSED
========================================

Week 1 PoC SUCCESS:
  ✅ WASM → WASI → XRT → FPGA chain validated
  ✅ All 6 host functions operational
  ✅ DMA buffer round-trip verified
  ✅ Data integrity confirmed
```

## Success Criteria (Week 1 Day 3)

- [x] WASM module compiles to wasm32-wasi without errors
- [x] Binary size <5MB (typically <50KB for this test)
- [x] WasmEdge loads WASM module and WASI extension library
- [x] FPGA device opens successfully (`fpga_init()`)
- [x] DMA buffer allocation succeeds
- [x] Write/read round-trip preserves data integrity
- [x] No segfaults, panics, or memory corruption

## Troubleshooting

### WASM Build Errors

**Error:** `target 'wasm32-wasi' not found`
**Fix:** `rustup target add wasm32-wasi`

**Error:** `binary too large (>5MB)`
**Fix:** Ensure `opt-level = "z"`, `lto = true`, use `wasm-opt -Oz`

### Runtime Errors

**Error:** `WasmEdge: import "fpga" not found`
**Fix:** Set `WASMEDGE_PLUGIN_PATH` to point to `libwasi_fpga_extensions.so`

**Error:** `fpga_init() returned -1`
**Fix:** Check FPGA device availability:
```bash
ls -l /dev/uio* /dev/dri/renderD*
lspci -d 10ee:
sudo modprobe zocl
```

**Error:** `fpga_alloc_buffer() returned -1`
**Fix:** Check CMA memory:
```bash
cat /proc/meminfo | grep Cma
# Increase if needed: add cma=512M to kernel cmdline
```

**Error:** Data mismatch
**Fix:** This indicates a DMA sync issue or XRT driver problem. Check `dmesg` for kernel errors.

## Next Steps

After PoC validation:

1. **Phase 2**: Implement full BufferManager with XRT bindings
2. **Phase 3**: Add kernel execution support (v2.0)
3. **Phase 4**: Build neuromorphic inference workload
4. **Phase 5**: Kubernetes integration and E2E testing

## File Size Targets

- Unoptimized: ~200KB
- With `opt-level = "z"`: ~50KB
- With `wasm-opt -Oz`: ~30KB

All well under 5MB target!
