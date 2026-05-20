//! Test de inferencia BCPNN via WASI-FPGA (backend OpenCL)
//!
//! Este módulo WASM replica el flujo de mnistmain_FPGA_inference.cpp
//! pero ejecutándose dentro de WasmEdge con el plugin WASI-FPGA.
//!
//! El kernel BCPNN tiene 21 argumentos:
//!   0: buf_inputdata   (H_in * sizeof(float))     → 784 floats
//!   1: buf_labeldata   (1 * sizeof(float))         → 1 float
//!   2: buf_outputdata  (M_hid * sizeof(float))     → 128 floats
//!   3: modeOps         (int scalar)                → 2 = inference
//!   4..20: weight/state buffers de distintos tamaños
//!
//! Compilar:
//!   rustc --target wasm32-wasip1 -o test_bcpnn_inference.wasm test_bcpnn_inference.rs
//!
//! Ejecutar:
//!   wasmedge test_bcpnn_inference.wasm ./BCPNN_Kernel.xclbin ./input_image.bin

// ============================================================================
// IMPORTS DEL PLUGIN
// ============================================================================

#[link(wasm_import_module = "fpga")]
extern "C" {
    fn load_xclbin(path_ptr: *const u8, path_len: i32) -> i32;
    fn create_kernel(name_ptr: *const u8, name_len: i32) -> i32;
    fn alloc(size: i32) -> i32;
    fn write(buf_id: i32, data_ptr: *const u8, data_len: i32) -> i32;
    fn read(buf_id: i32, data_ptr: *mut u8, data_len: i32) -> i32;
    fn set_arg(arg_idx: i32, buf_id: i32) -> i32;
    fn set_arg_int(arg_idx: i32, value: i32) -> i32;
    fn run(in_ids_ptr: *const i32, in_ids_len: i32,
           out_ids_ptr: *const i32, out_ids_len: i32) -> i32;
    fn free(buf_id: i32) -> i32;
}

// ============================================================================
// CONSTANTES BCPNN (de BCPNN_Kernel.h)
// ============================================================================

const H_IN: usize = 784;    // 28*28 pixels MNIST
const M_IN: usize = 2;      // Minicolumnas entrada
const H_HID: usize = 32;    // Hipercolumnas ocultas
const M_HID: usize = 128;   // Minicolumnas ocultas
const N_IN: usize = H_IN * M_IN;     // 1568
const N_HID: usize = H_HID * M_HID;  // 4096

const KERNEL_NAME: &str = "BCPNN_Kernel";
const MODE_INFERENCE: i32 = 2;

fn main() {
    println!("=== BCPNN Inference via WASI-FPGA (OpenCL) ===");
    println!("H_in={}, M_in={}, H_hid={}, M_hid={}", H_IN, M_IN, H_HID, M_HID);
    println!("N_in={}, N_hid={}", N_IN, N_HID);

    // --- Argumentos ---
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Uso: wasmedge test_bcpnn_inference.wasm <xclbin_path> [input_image.bin]");
        println!("  El xclbin debe ser del PAC stream_32x128_SP");
        std::process::exit(1);
    }
    let xclbin_path = &args[1];

    // --- 1. Cargar xclbin ---
    println!("\n[1] Loading xclbin: {}", xclbin_path);
    check(unsafe { load_xclbin(xclbin_path.as_ptr(), xclbin_path.len() as i32) }, "load_xclbin");

    // --- 2. Crear kernel ---
    println!("[2] Creating kernel: {}", KERNEL_NAME);
    check(unsafe { create_kernel(KERNEL_NAME.as_ptr(), KERNEL_NAME.len() as i32) }, "create_kernel");

    // --- 3. Alocar los 21 buffers ---
    println!("[3] Allocating 21 BCPNN buffers...");

    let sz_f = std::mem::size_of::<f32>() as i32;

    // Buffer sizes (en floats) según BCPNN_Kernel.h
    let buf_inputdata     = alloc_checked(N_IN as i32 * sz_f, "inputdata");
    let buf_labeldata     = alloc_checked(1 * sz_f, "labeldata");
    let buf_outputdata    = alloc_checked(M_HID as i32 * sz_f, "outputdata");
    // modeOps es un scalar int, no un buffer

    let buf_rndpoisson_hid = alloc_checked(N_HID as i32 * sz_f, "rndPoisson_hid");
    let buf_hihjhi_ih      = alloc_checked((H_IN * H_HID) as i32 * sz_f, "Hihjhi_ih");
    let buf_chjhi_ih       = alloc_checked((H_IN * H_HID) as i32 * sz_f, "Chjhi_ih");

    let buf_pj_ih   = alloc_checked(N_IN as i32 * sz_f, "Pj_ih");
    let buf_pi_ih   = alloc_checked(N_HID as i32 * sz_f, "Pi_ih");
    let buf_pji_ih  = alloc_checked((N_IN * N_HID) as i32 * sz_f, "Pji_ih");
    let buf_bj_ih   = alloc_checked(N_HID as i32 * sz_f, "Bj_ih");
    let buf_wji_ih  = alloc_checked((N_IN * N_HID) as i32 * sz_f, "Wji_ih");
    let buf_wji_ih1 = alloc_checked((N_IN * N_HID) as i32 * sz_f, "Wji_ih1");
    let buf_wji_ih2 = alloc_checked((N_IN * N_HID) as i32 * sz_f, "Wji_ih2");

    let buf_pj_hu          = alloc_checked(N_HID as i32 * sz_f, "Pj_hu");
    let buf_pi_hu          = alloc_checked(N_HID as i32 * sz_f, "Pi_hu");
    let buf_pji_hu         = alloc_checked((N_HID * N_HID) as i32 * sz_f, "Pji_hu");
    let buf_bj_hu          = alloc_checked(N_HID as i32 * sz_f, "Bj_hu");
    let buf_wji_hu         = alloc_checked((N_HID * N_HID) as i32 * sz_f, "Wji_hu");
    let buf_needsupdbw     = alloc_checked(1 * sz_f, "needsupdbw");

    println!("   20 buffers allocated");

    // --- 4. Inicializar datos de entrada ---
    println!("[4] Initializing input data...");

    // Input: imagen MNIST (784 pixels * 2 minicolumnas = 1568 floats)
    // Para test, usamos un patrón simple
    let mut inputdata = vec![0.0f32; N_IN];
    if args.len() > 2 {
        // Cargar desde archivo si se proporciona
        let img_path = &args[2];
        if let Ok(bytes) = std::fs::read(img_path) {
            let floats: &[f32] = unsafe {
                std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4)
            };
            let copy_len = floats.len().min(N_IN);
            inputdata[..copy_len].copy_from_slice(&floats[..copy_len]);
            println!("  Loaded {} floats from {}", copy_len, img_path);
        } else {
            println!("  Warning: could not read {}, using zeros", img_path);
        }
    } else {
        println!("  Using zero input (no image file provided)");
    }

    // Escribir inputdata
    write_floats(buf_inputdata, &inputdata);

    // Label (no importa en inferencia, poner 0)
    write_floats(buf_labeldata, &[0.0f32]);

    // Output buffer (zeros)
    write_floats(buf_outputdata, &vec![0.0f32; M_HID]);

    // rndPoisson (zeros para inferencia)
    write_floats(buf_rndpoisson_hid, &vec![0.0f32; N_HID]);

    // needsupdbw
    write_floats(buf_needsupdbw, &[0.0f32]);

    // Nota: Los buffers de pesos (Wji, Bj, Pj, Pi, Pji, etc.)
    // necesitan cargarse desde archivos .bin entrenados.
    // Para este test de infraestructura, los dejamos en zero.
    println!("    Weight buffers initialized to zero (no trained weights loaded)");
    println!("  → El resultado será puramente de la ejecución FPGA, no clasificación real");

    // --- 5. Set kernel arguments ---
    println!("[5] Setting 21 kernel arguments...");
    let mut arg = 0;
    check(unsafe { set_arg(arg, buf_inputdata) }, "set_arg(0, inputdata)"); arg += 1;
    check(unsafe { set_arg(arg, buf_labeldata) }, "set_arg(1, labeldata)"); arg += 1;
    check(unsafe { set_arg(arg, buf_outputdata) }, "set_arg(2, outputdata)"); arg += 1;
    check(unsafe { set_arg_int(arg, MODE_INFERENCE) }, "set_arg_int(3, modeOps=2)"); arg += 1;
    check(unsafe { set_arg(arg, buf_rndpoisson_hid) }, "set_arg(4)"); arg += 1;
    check(unsafe { set_arg(arg, buf_hihjhi_ih) }, "set_arg(5)"); arg += 1;
    check(unsafe { set_arg(arg, buf_chjhi_ih) }, "set_arg(6)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pj_ih) }, "set_arg(7)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pi_ih) }, "set_arg(8)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pji_ih) }, "set_arg(9)"); arg += 1;
    check(unsafe { set_arg(arg, buf_bj_ih) }, "set_arg(10)"); arg += 1;
    check(unsafe { set_arg(arg, buf_wji_ih) }, "set_arg(11)"); arg += 1;
    check(unsafe { set_arg(arg, buf_wji_ih1) }, "set_arg(12)"); arg += 1;
    check(unsafe { set_arg(arg, buf_wji_ih2) }, "set_arg(13)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pj_hu) }, "set_arg(14)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pi_hu) }, "set_arg(15)"); arg += 1;
    check(unsafe { set_arg(arg, buf_pji_hu) }, "set_arg(16)"); arg += 1;
    check(unsafe { set_arg(arg, buf_bj_hu) }, "set_arg(17)"); arg += 1;
    check(unsafe { set_arg(arg, buf_wji_hu) }, "set_arg(18)"); arg += 1;
    // arg 19: unused en algunas versiones, o buf_Wji_hu1
    // arg 20: buf_needsupdbw
    check(unsafe { set_arg(arg, buf_needsupdbw) }, "set_arg(19, needsupdbw)"); arg += 1;
    let _ = arg;  // suppress unused warning
    println!("   {} arguments set", arg);

    // --- 6. Ejecutar kernel ---
    println!("[6] Running BCPNN_Kernel (inference mode)...");

    // Todos los buffers como input (migrar al device)
    let all_input_bufs = [
        buf_inputdata, buf_labeldata, buf_rndpoisson_hid,
        buf_hihjhi_ih, buf_chjhi_ih,
        buf_pj_ih, buf_pi_ih, buf_pji_ih, buf_bj_ih,
        buf_wji_ih, buf_wji_ih1, buf_wji_ih2,
        buf_pj_hu, buf_pi_hu, buf_pji_hu, buf_bj_hu, buf_wji_hu,
    ];

    // Buffers de salida (migrar de vuelta al host)
    let output_bufs = [
        buf_outputdata, buf_needsupdbw,
        buf_pj_ih, buf_pi_ih, buf_pji_ih, buf_bj_ih,
        buf_wji_ih,
        buf_pj_hu, buf_pi_hu, buf_pji_hu, buf_bj_hu, buf_wji_hu,
    ];

    let ret = unsafe {
        run(
            all_input_bufs.as_ptr(), all_input_bufs.len() as i32,
            output_bufs.as_ptr(), output_bufs.len() as i32,
        )
    };
    check(ret, "run");

    // --- 7. Leer resultado ---
    println!("[7] Reading output...");
    let mut output = vec![0.0f32; M_HID];
    read_floats(buf_outputdata, &mut output);

    println!("\n=== OUTPUT (primeros 10 de {} valores) ===", M_HID);
    for i in 0..10.min(M_HID) {
        println!("  output[{}] = {:.6}", i, output[i]);
    }

    // Buscar máximo (clasificación)
    let (max_idx, max_val) = output.iter().enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();
    println!("\nMax output: output[{}] = {:.6}", max_idx, max_val);
    println!("Predicted class (hypercolumn index): {}", max_idx / (M_HID / H_HID));

    // Verificar que al menos algo fue escrito por el kernel
    let nonzero = output.iter().filter(|&&x| x != 0.0).count();
    if nonzero > 0 {
        println!("\n El kernel FPGA escribió {} valores no-zero en el output!", nonzero);
    } else {
        println!("\n  Todos los outputs son cero — posiblemente pesos no cargados o kernel no ejecutó");
    }

    // --- 8. Cleanup ---
    println!("\n[8] Freeing all buffers...");
    let all_bufs = [
        buf_inputdata, buf_labeldata, buf_outputdata,
        buf_rndpoisson_hid, buf_hihjhi_ih, buf_chjhi_ih,
        buf_pj_ih, buf_pi_ih, buf_pji_ih, buf_bj_ih,
        buf_wji_ih, buf_wji_ih1, buf_wji_ih2,
        buf_pj_hu, buf_pi_hu, buf_pji_hu, buf_bj_hu, buf_wji_hu,
        buf_needsupdbw,
    ];
    for id in all_bufs {
        unsafe { free(id); }
    }
    println!("   Done — {} buffers freed", all_bufs.len());
    println!("\n=== BCPNN Inference Test Complete ===");
}

// ============================================================================
// HELPERS
// ============================================================================

fn check(ret: i32, op: &str) {
    if ret != 0 {
        println!("   {} FAILED (ret={})", op, ret);
        std::process::exit(1);
    }
}

fn alloc_checked(size: i32, name: &str) -> i32 {
    let id = unsafe { alloc(size) };
    if id < 0 {
        println!("   alloc({}, {} bytes) FAILED", name, size);
        std::process::exit(1);
    }
    id
}

fn write_floats(buf_id: i32, data: &[f32]) {
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4)
    };
    let ret = unsafe { write(buf_id, bytes.as_ptr(), bytes.len() as i32) };
    if ret != 0 {
        println!("   write(buf={}) FAILED", buf_id);
        std::process::exit(1);
    }
}

fn read_floats(buf_id: i32, data: &mut [f32]) {
    let bytes: &mut [u8] = unsafe {
        std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, data.len() * 4)
    };
    let ret = unsafe { read(buf_id, bytes.as_mut_ptr(), bytes.len() as i32) };
    if ret != 0 {
        println!("   read(buf={}) FAILED", buf_id);
        std::process::exit(1);
    }
}
