//! Benchmark BCPNN inference on FPGA via WASI-FPGA
//!
//! Runs N iterations of BCPNN inference and reports timing data for FPL 2026.
//!
//! Two-layer timing:
//!   - Host-side: OpenCL profiling events (when WASI_FPGA_BENCH=1) → [BENCH] on stderr
//!   - WASM-side: std::time::Instant wall-clock → CSV on stdout
//!
//! Compile:
//!   rustc --target wasm32-wasip1 --edition 2021 -o wasm/bench_bcpnn_infer.wasm wasm/bench_bcpnn_infer.rs
//!
//! Run:
//!   WASI_FPGA_BENCH=1 wasmedge --dir /:/ bench_bcpnn_infer.wasm \
//!     ./BCPNN_infer_float.xclbin \
//!     ./alvis_fullmnist_32x128_64x64_eps-4.bin \
//!     --bench 100

use std::convert::TryInto;
use std::time::Instant;

#[link(wasm_import_module = "fpga")]
extern "C" {
    fn load_xclbin(path_ptr: *const u8, path_len: i32) -> i32;
    fn create_kernel(name_ptr: *const u8, name_len: i32) -> i32;
    fn alloc(size: i32) -> i32;
    fn write(buf_id: i32, data_ptr: *const u8, data_len: i32) -> i32;
    fn read(buf_id: i32, data_ptr: *mut u8, data_len: i32) -> i32;
    fn set_arg(arg_idx: i32, buf_id: i32) -> i32;
    fn set_arg_int(arg_idx: i32, value: i32) -> i32;
    fn set_arg_float(arg_idx: i32, value_bits: i32) -> i32;
    fn run(in_ids_ptr: *const i32, in_ids_len: i32,
           out_ids_ptr: *const i32, out_ids_len: i32) -> i32;
    fn free(buf_id: i32) -> i32;
}

// ===========================================================================
// BCPNN KERNEL CONSTANTS
// ===========================================================================
const H_IN: usize = 784;
const M_IN: usize = 2;
const N_IN: usize = H_IN * M_IN;           // 1568

const H_HID: usize = 32;
const M_HID: usize = 128;
const N_HID: usize = H_HID * M_HID;        // 4096

const H_UT: usize = 1;
const M_UT: usize = 10;
const N_UT: usize = H_UT * M_UT;            // 10

const NACTHI: usize = 64;
const NSILHI: usize = 64;

const DEN_HI_IH: usize = NACTHI + NSILHI;   // 128
const DEN_NI_IH: usize = DEN_HI_IH * M_IN;  // 256
const DEN_NI_HU: usize = H_HID * M_HID;     // 4096

const KERNEL_NAME: &str = "BCPNN_infer_float";

fn main() {
    eprintln!("=== WASI-FPGA BCPNN Inference Benchmark ===");

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: wasmedge bench_bcpnn_infer.wasm <xclbin> <weights.bin> [params.par] --bench N");
        eprintln!("  N = number of inference iterations (default: 10)");
        std::process::exit(1);
    }
    let xclbin_path = &args[1];
    let weights_path = &args[2];

    // Parse optional .par file and --bench N
    let mut params_path: Option<&str> = None;
    let mut bench_n: usize = 10;
    let mut i = 3;
    while i < args.len() {
        if args[i] == "--bench" {
            if i + 1 < args.len() {
                bench_n = args[i + 1].parse().unwrap_or(10);
                i += 2;
                continue;
            }
        } else if args[i].ends_with(".par") {
            params_path = Some(&args[i]);
        }
        i += 1;
    }

    eprintln!("Kernel: {}", KERNEL_NAME);
    eprintln!("Iterations: {} (+ 1 warmup)", bench_n);

    // ===========================================================================
    // 1. LOAD WEIGHTS
    // ===========================================================================
    eprintln!("\n[1] Loading weights: {}", weights_path);
    let weights_data = std::fs::read(weights_path).expect("Failed to read weights file");
    eprintln!("  File size: {} bytes", weights_data.len());

    let mut offset = 0;
    let hihjhi_ih = read_vec_i32(&weights_data, &mut offset);
    let bj_ih     = read_vec_f32(&weights_data, &mut offset);
    let wji_ih    = read_vec_f32(&weights_data, &mut offset);
    let bj_hu     = read_vec_f32(&weights_data, &mut offset);
    let wji_hu    = read_vec_f32(&weights_data, &mut offset);
    let _constant_hbm = read_vec_f32(&weights_data, &mut offset);

    // Scalar parameters
    let (nampl, nfreq, igain0, igain2, bwgain1, bwgain2, taumdt0, taumdt1, taumdt2) =
        if let Some(path) = params_path {
            load_params_from_file(path)
        } else {
            eprintln!("  No .par file, using defaults");
            (0.001f32, 100i32, 1.0f32, 1.0f32, 1.0f32, 1.0f32, 1.0f32, 1.0f32, 1.0f32)
        };

    // ===========================================================================
    // 2. LOAD XCLBIN + CREATE KERNEL
    // ===========================================================================
    eprintln!("\n[2] Loading xclbin: {}", xclbin_path);
    check(unsafe { load_xclbin(xclbin_path.as_ptr(), xclbin_path.len() as i32) }, "load_xclbin");

    eprintln!("[3] Creating kernel: {}", KERNEL_NAME);
    check(unsafe { create_kernel(KERNEL_NAME.as_ptr(), KERNEL_NAME.len() as i32) }, "create_kernel");

    // ===========================================================================
    // 3. ALLOCATE BUFFERS (once)
    // ===========================================================================
    eprintln!("\n[4] Allocating buffers...");

    let size_inputdata   = (N_IN * 4) as i32;
    let size_outputdata  = (N_UT * 4) as i32;
    let size_rnd_poisson = (N_HID * 4) as i32;
    let size_hihjhi_ih   = (H_HID * DEN_HI_IH * 4) as i32;
    let size_bj_ih       = (N_HID * 4) as i32;
    let size_wji_ih      = (N_HID * DEN_NI_IH * 4) as i32;
    let size_bj_hu       = (N_UT * 4) as i32;
    let size_wji_hu      = (N_UT * DEN_NI_HU * 4) as i32;

    let buf_inputdata   = alloc_check(size_inputdata,   "inputdata");
    let buf_outputdata  = alloc_check(size_outputdata,   "outputdata");
    let buf_rnd_poisson = alloc_check(size_rnd_poisson,  "rndPoisson_hid");
    let buf_hihjhi_ih   = alloc_check(size_hihjhi_ih,    "Hihjhi_ih");
    let buf_bj_ih       = alloc_check(size_bj_ih,        "Bj_ih");
    let buf_wji_ih      = alloc_check(size_wji_ih,       "Wji_ih");
    let buf_bj_hu       = alloc_check(size_bj_hu,        "Bj_hu");
    let buf_wji_hu      = alloc_check(size_wji_hu,       "Wji_hu");

    // ===========================================================================
    // 4. WRITE STATIC WEIGHTS (once)
    // ===========================================================================
    eprintln!("\n[5] Writing weights...");
    write_check(buf_rnd_poisson, i32_as_bytes(&vec![0i32; N_HID]), "rndPoisson");
    write_check(buf_hihjhi_ih,   i32_as_bytes(&hihjhi_ih),         "Hihjhi_ih");
    write_check(buf_bj_ih,       f32_as_bytes(&bj_ih),             "Bj_ih");
    write_check(buf_wji_ih,      f32_as_bytes(&wji_ih),            "Wji_ih");
    write_check(buf_bj_hu,       f32_as_bytes(&bj_hu),             "Bj_hu");
    write_check(buf_wji_hu,      f32_as_bytes(&wji_hu),            "Wji_hu");

    // ===========================================================================
    // 5. SET KERNEL ARGUMENTS (once)
    // ===========================================================================
    eprintln!("\n[6] Setting kernel arguments...");
    check(unsafe { set_arg(0, buf_inputdata) },   "set_arg 0");
    check(unsafe { set_arg(1, buf_outputdata) },   "set_arg 1");
    check(unsafe { set_arg(2, buf_rnd_poisson) },  "set_arg 2");
    check(unsafe { set_arg(3, buf_hihjhi_ih) },    "set_arg 3");
    check(unsafe { set_arg(4, buf_bj_ih) },        "set_arg 4");
    check(unsafe { set_arg(5, buf_wji_ih) },       "set_arg 5");
    check(unsafe { set_arg(6, buf_bj_hu) },        "set_arg 6");
    check(unsafe { set_arg(7, buf_wji_hu) },       "set_arg 7");

    set_float(8,  nampl,   "nampl");
    check(unsafe { set_arg_int(9, nfreq) }, "set_arg_int 9");
    set_float(10, igain0,  "igain0");
    set_float(11, igain2,  "igain2");
    set_float(12, bwgain1, "bwgain1");
    set_float(13, bwgain2, "bwgain2");
    set_float(14, taumdt0, "taumdt0");
    set_float(15, taumdt1, "taumdt1");
    set_float(16, taumdt2, "taumdt2");

    // ===========================================================================
    // 6. PREPARE INPUT (synthetic digit '1')
    // ===========================================================================
    let inputdata = make_synthetic_digit_1();

    let input_bufs = [buf_inputdata, buf_rnd_poisson, buf_hihjhi_ih,
                      buf_bj_ih, buf_wji_ih, buf_bj_hu, buf_wji_hu];
    let output_bufs = [buf_outputdata];

    // ===========================================================================
    // 7. WARMUP (1 iteration, not counted)
    // ===========================================================================
    eprintln!("\n[7] Warmup iteration...");
    write_check(buf_inputdata, f32_as_bytes(&inputdata), "input");
    write_check(buf_outputdata, &vec![0u8; size_outputdata as usize], "output");
    check(unsafe {
        run(input_bufs.as_ptr(), input_bufs.len() as i32,
            output_bufs.as_ptr(), output_bufs.len() as i32)
    }, "warmup run");
    eprintln!("  Warmup complete");

    // ===========================================================================
    // 8. BENCHMARK LOOP
    // ===========================================================================
    eprintln!("\n[8] Running {} benchmark iterations...\n", bench_n);

    // CSV header on stdout
    println!("iteration,write_input_ns,run_ns,read_output_ns,total_ns,predicted_class");

    let mut timings: Vec<(u64, u64, u64, u64)> = Vec::with_capacity(bench_n);

    for iter in 0..bench_n {
        // Write input
        let t_write = Instant::now();
        write_check(buf_inputdata, f32_as_bytes(&inputdata), "input");
        write_check(buf_outputdata, &vec![0u8; size_outputdata as usize], "output");
        let write_ns = t_write.elapsed().as_nanos() as u64;

        // Run kernel
        let t_run = Instant::now();
        check(unsafe {
            run(input_bufs.as_ptr(), input_bufs.len() as i32,
                output_bufs.as_ptr(), output_bufs.len() as i32)
        }, "run");
        let run_ns = t_run.elapsed().as_nanos() as u64;

        // Read output
        let t_read = Instant::now();
        let mut output = vec![0.0f32; N_UT];
        let output_bytes = f32_as_bytes_mut(&mut output);
        check(unsafe { read(buf_outputdata, output_bytes.as_mut_ptr(), size_outputdata) }, "read");
        let read_ns = t_read.elapsed().as_nanos() as u64;

        let total_ns = write_ns + run_ns + read_ns;

        // Argmax
        let predicted = output.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, _)| i)
            .unwrap_or(0);

        // CSV row on stdout
        println!("{},{},{},{},{},{}", iter, write_ns, run_ns, read_ns, total_ns, predicted);

        timings.push((write_ns, run_ns, read_ns, total_ns));
    }

    // ===========================================================================
    // 9. SUMMARY STATISTICS (on stderr)
    // ===========================================================================
    if timings.is_empty() {
        eprintln!("\n=== No benchmark iterations (--bench 0). Skipping summary. ===");
        // Jump to cleanup
        eprintln!("\n[9] Freeing buffers...");
        unsafe {
            free(buf_inputdata);
            free(buf_outputdata);
            free(buf_rnd_poisson);
            free(buf_hihjhi_ih);
            free(buf_bj_ih);
            free(buf_wji_ih);
            free(buf_bj_hu);
            free(buf_wji_hu);
        }
        eprintln!("=== Benchmark Complete ===");
        return;
    }

    let n = timings.len() as f64;
    let mut run_times: Vec<u64> = timings.iter().map(|t| t.1).collect();
    let mut total_times: Vec<u64> = timings.iter().map(|t| t.3).collect();
    run_times.sort();
    total_times.sort();

    let run_min = run_times[0];
    let run_max = *run_times.last().unwrap();
    let run_mean = run_times.iter().sum::<u64>() as f64 / n;
    let run_median = if run_times.len() % 2 == 0 {
        (run_times[run_times.len() / 2 - 1] + run_times[run_times.len() / 2]) as f64 / 2.0
    } else {
        run_times[run_times.len() / 2] as f64
    };

    let total_min = total_times[0];
    let total_max = *total_times.last().unwrap();
    let total_mean = total_times.iter().sum::<u64>() as f64 / n;
    let total_median = if total_times.len() % 2 == 0 {
        (total_times[total_times.len() / 2 - 1] + total_times[total_times.len() / 2]) as f64 / 2.0
    } else {
        total_times[total_times.len() / 2] as f64
    };

    let throughput = n / (total_times.iter().sum::<u64>() as f64 / 1_000_000_000.0);

    eprintln!("\n=== BENCHMARK RESULTS ({} iterations) ===", timings.len());
    eprintln!("Run (WASI run() call):");
    eprintln!("  min:    {:.3} ms", run_min as f64 / 1_000_000.0);
    eprintln!("  max:    {:.3} ms", run_max as f64 / 1_000_000.0);
    eprintln!("  mean:   {:.3} ms", run_mean / 1_000_000.0);
    eprintln!("  median: {:.3} ms", run_median / 1_000_000.0);
    eprintln!("Total (write + run + read):");
    eprintln!("  min:    {:.3} ms", total_min as f64 / 1_000_000.0);
    eprintln!("  max:    {:.3} ms", total_max as f64 / 1_000_000.0);
    eprintln!("  mean:   {:.3} ms", total_mean / 1_000_000.0);
    eprintln!("  median: {:.3} ms", total_median / 1_000_000.0);
    eprintln!("Throughput: {:.1} inferences/sec", throughput);

    // ===========================================================================
    // 10. CLEANUP
    // ===========================================================================
    eprintln!("\n[9] Freeing buffers...");
    unsafe {
        free(buf_inputdata);
        free(buf_outputdata);
        free(buf_rnd_poisson);
        free(buf_hihjhi_ih);
        free(buf_bj_ih);
        free(buf_wji_ih);
        free(buf_bj_hu);
        free(buf_wji_hu);
    }
    eprintln!("=== Benchmark Complete ===");
}

// ===========================================================================
// HELPERS
// ===========================================================================

fn make_synthetic_digit_1() -> Vec<f32> {
    let mut inputdata = vec![0.0f32; N_IN];
    // Background: inputdata[2*i]=1.0, inputdata[2*i+1]=0.0
    for i in 0..784 {
        inputdata[2 * i] = 1.0;
        inputdata[2 * i + 1] = 0.0;
    }
    // Digit "1": central columns active
    for row in 4..24 {
        for col in 12..16 {
            let pixel_idx = row * 28 + col;
            if pixel_idx < 784 {
                inputdata[2 * pixel_idx] = 0.0;
                inputdata[2 * pixel_idx + 1] = 1.0;
            }
        }
    }
    inputdata
}

fn load_params_from_file(path: &str) -> (f32, i32, f32, f32, f32, f32, f32, f32, f32) {
    let mut params = std::collections::HashMap::new();
    match std::fs::read_to_string(path) {
        Ok(content) => {
            eprintln!("  Loading parameters from: {}", path);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    params.insert(parts[0].to_string(), parts[1].to_string());
                }
            }
            let get_f32 = |k: &str, d: f32| -> f32 {
                params.get(k).and_then(|s| s.parse().ok()).unwrap_or(d)
            };
            let get_i32 = |k: &str, d: i32| -> i32 {
                params.get(k).and_then(|s| s.parse().ok()).unwrap_or(d)
            };
            (get_f32("nampl", 0.001), get_i32("nfreq", 100),
             get_f32("igain0", 1.0), get_f32("igain2", 1.0),
             get_f32("bwgain1", 1.0), get_f32("bwgain2", 1.0),
             get_f32("taumdt0", 1.0), get_f32("taumdt1", 1.0), get_f32("taumdt2", 1.0))
        }
        Err(e) => {
            eprintln!("  Could not read params: {} — using defaults", e);
            (0.001, 100, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        }
    }
}

fn check(ret: i32, op: &str) {
    if ret != 0 {
        eprintln!("  {} FAILED (ret={})", op, ret);
        std::process::exit(1);
    }
}

fn alloc_check(size: i32, name: &str) -> i32 {
    let id = unsafe { alloc(size) };
    if id <= 0 {
        eprintln!("  alloc({}) FAILED for {}", size, name);
        std::process::exit(1);
    }
    eprintln!("  {} → buf_id={}", name, id);
    id
}

fn write_check(buf_id: i32, data: &[u8], _name: &str) {
    let ret = unsafe { write(buf_id, data.as_ptr(), data.len() as i32) };
    if ret != 0 {
        eprintln!("  write FAILED for buf_id={}", buf_id);
        std::process::exit(1);
    }
}

fn set_float(arg_idx: i32, value: f32, _name: &str) {
    let bits = value.to_bits() as i32;
    let ret = unsafe { set_arg_float(arg_idx, bits) };
    if ret != 0 {
        eprintln!("  set_arg_float({}) FAILED", arg_idx);
        std::process::exit(1);
    }
}

fn read_vec_f32(data: &[u8], offset: &mut usize) -> Vec<f32> {
    if *offset + 8 > data.len() { return Vec::new(); }
    let count = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() { return Vec::new(); }
    let vec: Vec<f32> = data[*offset..*offset + byte_len]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    *offset += byte_len;
    vec
}

fn read_vec_i32(data: &[u8], offset: &mut usize) -> Vec<i32> {
    if *offset + 8 > data.len() { return Vec::new(); }
    let count = u64::from_le_bytes(data[*offset..*offset + 8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() { return Vec::new(); }
    let vec: Vec<i32> = data[*offset..*offset + byte_len]
        .chunks_exact(4)
        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    *offset += byte_len;
    vec
}

fn f32_as_bytes(data: &[f32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
}

fn f32_as_bytes_mut(data: &mut [f32]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, data.len() * 4) }
}

fn i32_as_bytes(data: &[i32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
}
