//! Test de inferencia BCPNN con video en FPGA via WASI-FPGA
//!
//! Procesa un archivo de video (frame por frame) o genera múltiples dígitos sintéticos
//! y corre inferencia BCPNN en lote.
//!
//! Requisitos en la ZCU104:
//!   - PAC "mnist_float" instalado: sudo xlnx-config -a mnist_float
//!   - xclbin + pesos copiados al board
//!
//! Compilar:
//!   rustc --target wasm32-wasip1 --edition 2021 -o wasm/test_bcpnn_infer_video.wasm wasm/test_bcpnn_infer_video.rs
//!
//! Ejecutar:
//!   wasmedge test_bcpnn_infer_video.wasm <xclbin_path> <weights_bin_path> [video_file_path]
//!
//! Si no se proporciona video, genera dígitos sintéticos (0-9)

use std::convert::TryInto;

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
// CONSTANTES DEL KERNEL
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

// ===========================================================================
// HELPERS
// ===========================================================================

/// Lee parámetros desde un archivo .par (formato: clave valor # comentario)
/// Retorna: (nampl, nfreq, igain0, igain2, bwgain1, bwgain2, taumdt0, taumdt1, taumdt2)
fn load_params_from_file(path: &str) -> (f32, i32, f32, f32, f32, f32, f32, f32, f32) {
    let mut params = std::collections::HashMap::new();
    
    match std::fs::read_to_string(path) {
        Ok(content) => {
            println!("  Loading parameters from: {}", path);
            for line in content.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                // Formato: clave valor [# comentario]
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    params.insert(parts[0], parts[1]);
                }
            }
            
            // Extraer valores (con defaults fallback)
            let nampl: f32 = params.get("nampl")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0.001);
            let nfreq: i32 = params.get("nfreq")
                .and_then(|s| s.parse().ok())
                .unwrap_or(100);
            let igain0: f32 = params.get("igain0")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let igain2: f32 = params.get("igain2")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let bwgain1: f32 = params.get("bwgain1")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let bwgain2: f32 = params.get("bwgain2")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let taumdt0: f32 = params.get("taumdt0")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let taumdt1: f32 = params.get("taumdt1")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            let taumdt2: f32 = params.get("taumdt2")
                .and_then(|s| s.parse().ok())
                .unwrap_or(1.0);
            
            println!("   Params loaded: nampl={}, nfreq={}", nampl, nfreq);
            (nampl, nfreq, igain0, igain2, bwgain1, bwgain2, taumdt0, taumdt1, taumdt2)
        }
        Err(e) => {
            println!("    Could not read params file: {} — using defaults", e);
            (0.001, 100, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        }
    }
}

fn main() {
    println!("=== WASI-FPGA BCPNN Video Inference Test ===");
    println!("Kernel: {}", KERNEL_NAME);

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Uso: wasmedge test_bcpnn_infer_video.wasm <xclbin_path> <weights_bin_path> [params_file_path] [video_file_path]");
        std::process::exit(1);
    }
    let xclbin_path = &args[1];
    let weights_path = &args[2];
    
    // Detectar si arg 3 es .par (params) o video
    let (params_path, video_path) = if args.len() >= 4 && args[3].ends_with(".par") {
        (Some(args[3].as_str()), args.get(4).map(|s| s.as_str()))
    } else {
        (None, args.get(3).map(|s| s.as_str()))
    };

    // ===========================================================================
    // 1. CARGAR PESOS
    // ===========================================================================
    println!("\n[1] Loading trained weights: {}", weights_path);

    let weights_data = std::fs::read(weights_path).expect("Failed to read weights file");
    println!("  File size: {} bytes", weights_data.len());

    let mut offset = 0;
    let hihjhi_ih = read_vec_i32(&weights_data, &mut offset);
    let bj_ih     = read_vec_f32(&weights_data, &mut offset);
    let wji_ih    = read_vec_f32(&weights_data, &mut offset);
    let bj_hu     = read_vec_f32(&weights_data, &mut offset);
    let wji_hu    = read_vec_f32(&weights_data, &mut offset);
    let constant_hbm = read_vec_f32(&weights_data, &mut offset);

    println!("   Weights loaded: Hihjhi_ih={}, Bj_ih={}, Wji_ih={}, Bj_hu={}, Wji_hu={}, Constants={}",
        hihjhi_ih.len(), bj_ih.len(), wji_ih.len(), bj_hu.len(), wji_hu.len(), constant_hbm.len());

    // Cargar parámetros (desde .par o defaults)
    println!("\n[2] Loading parameters");
    let (nampl, nfreq, igain0, igain2, bwgain1, bwgain2, taumdt0, taumdt1, taumdt2) = 
        if let Some(params_file) = params_path {
            load_params_from_file(params_file)
        } else {
            println!("  No .par file provided, using defaults");
            (0.001, 100, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0)
        };
    let taumdt2: f32 = if constant_hbm.len() > 20 { constant_hbm[20] } else { 1.0 };

    // ===========================================================================
    // 2. CARGAR XCLBIN + CREAR KERNEL
    // ===========================================================================
    println!("\n[2] Loading xclbin: {}", xclbin_path);
    check(unsafe { load_xclbin(xclbin_path.as_ptr(), xclbin_path.len() as i32) }, "load_xclbin");

    println!("[3] Creating kernel: {}", KERNEL_NAME);
    check(unsafe { create_kernel(KERNEL_NAME.as_ptr(), KERNEL_NAME.len() as i32) }, "create_kernel");

    // ===========================================================================
    // 4. ALOCAR BUFFERS (una sola vez)
    // ===========================================================================
    println!("\n[4] Allocating OpenCL buffers...");

    let size_inputdata      = (N_IN * 4) as i32;
    let size_outputdata     = (N_UT * 4) as i32;
    let size_rnd_poisson    = (N_HID * 4) as i32;
    let size_hihjhi_ih      = (H_HID * DEN_HI_IH * 4) as i32;
    let size_bj_ih          = (N_HID * 4) as i32;
    let size_wji_ih         = (N_HID * DEN_NI_IH * 4) as i32;
    let size_bj_hu          = (N_UT * 4) as i32;
    let size_wji_hu         = (N_UT * DEN_NI_HU * 4) as i32;

    let buf_inputdata   = alloc_check(size_inputdata,   "inputdata");
    let buf_outputdata  = alloc_check(size_outputdata,   "outputdata");
    let buf_rnd_poisson = alloc_check(size_rnd_poisson,  "rndPoisson_hid");
    let buf_hihjhi_ih   = alloc_check(size_hihjhi_ih,    "Hihjhi_ih");
    let buf_bj_ih       = alloc_check(size_bj_ih,        "Bj_ih");
    let buf_wji_ih      = alloc_check(size_wji_ih,       "Wji_ih");
    let buf_bj_hu       = alloc_check(size_bj_hu,        "Bj_hu");
    let buf_wji_hu      = alloc_check(size_wji_hu,       "Wji_hu");

    // ===========================================================================
    // 5. ESCRIBIR PESOS EN BUFFERS (una sola vez)
    // ===========================================================================
    println!("\n[5] Writing weights to buffers (static data)...");

    write_check(buf_rnd_poisson, i32_as_bytes(&vec![0i32; N_HID]), "rndPoisson_hid");
    write_check(buf_hihjhi_ih,   i32_as_bytes(&hihjhi_ih),         "Hihjhi_ih");
    write_check(buf_bj_ih,       f32_as_bytes(&bj_ih),             "Bj_ih");
    write_check(buf_wji_ih,      f32_as_bytes(&wji_ih),            "Wji_ih");
    write_check(buf_bj_hu,       f32_as_bytes(&bj_hu),             "Bj_hu");
    write_check(buf_wji_hu,      f32_as_bytes(&wji_hu),            "Wji_hu");

    // ===========================================================================
    // 6. ASIGNAR ARGUMENTOS DEL KERNEL (una sola vez)
    // ===========================================================================
    println!("\n[6] Setting kernel arguments...");

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

    // ===========================================================================
    // 7. GENERAR O LEER FRAMES
    // ===========================================================================
    let frames = if let Some(path) = video_path {
        println!("\n[7] Reading video frames from: {}", path);
        read_video_frames(path)
    } else {
        println!("\n[7] Generating synthetic digit frames (0-9)...");
        generate_synthetic_frames()
    };

    println!("  Total frames to process: {}", frames.len());

    // ===========================================================================
    // 8. PROCESAR FRAMES EN LOTE
    // ===========================================================================
    println!("\n[8] Processing frames...\n");

    let mut total_correct = 0;
    let mut predictions = vec![0i32; 10];

    let input_bufs = [buf_inputdata, buf_rnd_poisson, buf_hihjhi_ih,
                      buf_bj_ih, buf_wji_ih, buf_bj_hu, buf_wji_hu];
    let output_bufs = [buf_outputdata];

    for (frame_idx, (input_frame, expected_label)) in frames.iter().enumerate() {
        // Escribir input
        write_check(buf_inputdata, f32_as_bytes(input_frame), "inputdata");
        write_check(buf_outputdata, &vec![0u8; size_outputdata as usize], "outputdata");

        // Ejecutar kernel
        let ret = unsafe {
            run(
                input_bufs.as_ptr(), input_bufs.len() as i32,
                output_bufs.as_ptr(), output_bufs.len() as i32,
            )
        };

        if ret != 0 {
            println!("   Frame {}: kernel failed", frame_idx);
            continue;
        }

        // Leer output
        let mut output = vec![0.0f32; N_UT];
        let output_bytes = f32_as_bytes_mut(&mut output);
        unsafe { read(buf_outputdata, output_bytes.as_mut_ptr(), size_outputdata) };

        // Argmax
        let (pred_class, confidence) = output.iter().enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, &v)| (i as i32, v))
            .unwrap_or((0, 0.0));

        predictions[pred_class as usize] += 1;

        let correct = if pred_class == *expected_label { "" } else { "" };
        if pred_class == *expected_label {
            total_correct += 1;
        }

        println!("  Frame {}: expected={}, predicted={}, conf={:.4} {}", 
            frame_idx, expected_label, pred_class, confidence, correct);
    }

    // ===========================================================================
    // 9. ESTADÍSTICAS
    // ===========================================================================
    println!("\n=== ESTADÍSTICAS DE INFERENCIA ===");
    println!("Total frames: {}", frames.len());
    println!("Correctas: {} ({:.1}%)", total_correct, 
        (total_correct as f32 / frames.len() as f32) * 100.0);
    println!("\nPredicciones por clase:");
    for (class, count) in predictions.iter().enumerate() {
        if *count > 0 {
            println!("  Clase {}: {} frames", class, count);
        }
    }

    // ===========================================================================
    // 10. CLEANUP
    // ===========================================================================
    println!("\n[9] Freeing buffers...");
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
// SYNTHETIC FRAMES
// ===========================================================================

/// Genera 10 frames sintéticos (dígitos 0-9)
fn generate_synthetic_frames() -> Vec<(Vec<f32>, i32)> {
    let mut frames = Vec::new();

    // Patrón para cada dígito (simplificado)
    let patterns = [
        (3..7, 8..20),    // 0: óvalo
        (8..12, 8..20),   // 1: línea vertical derecha
        (3..10, 5..15),   // 2: patrón superior-inferior
        (3..10, 8..15),   // 3: línea central
        (8..12, 5..15),   // 4: línea vertical derecha con horizontal
        (3..10, 5..12),   // 5: patrón superior
        (3..7, 8..20),    // 6: óvalo bajo
        (3..10, 3..12),   // 7: línea horizontal superior
        (3..7, 5..20),    // 8: óvalo completo
        (3..7, 8..15),    // 9: óvalo con cola
    ];

    for (label, (row_range, col_range)) in patterns.iter().enumerate() {
        let mut inputdata = vec![0.0f32; N_IN];

        // Inicializar fondo (0): inputdata[2*i]=1.0, inputdata[2*i+1]=0.0
        for i in 0..784 {
            inputdata[2*i]     = 1.0;
            inputdata[2*i + 1] = 0.0;
        }

        // Aplicar patrón de dígito (píxeles activos)
        for row in row_range.clone() {
            for col in col_range.clone() {
                if row < 28 && col < 28 {
                    let pixel_idx = row * 28 + col;
                    inputdata[2 * pixel_idx]     = 0.0;  // 1 - pixel
                    inputdata[2 * pixel_idx + 1] = 1.0;  // pixel
                }
            }
        }

        frames.push((inputdata, label as i32));
    }

    frames
}

/// Intenta leer frames de un archivo .bin (formato: para cada frame: 1568*4 bytes)
fn read_video_frames(path: &str) -> Vec<(Vec<f32>, i32)> {
    let mut frames = Vec::new();
    
    match std::fs::read(path) {
        Ok(data) => {
            let frame_size = N_IN * 4; // 1568 floats * 4 bytes
            let num_frames = data.len() / frame_size;

            println!("  File size: {} bytes → {} frames", data.len(), num_frames);

            for frame_idx in 0..num_frames {
                let offset = frame_idx * frame_size;
                let frame_data = &data[offset..offset + frame_size];

                let frame: Vec<f32> = frame_data
                    .chunks_exact(4)
                    .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
                    .collect();

                // Etiqueta: se asume que el nombre del archivo o índice contiene el dígito
                // Si no, usar dígito predictivo
                let label = (frame_idx % 10) as i32; // Fallback: cíclico

                frames.push((frame, label));
            }
        }
        Err(e) => {
            println!("    Could not read video file: {} — using synthetic frames instead", e);
            return generate_synthetic_frames();
        }
    }

    if frames.is_empty() {
        println!("    No frames read — using synthetic instead");
        return generate_synthetic_frames();
    }

    frames
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
    println!("   {} → buf_id={}", name, id);
    id
}

fn write_check(buf_id: i32, data: &[u8], name: &str) {
    let ret = unsafe { write(buf_id, data.as_ptr(), data.len() as i32) };
    if ret != 0 {
        println!("   write {} FAILED", name);
        std::process::exit(1);
    }
}

fn set_float(arg_idx: i32, value: f32, _name: &str) {
    let bits = value.to_bits() as i32;
    let ret = unsafe { set_arg_float(arg_idx, bits) };
    if ret != 0 {
        println!("   set_arg_float({}) FAILED", arg_idx);
        std::process::exit(1);
    }
}

fn read_vec_f32(data: &[u8], offset: &mut usize) -> Vec<f32> {
    if *offset + 8 > data.len() {
        return Vec::new();
    }
    let count = u64::from_le_bytes(data[*offset..*offset+8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() {
        return Vec::new();
    }
    let vec: Vec<f32> = data[*offset..*offset+byte_len]
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    *offset += byte_len;
    vec
}

fn read_vec_i32(data: &[u8], offset: &mut usize) -> Vec<i32> {
    if *offset + 8 > data.len() {
        return Vec::new();
    }
    let count = u64::from_le_bytes(data[*offset..*offset+8].try_into().unwrap()) as usize;
    *offset += 8;
    let byte_len = count * 4;
    if *offset + byte_len > data.len() {
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
