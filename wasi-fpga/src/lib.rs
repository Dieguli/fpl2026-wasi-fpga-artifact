//! WASI FPGA Plugin para WasmEdge — Backend OpenCL
//!
//! Plugin nativo que expone operaciones FPGA (Xilinx ZCU104) a módulos WASM
//! a través del módulo de imports "fpga".
//!
//! Backend OpenCL (via XRT) compatible con los PAC de las demos BCPNN.
//!
//! Funciones WASI exportadas (módulo "fpga"):
//!   load_xclbin(path_ptr, path_len) -> i32
//!   create_kernel(name_ptr, name_len) -> i32
//!   alloc(size) -> buf_id
//!   write(buf_id, data_ptr, data_len) -> i32
//!   read(buf_id, data_ptr, data_len) -> i32
//!   set_arg(arg_idx, buf_id) -> i32
//!   set_arg_int(arg_idx, value) -> i32
//!   run(in_ids_ptr, in_ids_len, out_ids_ptr, out_ids_len) -> i32
//!   free(buf_id) -> i32

use std::sync::{Arc, Mutex};
use std::os::raw::{c_char, c_void};
use std::time::Instant;

mod opencl;
mod fpga_state;
mod error;

// Módulos legacy (XRT directo) — archivos preservados en el repo pero no compilados
// mod xrt;
// mod buffer_manager;

use fpga_state::{FpgaState, bench_enabled};

// ============================================================================
// GLOBAL STATE
// ============================================================================

lazy_static::lazy_static! {
    static ref FPGA_STATE: Arc<Mutex<Option<FpgaState>>> = Arc::new(Mutex::new(None));
}

// ============================================================================
// PLUGIN ENTRY POINT
// ============================================================================

#[used]
#[link_section = ".init_array"]
static INIT_ARRAY: extern "C" fn() = {
    extern "C" fn init() {
        eprintln!("[wasi_fpga] ====== LIBRARY LOADED (OpenCL backend) ======");
    }
    init
};

static PLUGIN_NAME: &[u8] = b"wasi_fpga\0";
static PLUGIN_DESC: &[u8] = b"WASI FPGA Extension - OpenCL backend for ZCU104\0";
static MODULE_NAME: &[u8] = b"fpga\0";

#[repr(C)]
pub struct WasmEdge_ModuleDescriptor {
    pub name: *const c_char,
    pub description: *const c_char,
    pub create: Option<unsafe extern "C" fn(*const WasmEdge_ModuleDescriptor) -> *mut WasmEdge_ModuleInstanceContext>,
}

unsafe impl Sync for WasmEdge_ModuleDescriptor {}

#[repr(C)]
pub struct WasmEdge_PluginDescriptor {
    pub name: *const c_char,
    pub description: *const c_char,
    pub api_version: u32,
    pub version: WasmEdge_PluginVersionData,
    pub module_count: u32,
    pub program_option_count: u32,
    pub module_descriptions: *mut WasmEdge_ModuleDescriptor,
    pub program_options: *mut c_void,
}

#[repr(C)]
pub struct WasmEdge_PluginVersionData {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub build: u32,
}

unsafe impl Sync for WasmEdge_PluginDescriptor {}

static mut MODULE_DESCRIPTORS: [WasmEdge_ModuleDescriptor; 1] = [
    WasmEdge_ModuleDescriptor {
        name: MODULE_NAME.as_ptr() as *const c_char,
        description: PLUGIN_DESC.as_ptr() as *const c_char,
        create: Some(create_fpga_module_instance),
    }
];

static mut PLUGIN_DESCRIPTOR: WasmEdge_PluginDescriptor = WasmEdge_PluginDescriptor {
    name: PLUGIN_NAME.as_ptr() as *const c_char,
    description: PLUGIN_DESC.as_ptr() as *const c_char,
    api_version: 4,
    version: WasmEdge_PluginVersionData { major: 0, minor: 2, patch: 0, build: 0 },
    module_count: 1,
    program_option_count: 0,
    module_descriptions: std::ptr::null_mut(),
    program_options: std::ptr::null_mut(),
};

#[no_mangle]
pub unsafe extern "C" fn WasmEdge_Plugin_GetDescriptor() -> *const WasmEdge_PluginDescriptor {
    PLUGIN_DESCRIPTOR.module_descriptions = MODULE_DESCRIPTORS.as_mut_ptr();
    &raw const PLUGIN_DESCRIPTOR
}

// ============================================================================
// WASMEDGE C API IMPORTS & TYPES
// ============================================================================

#[repr(C)] pub struct WasmEdge_ModuleInstanceContext { _opaque: [u8; 0] }
#[repr(C)] pub struct WasmEdge_FunctionInstanceContext { _opaque: [u8; 0] }
#[repr(C)] pub struct WasmEdge_FunctionTypeContext { _opaque: [u8; 0] }
#[repr(C)] pub struct WasmEdge_CallingFrameContext { _opaque: [u8; 0] }
#[repr(C)] pub struct WasmEdge_MemoryInstanceContext { _opaque: [u8; 0] }

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WasmEdge_Value {
    pub value: u128,
    pub val_type: WasmEdge_ValType,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WasmEdge_ValType {
    pub data: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WasmEdge_String {
    pub length: u32,
    pub buf: *const c_char,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WasmEdge_Result {
    pub code: u32,
}

extern "C" {
    fn WasmEdge_ModuleInstanceCreate(name: WasmEdge_String) -> *mut WasmEdge_ModuleInstanceContext;
    fn WasmEdge_ModuleInstanceAddFunction(cxt: *mut WasmEdge_ModuleInstanceContext, name: WasmEdge_String, func_cxt: *mut WasmEdge_FunctionInstanceContext);
    fn WasmEdge_FunctionTypeCreate(p_list: *const WasmEdge_ValType, p_len: u32, r_list: *const WasmEdge_ValType, r_len: u32) -> *mut WasmEdge_FunctionTypeContext;
    fn WasmEdge_FunctionTypeDelete(cxt: *mut WasmEdge_FunctionTypeContext);
    fn WasmEdge_FunctionInstanceCreate(ft: *const WasmEdge_FunctionTypeContext, func: HostFuncType, data: *mut c_void, cost: u64) -> *mut WasmEdge_FunctionInstanceContext;
    fn WasmEdge_ValTypeGenI32() -> WasmEdge_ValType;
    fn WasmEdge_ValueGenI32(val: i32) -> WasmEdge_Value;
    fn WasmEdge_ValueGetI32(val: WasmEdge_Value) -> i32;
    fn WasmEdge_StringWrap(buf: *const c_char, len: u32) -> WasmEdge_String;
    fn WasmEdge_CallingFrameGetMemoryInstance(cxt: *const WasmEdge_CallingFrameContext, idx: u32) -> *mut WasmEdge_MemoryInstanceContext;
    fn WasmEdge_MemoryInstanceGetData(cxt: *const WasmEdge_MemoryInstanceContext, data: *mut u8, off: u32, len: u32) -> WasmEdge_Result;
    fn WasmEdge_MemoryInstanceSetData(cxt: *mut WasmEdge_MemoryInstanceContext, data: *const u8, off: u32, len: u32) -> WasmEdge_Result;
}

type HostFuncType = unsafe extern "C" fn(*mut c_void, *const WasmEdge_CallingFrameContext, *const WasmEdge_Value, *mut WasmEdge_Value) -> WasmEdge_Result;

// ============================================================================
// HELPER FUNCTIONS
// ============================================================================

unsafe fn ensure_initialized() -> bool {
    let mut lock = FPGA_STATE.lock().unwrap();
    if lock.is_none() {
        eprintln!("[wasi_fpga] Auto-initializing FpgaState (OpenCL, device 0)...");
        match FpgaState::new(0) {
            Ok(state) => {
                *lock = Some(state);
                true
            }
            Err(e) => {
                eprintln!("[wasi_fpga] ERROR initializing: {:?}", e);
                false
            }
        }
    } else {
        true
    }
}

/// Leer un string UTF-8 desde la memoria lineal del WASM
unsafe fn read_wasm_string(frame: *const WasmEdge_CallingFrameContext, ptr: u32, len: u32) -> Option<String> {
    let mem = WasmEdge_CallingFrameGetMemoryInstance(frame, 0);
    if mem.is_null() { return None; }
    let mut buf = vec![0u8; len as usize];
    WasmEdge_MemoryInstanceGetData(mem, buf.as_mut_ptr(), ptr, len);
    String::from_utf8(buf).ok()
}

/// Leer un array de i32 (little-endian) desde la memoria lineal del WASM
unsafe fn read_wasm_i32_array(frame: *const WasmEdge_CallingFrameContext, ptr: u32, count: u32) -> Option<Vec<i32>> {
    let mem = WasmEdge_CallingFrameGetMemoryInstance(frame, 0);
    if mem.is_null() { return None; }
    let byte_len = count * 4;
    let mut raw = vec![0u8; byte_len as usize];
    WasmEdge_MemoryInstanceGetData(mem, raw.as_mut_ptr(), ptr, byte_len);
    Some(raw.chunks_exact(4).map(|b| i32::from_le_bytes(b.try_into().unwrap())).collect())
}

const RESULT_OK: WasmEdge_Result = WasmEdge_Result { code: 0 };

// ============================================================================
// HOST FUNCTIONS
// ============================================================================

/// fpga.load_xclbin(path_ptr: i32, path_len: i32) -> i32
unsafe extern "C" fn c_load_xclbin(_d: *mut c_void, frame: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let path_ptr = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let path_len = WasmEdge_ValueGetI32(*p.offset(1)) as u32;

    let path = match read_wasm_string(frame, path_ptr, path_len) {
        Some(s) => s,
        None => { *r = WasmEdge_ValueGenI32(-1); return RESULT_OK; }
    };
    eprintln!("[wasi_fpga] load_xclbin('{}')", path);

    if !ensure_initialized() {
        *r = WasmEdge_ValueGenI32(-1);
        return RESULT_OK;
    }

    let mut lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        match state.load_xclbin(&path) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(e) => {
                eprintln!("[wasi_fpga] load_xclbin error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=load_xclbin wall_ns={}", t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.create_kernel(name_ptr: i32, name_len: i32) -> i32
unsafe extern "C" fn c_create_kernel(_d: *mut c_void, frame: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let name_ptr = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let name_len = WasmEdge_ValueGetI32(*p.offset(1)) as u32;

    let name = match read_wasm_string(frame, name_ptr, name_len) {
        Some(s) => s,
        None => { *r = WasmEdge_ValueGenI32(-1); return RESULT_OK; }
    };
    eprintln!("[wasi_fpga] create_kernel('{}')", name);

    let mut lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        match state.create_kernel(&name) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(e) => {
                eprintln!("[wasi_fpga] create_kernel error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=create_kernel wall_ns={}", t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.alloc(size: i32) -> buf_id: i32
unsafe extern "C" fn c_alloc(_d: *mut c_void, _f: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    if !ensure_initialized() {
        *r = WasmEdge_ValueGenI32(-1);
        return RESULT_OK;
    }

    let size = WasmEdge_ValueGetI32(*p) as usize;
    let mut lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        match state.alloc(size) {
            Ok(id) => *r = WasmEdge_ValueGenI32(id),
            Err(e) => {
                eprintln!("[wasi_fpga] alloc error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=alloc size={} wall_ns={}", size, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.write(buf_id: i32, data_ptr: i32, data_len: i32) -> i32
unsafe extern "C" fn c_write(_d: *mut c_void, frame: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let id = WasmEdge_ValueGetI32(*p.offset(0));
    let wasm_ptr = WasmEdge_ValueGetI32(*p.offset(1)) as u32;
    let len = WasmEdge_ValueGetI32(*p.offset(2)) as u32;

    eprintln!("[wasi_fpga] write(buf={}, wasm_ptr=0x{:X}, len={})", id, wasm_ptr, len);

    let mem_ctx = WasmEdge_CallingFrameGetMemoryInstance(frame, 0);
    if mem_ctx.is_null() {
        eprintln!("[wasi_fpga] write: ERROR - WASM memory is null!");
        *r = WasmEdge_ValueGenI32(-1);
        return RESULT_OK;
    }
    let mut data = vec![0u8; len as usize];
    let get_result = WasmEdge_MemoryInstanceGetData(mem_ctx, data.as_mut_ptr(), wasm_ptr, len);
    eprintln!("[wasi_fpga] write: GetData result code={}", get_result.code);

    // Diagnóstico: imprimir primeros bytes leídos del WASM
    let preview_len = data.len().min(40);
    let hex: Vec<String> = data[..preview_len].iter().map(|b| format!("{:02X}", b)).collect();
    eprintln!("[wasi_fpga] write: WASM data first {} bytes: {}", preview_len, hex.join(" "));

    let mut lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        match state.write(id, &data) {
            Ok(_) => {
                eprintln!("[wasi_fpga] write: clEnqueueWriteBuffer OK");
                *r = WasmEdge_ValueGenI32(0);
            }
            Err(e) => {
                eprintln!("[wasi_fpga] write error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=write buf_id={} bytes={} wall_ns={}", id, len, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.read(buf_id: i32, data_ptr: i32, data_len: i32) -> i32
unsafe extern "C" fn c_read(_d: *mut c_void, frame: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let id = WasmEdge_ValueGetI32(*p.offset(0));
    let wasm_ptr = WasmEdge_ValueGetI32(*p.offset(1)) as u32;
    let len = WasmEdge_ValueGetI32(*p.offset(2)) as u32;

    eprintln!("[wasi_fpga] read(buf={}, wasm_ptr=0x{:X}, len={})", id, wasm_ptr, len);

    let lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_ref() {
        match state.read(id, len as usize) {
            Ok(data) => {
                // Diagnóstico: imprimir primeros bytes que devuelve OpenCL
                let preview_len = data.len().min(40);
                let hex: Vec<String> = data[..preview_len].iter().map(|b| format!("{:02X}", b)).collect();
                eprintln!("[wasi_fpga] read: OpenCL returned {} bytes, first {}: {}",
                    data.len(), preview_len, hex.join(" "));

                // Interpretar como i32s para diagnóstico
                if data.len() >= 4 {
                    let n_ints = (data.len() / 4).min(10);
                    let ints: Vec<i32> = data.chunks_exact(4).take(n_ints)
                        .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                        .collect();
                    eprintln!("[wasi_fpga] read: as i32s: {:?}", ints);
                }

                let mem = WasmEdge_CallingFrameGetMemoryInstance(frame, 0);
                if mem.is_null() {
                    eprintln!("[wasi_fpga] read: ERROR - WASM memory is null!");
                    *r = WasmEdge_ValueGenI32(-1);
                } else {
                    let set_result = WasmEdge_MemoryInstanceSetData(mem, data.as_ptr(), wasm_ptr, data.len() as u32);
                    eprintln!("[wasi_fpga] read: SetData result code={}", set_result.code);
                    *r = WasmEdge_ValueGenI32(0);
                }
            }
            Err(e) => {
                eprintln!("[wasi_fpga] read error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=read buf_id={} bytes={} wall_ns={}", id, len, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.set_arg(arg_idx: i32, buf_id: i32) -> i32
unsafe extern "C" fn c_set_arg(_d: *mut c_void, _f: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let arg_idx = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let buf_id = WasmEdge_ValueGetI32(*p.offset(1));

    let lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_ref() {
        match state.set_arg_buffer(arg_idx, buf_id) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(e) => {
                eprintln!("[wasi_fpga] set_arg error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=set_arg arg_idx={} buf_id={} wall_ns={}", arg_idx, buf_id, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.set_arg_int(arg_idx: i32, value: i32) -> i32
unsafe extern "C" fn c_set_arg_int(_d: *mut c_void, _f: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let arg_idx = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let value = WasmEdge_ValueGetI32(*p.offset(1));

    let lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_ref() {
        match state.set_arg_int(arg_idx, value) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(e) => {
                eprintln!("[wasi_fpga] set_arg_int error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=set_arg_int arg_idx={} value={} wall_ns={}", arg_idx, value, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.set_arg_float(arg_idx: i32, value_bits: i32) -> i32
///
/// El valor float se pasa como i32 con los mismos bits (reinterpret cast).
/// Desde WASM: `set_arg_float(idx, f32::to_bits(val) as i32)`
unsafe extern "C" fn c_set_arg_float(_d: *mut c_void, _f: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let arg_idx = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let bits = WasmEdge_ValueGetI32(*p.offset(1)) as u32;
    let value = f32::from_bits(bits);

    let lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_ref() {
        match state.set_arg_float(arg_idx, value) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(e) => {
                eprintln!("[wasi_fpga] set_arg_float error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=set_arg_float arg_idx={} value={} wall_ns={}", arg_idx, value, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

/// fpga.run(in_ids_ptr: i32, in_ids_len: i32, out_ids_ptr: i32, out_ids_len: i32) -> i32
///
/// Ejecuta el kernel con el patrón completo BCPNN:
///   1. Migrar buffers de entrada al dispositivo
///   2. clEnqueueTask
///   3. Migrar buffers de salida al host
///   4. clFinish
unsafe extern "C" fn c_run(_d: *mut c_void, frame: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let in_ptr = WasmEdge_ValueGetI32(*p.offset(0)) as u32;
    let in_len = WasmEdge_ValueGetI32(*p.offset(1)) as u32;
    let out_ptr = WasmEdge_ValueGetI32(*p.offset(2)) as u32;
    let out_len = WasmEdge_ValueGetI32(*p.offset(3)) as u32;

    let input_ids = read_wasm_i32_array(frame, in_ptr, in_len).unwrap_or_default();
    let output_ids = read_wasm_i32_array(frame, out_ptr, out_len).unwrap_or_default();

    eprintln!("[wasi_fpga] run(inputs={:?}, outputs={:?})", input_ids, output_ids);

    let lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_ref() {
        match state.run_kernel(&input_ids, &output_ids) {
            Ok(timings) => {
                if bench_enabled() {
                    eprintln!(
                        "[BENCH] op=run migrate_in_ns={} kernel_ns={} migrate_out_ns={} total_wall_ns={}",
                        timings.migrate_in_ns.map_or("N/A".to_string(), |v| v.to_string()),
                        timings.kernel_ns.map_or("N/A".to_string(), |v| v.to_string()),
                        timings.migrate_out_ns.map_or("N/A".to_string(), |v| v.to_string()),
                        timings.total_wall_ns,
                    );
                }
                *r = WasmEdge_ValueGenI32(0);
            }
            Err(e) => {
                eprintln!("[wasi_fpga] run error: {:?}", e);
                *r = WasmEdge_ValueGenI32(-1);
            }
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    RESULT_OK
}

/// fpga.free(buf_id: i32) -> i32
unsafe extern "C" fn c_free(_d: *mut c_void, _f: *const WasmEdge_CallingFrameContext, p: *const WasmEdge_Value, r: *mut WasmEdge_Value) -> WasmEdge_Result {
    let t0 = if bench_enabled() { Some(Instant::now()) } else { None };
    let id = WasmEdge_ValueGetI32(*p);
    let mut lock = FPGA_STATE.lock().unwrap();
    if let Some(state) = lock.as_mut() {
        match state.free(id) {
            Ok(_) => *r = WasmEdge_ValueGenI32(0),
            Err(_) => *r = WasmEdge_ValueGenI32(-1),
        }
    } else {
        *r = WasmEdge_ValueGenI32(-1);
    }
    if let Some(t0) = t0 {
        eprintln!("[BENCH] op=free buf_id={} wall_ns={}", id, t0.elapsed().as_nanos());
    }
    RESULT_OK
}

// ============================================================================
// MODULE CREATION
// ============================================================================

unsafe extern "C" fn create_fpga_module_instance(name: *const WasmEdge_ModuleDescriptor) -> *mut WasmEdge_ModuleInstanceContext {
    let name_str = WasmEdge_StringWrap((*name).name, 4);
    let module = WasmEdge_ModuleInstanceCreate(name_str);
    
    if module.is_null() { return std::ptr::null_mut(); }

    eprintln!("[wasi_fpga] Creating module instance (OpenCL backend v0.2)");

    let add = |n: &[u8], f: HostFuncType, pc: u32| {
        let func_name = WasmEdge_StringWrap(n.as_ptr() as *const c_char, (n.len()-1) as u32);
        let i32_t = WasmEdge_ValTypeGenI32();
        let params: Vec<WasmEdge_ValType> = (0..pc).map(|_| i32_t).collect();
        let returns = [i32_t];
        
        let ft = WasmEdge_FunctionTypeCreate(
            if pc > 0 { params.as_ptr() } else { std::ptr::null() }, pc,
            returns.as_ptr(), 1
        );
        let fi = WasmEdge_FunctionInstanceCreate(ft, f, std::ptr::null_mut(), 0);
        WasmEdge_FunctionTypeDelete(ft);
        WasmEdge_ModuleInstanceAddFunction(module, func_name, fi);
    };

    // --- Initialization ---
    add(b"load_xclbin\0",   c_load_xclbin,   2);  // (path_ptr, path_len) -> i32
    add(b"create_kernel\0", c_create_kernel,  2);  // (name_ptr, name_len) -> i32

    // --- Buffer management ---
    add(b"alloc\0",         c_alloc,          1);  // (size) -> buf_id
    add(b"write\0",         c_write,          3);  // (buf_id, data_ptr, data_len) -> i32
    add(b"read\0",          c_read,           3);  // (buf_id, data_ptr, data_len) -> i32
    add(b"free\0",          c_free,           1);  // (buf_id) -> i32

    // --- Kernel arguments ---
    add(b"set_arg\0",       c_set_arg,        2);  // (arg_idx, buf_id) -> i32
    add(b"set_arg_int\0",   c_set_arg_int,    2);  // (arg_idx, value) -> i32
    add(b"set_arg_float\0", c_set_arg_float,  2);  // (arg_idx, value_bits) -> i32

    // --- Execution ---
    add(b"run\0",           c_run,            4);  // (in_ptr, in_len, out_ptr, out_len) -> i32

    eprintln!("[wasi_fpga]  10 host functions registered");
    module
}