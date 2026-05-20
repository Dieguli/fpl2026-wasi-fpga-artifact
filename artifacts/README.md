# External Release Assets

This directory documents files that are expected to accompany the GitHub release or Zenodo deposit but are not tracked in Git.

Place local copies here only while assembling a release. Do not commit proprietary vendor files or large generated result sets unless their license and size make that appropriate.

Expected release assets:

| Asset | Purpose |
| --- | --- |
| `BCPNN_infer_float.xclbin` | FPGA bitstream used by the WASM and native inference paths. |
| `alvis_fullmnist_32x128_64x64_eps-4.bin` | Trained BCPNN data used by the benchmark workloads. |
| `environment_record.txt` | Captured board/runtime/toolchain fingerprint. |
| `checksums.sha256` | SHA-256 checksums for release assets and generated binaries. |
| `bench_results_*.csv` | Raw benchmark results used in paper tables or figures. |
| `bench_log_*.txt` | Raw benchmark stderr logs, including `[BENCH]` lines. |
