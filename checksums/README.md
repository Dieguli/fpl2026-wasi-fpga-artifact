# Checksums

Generate SHA-256 checksums for every release asset before uploading to Zenodo:

```bash
sha256sum \
  BCPNN_infer_float.xclbin \
  alvis_fullmnist_32x128_64x64_eps-4.bin \
  wasi-fpga/target/release/libwasi_fpga.so \
  wasi-fpga/wasm/*.wasm \
  bench_results_*.csv \
  bench_log_*.txt \
  > checksums.sha256
```

Archive `checksums.sha256` with the release and copy it into this directory for the final repository version.
