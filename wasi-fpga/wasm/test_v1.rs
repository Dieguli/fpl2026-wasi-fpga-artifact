// test_v1.rs
use std::mem;
use std::env;

// Definimos las funciones que el PLUGIN (el .so) nos ofrece
#[link(wasm_import_module = "fpga")]
extern "C" {
    // AÑADIDO: Función para cargar el bitstream
    fn load_xclbin(name_ptr: *const u8, name_len: u32) -> i32;
    
    fn alloc(size: u32) -> i32;
    fn write(id: i32, data_ptr: *const u8, data_len: u32) -> i32;
    fn execute(name_ptr: *const u8, name_len: u32, ids_ptr: *const i32, ids_len: u32) -> i32;
    fn read(id: i32, data_ptr: *mut u8, data_len: u32) -> i32;
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    // args[1] sería el primer parámetro que pasamos (el xclbin)
    let xclbin_name = if args.len() > 1 {
        &args[1]
    } else {
        "vadd.xclbin" // Valor por defecto
    };

    println!("[WASM] Usando bitstream: {}", xclbin_name);

    // 0. CARGAR EL BITSTREAM (El paso que faltaba)
    println!("[WASM] Solicitando carga de bitstream a la FPGA...");
    let res_load = unsafe {
        load_xclbin(
            xclbin_name.as_ptr(),
            xclbin_name.len() as u32
        )
    };

    if res_load != 0 {
        println!(" [WASM] Error crítico: No se pudo cargar el bitstream {}", xclbin_name);
        return; // Salimos si no hay hardware
    }
    println!(" [WASM] Bitstream cargado correctamente.");

    println!("--- [WASM] Iniciando Test de Suma de Vectores ---");

    let size: u32 = 10;
    let byte_size = size * 4; // 10 enteros de 4 bytes cada uno

    // 1. Reservar buffers en la FPGA
    let id_a = unsafe { alloc(byte_size) };
    let id_b = unsafe { alloc(byte_size) };
    let id_res = unsafe { alloc(byte_size) };
    println!("[WASM] Buffers creados. IDs: A={}, B={}, RES={}", id_a, id_b, id_res);

    // 2. Preparar datos (10 unidades de '1' y 10 unidades de '10')
    // Usamos i32 explícitamente para coincidir con el kernel C++
    let data_a: Vec<i32> = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let data_b: Vec<i32> = vec![10, 10, 10, 10, 10, 10, 10, 10, 10, 10];

    // Convertir a bytes para enviarlos
    let bytes_a: &[u8] = unsafe { 
        std::slice::from_raw_parts(data_a.as_ptr() as *const u8, byte_size as usize) 
    };
    let bytes_b: &[u8] = unsafe { 
        std::slice::from_raw_parts(data_b.as_ptr() as *const u8, byte_size as usize) 
    };

    unsafe {
        write(id_a, bytes_a.as_ptr(), byte_size);
        write(id_b, bytes_b.as_ptr(), byte_size);
    }
    println!("[WASM] Datos escritos en buffers A y B.");

    // 3. Ejecutar el Kernel
    let kernel_name = "vadd";
    let buffer_ids = vec![id_a, id_b, id_res];

    println!("[WASM] Llamando a execute_kernel('{}')...", kernel_name);
    unsafe {
        execute(
            kernel_name.as_ptr(),
            kernel_name.len() as u32,
            buffer_ids.as_ptr(),
            buffer_ids.len() as u32,
        );
    }

    // 4. Leer el resultado
    let mut result_data: Vec<i32> = vec![0; size as usize];
    let result_bytes = unsafe {
        std::slice::from_raw_parts_mut(result_data.as_mut_ptr() as *mut u8, byte_size as usize)
    };

    unsafe {
        read(id_res, result_bytes.as_mut_ptr(), byte_size);
    }

    println!("[WASM] Resultado final del primer índice: {}", result_data[0]);
    if result_data[0] == 11 {
        println!(" TEST EXITOSO: 1 + 10 = 11");
    } else {
        println!(" FALLO: Se esperaba 11, se obtuvo {}", result_data[0]);
    }
}