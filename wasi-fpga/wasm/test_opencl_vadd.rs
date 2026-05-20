//! Test del plugin WASI-FPGA con backend OpenCL
//!
//! Usa i32 (el kernel vadd usa volatile int*, NO float).
//! Incluye test de round-trip para verificar buffer I/O independiente del kernel.
//!
//! Compilar:
//!   rustc --target wasm32-wasip1 -o wasm/test_opencl_vadd.wasm wasm/test_opencl_vadd.rs
//!
//! Ejecutar:
//!   wasmedge test_opencl_vadd.wasm ./vadd.xclbin

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

const KERNEL_NAME: &str = "vadd";
const N: usize = 10;

fn main() {
    println!("=== WASI-FPGA OpenCL Backend Test (i32) ===");
    println!("Kernel: {}, N={}", KERNEL_NAME, N);

    let args: Vec<String> = std::env::args().collect();
    let xclbin_path = if args.len() > 1 {
        args[1].clone()
    } else {
        println!("Uso: wasmedge test_opencl_vadd.wasm <path_to_xclbin>");
        std::process::exit(1);
    };

    // =============================================
    // PASO 0: Cargar xclbin PRIMERO
    // =============================================
    // XRT edge necesita la topología de memoria del xclbin para saber
    // a qué banco (HP0, HPC0...) asignar buffers.  Sin xclbin cargado,
    // clCreateBuffer falla con "unknown memory index".
    println!("\n[0] Loading xclbin FIRST (required for buffer allocation): {}", xclbin_path);
    check(unsafe { load_xclbin(xclbin_path.as_ptr(), xclbin_path.len() as i32) }, "load_xclbin");

    // =============================================
    // FASE 1: ROUND-TRIP TEST (sin kernel, pero CON xclbin)
    // =============================================
    println!("\n========== FASE 1: ROUND-TRIP TEST ==========");
    println!("Escribir datos → buffer OpenCL → leer de vuelta (SIN kernel)");

    let test_pattern: Vec<i32> = vec![
        0x11111111_u32 as i32,
        0x22222222_u32 as i32,
        0x33333333_u32 as i32,
        0xDEADBEEF_u32 as i32,
    ];
    let test_bytes = i32_slice_as_bytes(&test_pattern);
    let test_size = test_bytes.len() as i32;

    println!("[RT-1] Allocating test buffer ({} bytes)...", test_size);
    let test_buf = unsafe { alloc(test_size) };
    assert!(test_buf > 0, "alloc failed");
    println!("  buf_id = {}", test_buf);

    println!("[RT-2] Writing test pattern: {:08X?}", test_pattern);
    let ret = unsafe { write(test_buf, test_bytes.as_ptr(), test_size) };
    assert!(ret == 0, "write failed");

    println!("[RT-3] Reading back...");
    let mut readback = vec![0i32; test_pattern.len()];
    let readback_bytes = i32_slice_as_bytes_mut(&mut readback);
    let ret = unsafe { read(test_buf, readback_bytes.as_mut_ptr(), test_size) };
    assert!(ret == 0, "read failed");

    println!("  Wrote:    {:08X?}", test_pattern);
    println!("  Read back: {:08X?}", readback);

    if test_pattern == readback {
        println!("   ROUND-TRIP OK — buffer I/O funciona correctamente");
    } else {
        println!("   ROUND-TRIP FAILED — buffer I/O NO funciona");
        println!("  → Esto significa que clEnqueueWriteBuffer/ReadBuffer tiene problemas");
        println!("  → No tiene sentido probar el kernel si el I/O falla");
    }

    unsafe { free(test_buf); }

    // =============================================
    // FASE 2: KERNEL TEST (con i32)
    // =============================================
    println!("\n========== FASE 2: KERNEL TEST (vadd i32) ==========");

    // --- Crear kernel (xclbin ya cargado arriba) ---
    println!("[2] Creating kernel: {}", KERNEL_NAME);
    check(unsafe { create_kernel(KERNEL_NAME.as_ptr(), KERNEL_NAME.len() as i32) }, "create_kernel");

    // --- Preparar datos (i32, NO float) ---
    let buf_size = (N * 4) as i32;

    // A = [1, 2, 3, ..., 10]
    let a: Vec<i32> = (1..=N as i32).collect();
    // B = [10, 20, 30, ..., 100]
    let b: Vec<i32> = (1..=N as i32).map(|x| x * 10).collect();
    // C = [0xDEADBEEF, ...] — patrón canario para detectar si el kernel escribe
    let mut c: Vec<i32> = vec![0xDEADBEEFu32 as i32; N];

    println!("[3] Input data (i32):");
    println!("  A = {:?}", a);
    println!("  B = {:?}", b);
    println!("  C (canary) = 0x{:08X} x {}", c[0] as u32, N);

    // --- Alocar buffers ---
    println!("[4] Allocating 3 buffers ({} bytes each)...", buf_size);
    let buf_a = unsafe { alloc(buf_size) };
    let buf_b = unsafe { alloc(buf_size) };
    let buf_c = unsafe { alloc(buf_size) };
    assert!(buf_a > 0 && buf_b > 0 && buf_c > 0, "alloc failed");
    println!("   buf_a={}, buf_b={}, buf_c={}", buf_a, buf_b, buf_c);

    // --- Escribir datos ---
    println!("[5] Writing input data...");
    check(unsafe { write(buf_a, i32_slice_as_bytes(&a).as_ptr(), buf_size) }, "write A");
    check(unsafe { write(buf_b, i32_slice_as_bytes(&b).as_ptr(), buf_size) }, "write B");
    check(unsafe { write(buf_c, i32_slice_as_bytes(&c).as_ptr(), buf_size) }, "write C (canary)");

    // --- Verificar round-trip de A antes del kernel ---
    println!("[5b] Verifying A round-trip before kernel...");
    let mut a_check = vec![0i32; N];
    check(unsafe { read(buf_a, i32_slice_as_bytes_mut(&mut a_check).as_mut_ptr(), buf_size) }, "read A check");
    if a == a_check {
        println!("   A round-trip OK: {:?}", a_check);
    } else {
        println!("   A round-trip FAILED!");
        println!("    Wrote: {:?}", a);
        println!("    Read:  {:?}", a_check);
    }

    // --- Verificar round-trip de C (canary) antes del kernel ---
    println!("[5c] Verifying C round-trip before kernel...");
    let mut c_check = vec![0i32; N];
    check(unsafe { read(buf_c, i32_slice_as_bytes_mut(&mut c_check).as_mut_ptr(), buf_size) }, "read C check");
    if c == c_check {
        println!("   C canary round-trip OK: 0x{:08X}", c_check[0] as u32);
    } else {
        println!("   C canary round-trip FAILED!");
        println!("    Wrote: {:08X?}", c.iter().map(|x| *x as u32).collect::<Vec<_>>());
        println!("    Read:  {:08X?}", c_check.iter().map(|x| *x as u32).collect::<Vec<_>>());
    }

    // --- Set kernel arguments ---
    println!("[6] Setting kernel arguments...");
    check(unsafe { set_arg(0, buf_a) }, "set_arg(0, A)");
    check(unsafe { set_arg(1, buf_b) }, "set_arg(1, B)");
    check(unsafe { set_arg(2, buf_c) }, "set_arg(2, C)");
    check(unsafe { set_arg_int(3, N as i32) }, "set_arg_int(3, N)");
    println!("   OK");

    // --- Ejecutar kernel ---
    // Migrar TODOS al device, leer C de vuelta
    let all_bufs = [buf_a, buf_b, buf_c];
    let output_ids = [buf_c];

    println!("[7] Running kernel (migrate ALL → execute → migrate C back)...");
    let ret = unsafe {
        run(
            all_bufs.as_ptr(), all_bufs.len() as i32,
            output_ids.as_ptr(), output_ids.len() as i32,
        )
    };
    check(ret, "run");

    // --- Leer resultado ---
    println!("[8] Reading output buffer C...");
    let mut result = vec![0i32; N];
    check(unsafe { read(buf_c, i32_slice_as_bytes_mut(&mut result).as_mut_ptr(), buf_size) }, "read C");

    // --- Verificar ---
    println!("\n=== RESULTADO ===");
    let expected: Vec<i32> = a.iter().zip(b.iter()).map(|(x, y)| x + y).collect();
    println!("  A        = {:?}", a);
    println!("  B        = {:?}", b);
    println!("  C result = {:?}", result);
    println!("  Expected = {:?}", expected);
    println!("  C hex    = {:08X?}", result.iter().map(|x| *x as u32).collect::<Vec<_>>());

    if result == expected {
        println!("\n   TEST PASSED! kernel vadd calculó A + B correctamente ");
    } else if result.iter().all(|&x| x == 0xDEADBEEFu32 as i32) {
        println!("\n   CANARY UNCHANGED — el kernel NO escribió en memoria");
        println!("  → M_AXI write no funciona (plataforma base sin PAC)");
        println!("  → Necesitas instalar un PAC o recompilar el xclbin con plataforma custom");
    } else if result.iter().all(|&x| x == 0) {
        println!("\n   ALL ZEROS — kernel o migrate borraron el canary pero no escribieron resultado");
    } else {
        println!("\n   VALORES INESPERADOS:");
        for i in 0..N {
            println!("    result[{}] = {} (0x{:08X})  expected: {}", i, result[i], result[i] as u32, expected[i]);
        }
    }

    // --- Cleanup ---
    println!("\n[9] Freeing buffers...");
    unsafe { free(buf_a); free(buf_b); free(buf_c); }
    println!("   Done");
    println!("\n=== Test Complete ===");
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

fn i32_slice_as_bytes(data: &[i32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 4) }
}

fn i32_slice_as_bytes_mut(data: &mut [i32]) -> &mut [u8] {
    unsafe { std::slice::from_raw_parts_mut(data.as_mut_ptr() as *mut u8, data.len() * 4) }
}
