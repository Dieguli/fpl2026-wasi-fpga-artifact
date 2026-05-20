//! Test de inferencia BCPNN en FPGA via WASI-FPGA
//!
//! Replica exactamente el flujo de bcpnn_artifacts:
//!   1. Carga pesos pre-entrenados de archivo .bin
//!   2. Carga BCPNN_infer_float.xclbin
//!   3. Crea kernel "BCPNN_infer_float"
//!   4. Aloca 8 buffers OpenCL
//!   5. Escribe pesos + datos de entrada (un dígito MNIST sintético)
//!   6. Asigna 17 argumentos (8 buffers + 9 escalares)
//!   7. Ejecuta: migrate inputs → enqueueTask → migrate output → finish
//!   8. Lee 10 floats de salida → argmax = clase predicha
//!
//! Requisitos en la ZCU104:
//!   - PAC "mnist_float" instalado: sudo xlnx-config -a mnist_float
//!   - xclbin + pesos copiados al board
//!
//! Compilar:
//!   rustc --target wasm32-wasip1 -o wasm/test_bcpnn_infer.wasm wasm/test_bcpnn_infer.rs
//!
//! Ejecutar:
//!   wasmedge test_bcpnn_infer.wasm <xclbin_path> <weights_bin_path>
//!
//! Ejemplo:
//!   wasmedge test_bcpnn_infer.wasm \
//!     ./BCPNN_infer_float.xclbin \
//!     ./alvis_fullmnist_32x128_64x64_eps-4.bin

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
// CONSTANTES DEL KERNEL (deben coincidir con Makefile: H_IN=784 M_IN=2 etc.)
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
    println!("=== WASI-FPGA BCPNN Inference Test ===");
    println!("Kernel: {}", KERNEL_NAME);
    println!("Network: {}x{} → {}x{} → {}x{}", H_IN, M_IN, H_HID, M_HID, H_UT, M_UT);
    println!("N_in={}, N_hid={}, N_ut={}", N_IN, N_HID, N_UT);
    println!("denHi_ih={}, denNi_ih={}, denNi_hu={}", DEN_HI_IH, DEN_NI_IH, DEN_NI_HU);

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Uso: wasmedge test_bcpnn_infer.wasm <xclbin_path> <weights_bin_path>");
        std::process::exit(1);
    }
    let xclbin_path = &args[1];
    let weights_path = &args[2];

    // ===========================================================================
    // 1. CARGAR PESOS PRE-ENTRENADOS
    // ===========================================================================
    println!("\n[1] Loading trained weights: {}", weights_path);

    let weights_data = std::fs::read(weights_path).expect("Failed to read weights file");
    println!("  File size: {} bytes", weights_data.len());

    // Formato del .bin: para cada vector: size_t (8 bytes LE) + data
    let mut offset = 0;

    let hihjhi_ih = read_vec_i32(&weights_data, &mut offset);     // int[]
    let bj_ih     = read_vec_f32(&weights_data, &mut offset);     // float[]
    let wji_ih    = read_vec_f32(&weights_data, &mut offset);     // float[]
    let bj_hu     = read_vec_f32(&weights_data, &mut offset);     // float[]
    let wji_hu    = read_vec_f32(&weights_data, &mut offset);     // float[]
    let constant_hbm = read_vec_f32(&weights_data, &mut offset);  // float[21]

    println!("  Hihjhi_ih: {} elements (expected {})", hihjhi_ih.len(), H_HID * DEN_HI_IH);
    println!("  Bj_ih:     {} elements (expected {})", bj_ih.len(), N_HID);
    println!("  Wji_ih:    {} elements (expected {})", wji_ih.len(), N_HID * DEN_NI_IH);
    println!("  Bj_hu:     {} elements (expected {})", bj_hu.len(), N_UT);
    println!("  Wji_hu:    {} elements (expected {})", wji_hu.len(), N_UT * DEN_NI_HU);
    println!("  Constants: {} elements (expected 21)", constant_hbm.len());

    if constant_hbm.len() >= 21 {
        println!("  constant_hbm[8]  (nampl) = {}", constant_hbm[8]);
        println!("  constant_hbm[9]  (nfreq) = {}", constant_hbm[9]);
        println!("  constant_hbm[13] (igain_hid) = {}", constant_hbm[13]);
        println!("  constant_hbm[14] (igain_ut)  = {}", constant_hbm[14]);
    }

    // Scalar parameters: use defaults (same as BCPNN reference)
    let (nampl, nfreq, igain0, igain2, bwgain1, bwgain2, taumdt0, taumdt1, taumdt2): (f32, i32, f32, f32, f32, f32, f32, f32, f32) = {
        println!("    Using default scalar parameters");
        (0.001, 100, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
    };
    
    println!("\n  Scalar args: nampl={}, nfreq={}, igain0={}, igain2={}", nampl, nfreq, igain0, igain2);
    println!("  bwgain1={}, bwgain2={}, taumdt0={}, taumdt1={}, taumdt2={}",
        bwgain1, bwgain2, taumdt0, taumdt1, taumdt2);

    // ===========================================================================
    // 2. CARGAR XCLBIN + CREAR KERNEL
    // ===========================================================================
    println!("\n[2] Loading xclbin: {}", xclbin_path);
    check(unsafe { load_xclbin(xclbin_path.as_ptr(), xclbin_path.len() as i32) }, "load_xclbin");

    println!("[3] Creating kernel: {}", KERNEL_NAME);
    check(unsafe { create_kernel(KERNEL_NAME.as_ptr(), KERNEL_NAME.len() as i32) }, "create_kernel");

    // ===========================================================================
    // 3. PREPARAR DATOS DE ENTRADA — un "1" sintético
    // ===========================================================================
    // El input es N_in=1568 floats (784 pixels × 2 para binarización complementaria)
    // Formato: inputdata[2*i] = 1-pixel, inputdata[2*i+1] = pixel
    // Creamos un dígito "1" simple: columnas centrales activas
    println!("\n[4] Creating synthetic input (digit '1')...");
    let mut inputdata = vec![0.0f32; N_IN];
    // Un "1" en 28x28: columna central (col 13-14) activa
    for row in 4..24 {
        for col in 12..16 {
            let pixel_idx = row * 28 + col;
            if pixel_idx < 784 {
                inputdata[2 * pixel_idx]     = 0.0;  // 1 - pixel
                inputdata[2 * pixel_idx + 1] = 1.0;  // pixel
            }
        }
    }
    // Resto ya es 0.0 (fondo): inputdata[2*i]=1-0=... bueno, el fondo debería
    // ser inputdata[2*i]=1.0, inputdata[2*i+1]=0.0
    for i in 0..784 {
        // Si el pixel no fue seteado arriba, es fondo (0)
        if inputdata[2*i] == 0.0 && inputdata[2*i+1] == 0.0 {
            inputdata[2*i]     = 1.0;  // 1 - 0 = 1
            inputdata[2*i + 1] = 0.0;  // 0
        }
    }
    let active_count = inputdata.iter().step_by(2).filter(|&&x| x < 0.5).count();
    println!("  Active pixels: {}/784", active_count);

    // Generar rndPoisson_hid (en la demo se genera con gnextpoisson, usamos constantes)
    let rnd_poisson_hid: Vec<i32> = (0..N_HID as i32).map(|i| (i * 7 + 3) % 200).collect();

    // ===========================================================================
    // 4. ALOCAR BUFFERS
    // ===========================================================================
    println!("\n[5] Allocating 8 OpenCL buffers...");

    let size_inputdata      = (N_IN * 4) as i32;                           // float[1568]
    let size_outputdata     = (N_UT * 4) as i32;                           // float[10]
    let size_rnd_poisson    = (N_HID * 4) as i32;                          // int[4096]
    let size_hihjhi_ih      = (H_HID * DEN_HI_IH * 4) as i32;            // int[4096]
    let size_bj_ih          = (N_HID * 4) as i32;                          // float[4096]
    let size_wji_ih         = (N_HID * DEN_NI_IH * 4) as i32;            // float[1048576]
    let size_bj_hu          = (N_UT * 4) as i32;                           // float[10]
    let size_wji_hu         = (N_UT * DEN_NI_HU * 4) as i32;             // float[40960]

    println!("  inputdata:      {} bytes", size_inputdata);
    println!("  outputdata:     {} bytes", size_outputdata);
    println!("  rndPoisson_hid: {} bytes", size_rnd_poisson);
    println!("  Hihjhi_ih:      {} bytes", size_hihjhi_ih);
    println!("  Bj_ih:          {} bytes", size_bj_ih);
    println!("  Wji_ih:         {} bytes ({}MB)", size_wji_ih, size_wji_ih / 1048576);
    println!("  Bj_hu:          {} bytes", size_bj_hu);
    println!("  Wji_hu:         {} bytes", size_wji_hu);

    let buf_inputdata   = alloc_check(size_inputdata,   "inputdata");
    let buf_outputdata  = alloc_check(size_outputdata,   "outputdata");
    let buf_rnd_poisson = alloc_check(size_rnd_poisson,  "rndPoisson_hid");
    let buf_hihjhi_ih   = alloc_check(size_hihjhi_ih,    "Hihjhi_ih");
    let buf_bj_ih       = alloc_check(size_bj_ih,        "Bj_ih");
    let buf_wji_ih      = alloc_check(size_wji_ih,       "Wji_ih");
    let buf_bj_hu       = alloc_check(size_bj_hu,        "Bj_hu");
    let buf_wji_hu      = alloc_check(size_wji_hu,       "Wji_hu");

    // ===========================================================================
    // 5. ESCRIBIR DATOS EN BUFFERS
    // ===========================================================================
    println!("\n[6] Writing data to buffers...");

    write_check(buf_inputdata,   f32_as_bytes(&inputdata),                   "inputdata");
    write_check(buf_outputdata,  &vec![0u8; size_outputdata as usize],       "outputdata (zeros)");
    write_check(buf_rnd_poisson, i32_as_bytes(&rnd_poisson_hid),             "rndPoisson_hid");
    write_check(buf_hihjhi_ih,   i32_as_bytes(&hihjhi_ih),                   "Hihjhi_ih");
    write_check(buf_bj_ih,       f32_as_bytes(&bj_ih),                       "Bj_ih");
    write_check(buf_wji_ih,      f32_as_bytes(&wji_ih),                      "Wji_ih");
    write_check(buf_bj_hu,       f32_as_bytes(&bj_hu),                       "Bj_hu");
    write_check(buf_wji_hu,      f32_as_bytes(&wji_hu),                      "Wji_hu");

    // ===========================================================================
    // 6. ASIGNAR ARGUMENTOS DEL KERNEL
    // ===========================================================================
    // Firma: BCPNN_infer_float(
    //   float *input_hbm,           // arg 0 - buffer
    //   float *output_hbm,          // arg 1 - buffer
    //   int   *rndPoisson_hid_hbm,  // arg 2 - buffer
    //   int   *Hihjhi_ih_hbm,       // arg 3 - buffer
    //   float *Bj_ih_hbm,           // arg 4 - buffer
    //   float *Wji_ih_hbm,          // arg 5 - buffer
    //   float *Bj_hu_hbm,           // arg 6 - buffer
    //   float *Wji_hu_hbm,          // arg 7 - buffer
    //   float nampl,                // arg 8 - scalar float
    //   int   nfreq,                // arg 9 - scalar int
    //   float igain0,               // arg 10 - scalar float
    //   float igain2,               // arg 11 - scalar float
    //   float bwgain1,              // arg 12 - scalar float
    //   float bwgain2,              // arg 13 - scalar float
    //   float taumdt0,              // arg 14 - scalar float
    //   float taumdt1,              // arg 15 - scalar float
    //   float taumdt2               // arg 16 - scalar float
    // )
    println!("\n[7] Setting kernel arguments (8 buffers + 9 scalars)...");

    check(unsafe { set_arg(0, buf_inputdata) },   "set_arg(0, inputdata)");
    check(unsafe { set_arg(1, buf_outputdata) },   "set_arg(1, outputdata)");
    check(unsafe { set_arg(2, buf_rnd_poisson) },  "set_arg(2, rndPoisson)");
    check(unsafe { set_arg(3, buf_hihjhi_ih) },    "set_arg(3, Hihjhi_ih)");
    check(unsafe { set_arg(4, buf_bj_ih) },        "set_arg(4, Bj_ih)");
    check(unsafe { set_arg(5, buf_wji_ih) },       "set_arg(5, Wji_ih)");
    check(unsafe { set_arg(6, buf_bj_hu) },        "set_arg(6, Bj_hu)");
    check(unsafe { set_arg(7, buf_wji_hu) },       "set_arg(7, Wji_hu)");

    set_float(8,  nampl,   "nampl");
    check(unsafe { set_arg_int(9, nfreq) }, "set_arg_int(9, nfreq)");
    set_float(10, igain0,  "igain0");
    set_float(11, igain2,  "igain2");
    set_float(12, bwgain1, "bwgain1");
    set_float(13, bwgain2, "bwgain2");
    set_float(14, taumdt0, "taumdt0");
    set_float(15, taumdt1, "taumdt1");
    set_float(16, taumdt2, "taumdt2");

    println!("   All 17 arguments set");

    // ===========================================================================
    // 7. EJECUTAR KERNEL
    // ===========================================================================
    // Inputs a migrar: todos los buffers de lectura (7 buffers, no outputdata)
    let input_bufs = [buf_inputdata, buf_rnd_poisson, buf_hihjhi_ih,
                      buf_bj_ih, buf_wji_ih, buf_bj_hu, buf_wji_hu];
    let output_bufs = [buf_outputdata];

    println!("\n[8] Running kernel (migrate {} inputs → execute → migrate output)...",
        input_bufs.len());
    let t_run = Instant::now();
    check(unsafe {
        run(
            input_bufs.as_ptr(), input_bufs.len() as i32,
            output_bufs.as_ptr(), output_bufs.len() as i32,
        )
    }, "run");
    let run_elapsed = t_run.elapsed();
    println!("  Kernel execution wall time: {:.3} ms", run_elapsed.as_secs_f64() * 1000.0);

    // ===========================================================================
    // 8. LEER RESULTADO
    // ===========================================================================
    println!("\n[9] Reading output ({} floats)...", N_UT);
    let mut output = vec![0.0f32; N_UT];
    let output_bytes = f32_as_bytes_mut(&mut output);
    check(unsafe { read(buf_outputdata, output_bytes.as_mut_ptr(), size_outputdata) }, "read output");

    // ===========================================================================
    // 9. INTERPRETAR RESULTADO
    // ===========================================================================
    println!("\n=== RESULTADO DE INFERENCIA ===");
    println!("  Output (10 clases):");
    for i in 0..N_UT {
        let bar_len = ((output[i].abs() * 50.0).min(50.0)) as usize;
        let bar: String = "█".repeat(bar_len);
        println!("    Clase {}: {:>12.6}  {}", i, output[i], bar);
    }

    // Argmax
    let (max_idx, max_val) = output.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    let all_zero = output.iter().all(|&x| x == 0.0);
    let all_nan = output.iter().all(|x| x.is_nan());
    let all_same = output.iter().all(|&x| x == output[0]);

    println!("\n  Predicted class: {} (value: {:.6})", max_idx, max_val);

    if all_zero {
        println!("    TODOS CEROS — el kernel no escribió resultado");
        println!("  → Verificar que el PAC mnist_float esté instalado");
    } else if all_nan {
        println!("   TODOS NaN — mismo problema que vadd con plataforma base");
    } else if all_same {
        println!("    TODOS IGUALES ({:.6}) — kernel ejecutó pero no diferenció", output[0]);
    } else {
        println!("   ¡Resultado diferenciado! La FPGA está computando.");
        if max_idx == 1 {
            println!("   ¡Predijo clase 1 correctamente para nuestro dígito '1' sintético!");
        } else {
            println!("  Nota: Predijo clase {} (el input sintético puede no ser perfecto)", max_idx);
        }
    }

    // ===========================================================================
    // 10. CLEANUP
    // ===========================================================================
    println!("\n[10] Freeing buffers...");
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
    println!("   Done");
    println!("\n=== Test Complete ===");
}

// ===========================================================================
// HELPERS
// ===========================================================================

fn check(ret: i32, op: &str) {
    if ret != 0 {
        println!("   {} FAILED (ret={})", op, ret);
        std::process::exit(1);
    }
}

fn alloc_check(size: i32, name: &str) -> i32 {
    let id = unsafe { alloc(size) };
    if id <= 0 {
        println!("   alloc({}) FAILED for {}", size, name);
        std::process::exit(1);
    }
    println!("   {} → buf_id={} ({} bytes)", name, id, size);
    id
}

fn write_check(buf_id: i32, data: &[u8], name: &str) {
    let ret = unsafe { write(buf_id, data.as_ptr(), data.len() as i32) };
    if ret != 0 {
        println!("   write {} FAILED", name);
        std::process::exit(1);
    }
    println!("   {} written ({} bytes)", name, data.len());
}

fn set_float(arg_idx: i32, value: f32, name: &str) {
    let bits = value.to_bits() as i32;
    let ret = unsafe { set_arg_float(arg_idx, bits) };
    if ret != 0 {
        println!("   set_arg_float({}, {} [{}]) FAILED", arg_idx, name, value);
        std::process::exit(1);
    }
}

/// Lee un vector del archivo .bin (formato: size_t LE + data como float[])
fn read_vec_f32(data: &[u8], offset: &mut usize) -> Vec<f32> {
    if *offset + 8 > data.len() {
        println!("    read_vec_f32: offset {} + 8 > len {}", offset, data.len());
        return Vec::new();
    }
    let count = u64::from_le_bytes(data[*offset..*offset+8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() {
        println!("    read_vec_f32: need {} bytes at offset {}, have {}", byte_len, offset, data.len());
        return Vec::new();
    }
    let vec: Vec<f32> = data[*offset..*offset+byte_len]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    *offset += byte_len;
    vec
}

/// Lee un vector del archivo .bin (formato: size_t LE + data como int[])
fn read_vec_i32(data: &[u8], offset: &mut usize) -> Vec<i32> {
    if *offset + 8 > data.len() {
        println!("    read_vec_i32: offset {} + 8 > len {}", offset, data.len());
        return Vec::new();
    }
    let count = u64::from_le_bytes(data[*offset..*offset+8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() {
        println!("    read_vec_i32: need {} bytes at offset {}, have {}", byte_len, offset, data.len());
        return Vec::new();
    }
    let vec: Vec<i32> = data[*offset..*offset+byte_len]
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
