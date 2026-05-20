//! FpgaState — Gestor de estado FPGA basado en OpenCL
//!
//! Reemplaza el buffer_manager anterior que usaba xclRegWrite/xclRegRead.
//! Este usa la API OpenCL completa (contexto, queue, kernel, buffers),
//! que es lo que las demos BCPNN usan exitosamente con los PAC.
//!
//! Flujo BCPNN (referencia):
//!   1. clCreateContext + clCreateCommandQueue
//!   2. clCreateProgramWithBinary (xclbin)
//!   3. clCreateKernel("BCPNN_Kernel")
//!   4. clCreateBuffer (21 buffers)
//!   5. clSetKernelArg (21 argumentos)
//!   6. clEnqueueMigrateMemObjects → device (inputs)
//!   7. clEnqueueTask (ejecutar kernel)
//!   8. clEnqueueMigrateMemObjects → host (outputs)
//!   9. clFinish

use crate::opencl::*;
use crate::error::{WasiFpgaError, Result};
use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::collections::HashMap;
use std::ffi::CString;
use std::os::raw::c_void;
use std::ptr;
use std::time::Instant;

// ============================================================================
// PAGE-ALIGNED HOST MEMORY
// ============================================================================

/// Page-aligned host memory for CL_MEM_USE_HOST_PTR buffers.
///
/// XRT edge requires 4096-byte alignment for DMA host pointers.
/// When an unaligned pointer is passed, XRT silently creates an
/// internal copy and the kernel output never reaches the original
/// host memory — causing read() to return stale zeros.
pub struct AlignedBuffer {
    ptr: *mut u8,
    size: usize,
    layout: Layout,
}

impl AlignedBuffer {
    fn new(size: usize) -> Result<Self> {
        const PAGE_SIZE: usize = 4096;
        let alloc_size = if size == 0 { PAGE_SIZE } else { size };
        let layout = Layout::from_size_align(alloc_size, PAGE_SIZE)
            .map_err(|_| WasiFpgaError::XrtError(
                format!("Invalid aligned layout: size={}", size)
            ))?;
        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err(WasiFpgaError::XrtError(
                format!("Page-aligned alloc failed: {} bytes", alloc_size)
            ));
        }
        Ok(Self { ptr, size: alloc_size, layout })
    }

    fn as_mut_ptr(&mut self) -> *mut u8 { self.ptr }

    fn as_slice(&self, len: usize) -> &[u8] {
        let n = len.min(self.size);
        unsafe { std::slice::from_raw_parts(self.ptr, n) }
    }

    fn copy_from_slice(&mut self, data: &[u8]) {
        let len = data.len().min(self.size);
        unsafe { std::ptr::copy_nonoverlapping(data.as_ptr(), self.ptr, len); }
    }
}

impl Drop for AlignedBuffer {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe { dealloc(self.ptr, self.layout); }
            self.ptr = ptr::null_mut();
        }
    }
}

// SAFETY: The pointed-to memory is exclusively owned by AlignedBuffer.
unsafe impl Send for AlignedBuffer {}
unsafe impl Sync for AlignedBuffer {}

// ============================================================================
// BUFFER WRAPPER
// ============================================================================

/// Buffer OpenCL con host memory alineado a pagina.
///
/// `host_data` mantiene la memoria alineada cuyo puntero se pasó a
/// clCreateBuffer con CL_MEM_USE_HOST_PTR. La alineación a 4096 bytes
/// garantiza que XRT use el puntero directamente sin copias internas.
pub struct OclBuffer {
    pub mem: cl_mem,
    pub size: usize,
    host_data: AlignedBuffer,
}

impl OclBuffer {
    pub fn is_null(&self) -> bool {
        self.mem.is_null()
    }
}

// ============================================================================
// BENCHMARK SUPPORT
// ============================================================================

/// Timing data from a single `run_kernel` invocation.
/// OpenCL fields are populated only when `WASI_FPGA_BENCH=1`.
#[derive(Debug, Default)]
pub struct RunTimings {
    pub migrate_in_ns: Option<u64>,
    pub kernel_ns: Option<u64>,
    pub migrate_out_ns: Option<u64>,
    pub total_wall_ns: u64,
}

/// Returns true if benchmarking is enabled via `WASI_FPGA_BENCH=1`.
pub fn bench_enabled() -> bool {
    std::env::var("WASI_FPGA_BENCH").map(|v| v == "1").unwrap_or(false)
}

// ============================================================================
// FPGA STATE
// ============================================================================

pub struct FpgaState {
    // OpenCL core objects
    context: cl_context,
    queue: cl_command_queue,
    device: cl_device_id,

    // Program (loaded xclbin)
    program: Option<cl_program>,

    // Active kernel
    kernel: Option<cl_kernel>,
    kernel_name: Option<String>,

    // Buffer management
    buffers: HashMap<i32, OclBuffer>,
    next_id: i32,
}

unsafe impl Send for FpgaState {}
unsafe impl Sync for FpgaState {}

impl FpgaState {
    // ========================================================================
    // INITIALIZATION
    // ========================================================================

    /// Inicializa OpenCL: busca plataforma Xilinx, abre dispositivo FPGA
    pub fn new(device_index: u32) -> Result<Self> {
        unsafe {
            eprintln!("[fpga_ocl] Initializing OpenCL...");

            // --- 1. Get platforms ---
            let mut num_platforms: cl_uint = 0;
            let ret = clGetPlatformIDs(0, ptr::null_mut(), &mut num_platforms);
            if ret != CL_SUCCESS || num_platforms == 0 {
                return Err(WasiFpgaError::XrtError(
                    format!("clGetPlatformIDs failed: ret={}, num_platforms={}", ret, num_platforms)
                ));
            }
            eprintln!("[fpga_ocl] Found {} OpenCL platform(s)", num_platforms);

            let mut platforms = vec![ptr::null_mut(); num_platforms as usize];
            clGetPlatformIDs(num_platforms, platforms.as_mut_ptr(), ptr::null_mut());

            // --- 2. Get devices (ACCELERATOR type = Xilinx FPGA) ---
            let mut device: cl_device_id = ptr::null_mut();
            let mut found_device = false;

            for (i, &platform) in platforms.iter().enumerate() {
                let mut num_devices: cl_uint = 0;
                let ret = clGetDeviceIDs(
                    platform,
                    CL_DEVICE_TYPE_ACCELERATOR,
                    0,
                    ptr::null_mut(),
                    &mut num_devices,
                );

                if ret == CL_SUCCESS && num_devices > 0 {
                    let mut devices = vec![ptr::null_mut(); num_devices as usize];
                    clGetDeviceIDs(
                        platform,
                        CL_DEVICE_TYPE_ACCELERATOR,
                        num_devices,
                        devices.as_mut_ptr(),
                        ptr::null_mut(),
                    );

                    let idx = device_index as usize;
                    if idx < devices.len() {
                        device = devices[idx];
                        let name = get_device_name(device);
                        eprintln!("[fpga_ocl] Platform {}: device[{}] = '{}'", i, idx, name);
                        found_device = true;
                        break;
                    }
                }
            }

            if !found_device || device.is_null() {
                return Err(WasiFpgaError::DeviceNotFound);
            }

            // --- 3. Create context ---
            let mut err: cl_int = 0;
            let context = clCreateContext(
                ptr::null(),
                1,
                &device,
                ptr::null(),
                ptr::null_mut(),
                &mut err,
            );
            if err != CL_SUCCESS || context.is_null() {
                return Err(WasiFpgaError::XrtError(
                    format!("clCreateContext failed: err={}", err)
                ));
            }
            eprintln!("[fpga_ocl]  Context created");

            // --- 4. Create command queue ---
            let queue = clCreateCommandQueue(
                context,
                device,
                CL_QUEUE_PROFILING_ENABLE,
                &mut err,
            );
            if err != CL_SUCCESS || queue.is_null() {
                clReleaseContext(context);
                return Err(WasiFpgaError::XrtError(
                    format!("clCreateCommandQueue failed: err={}", err)
                ));
            }
            eprintln!("[fpga_ocl]  Command queue created");

            Ok(FpgaState {
                context,
                queue,
                device,
                program: None,
                kernel: None,
                kernel_name: None,
                buffers: HashMap::new(),
                next_id: 1,
            })
        }
    }

    // ========================================================================
    // XCLBIN LOADING
    // ========================================================================

    /// Carga un xclbin y crea el cl::Program
    pub fn load_xclbin(&mut self, filename: &str) -> Result<()> {
        let bin = std::fs::read(filename).map_err(|e| {
            WasiFpgaError::XrtError(format!("Cannot read xclbin '{}': {}", filename, e))
        })?;
        eprintln!("[fpga_ocl] Loading xclbin: {} ({} bytes)", filename, bin.len());

        unsafe {
            // Release previous program if any
            if let Some(prog) = self.program.take() {
                // Also release active kernel
                if let Some(kern) = self.kernel.take() {
                    clReleaseKernel(kern);
                }
                clReleaseProgram(prog);
            }

            let bin_size = bin.len();
            let bin_ptr = bin.as_ptr();
            let mut binary_status: cl_int = 0;
            let mut err: cl_int = 0;

            let program = clCreateProgramWithBinary(
                self.context,
                1,
                &self.device,
                &bin_size,
                &bin_ptr,
                &mut binary_status,
                &mut err,
            );

            if err != CL_SUCCESS || program.is_null() {
                return Err(WasiFpgaError::XrtError(
                    format!("clCreateProgramWithBinary failed: err={}, binary_status={}", err, binary_status)
                ));
            }

            // Build program
            let ret = clBuildProgram(
                program,
                1,
                &self.device,
                ptr::null(),
                ptr::null(),
                ptr::null_mut(),
            );
            if ret != CL_SUCCESS {
                // Get build log
                let mut log_size: usize = 0;
                clGetProgramBuildInfo(
                    program, self.device, CL_PROGRAM_BUILD_LOG,
                    0, ptr::null_mut(), &mut log_size,
                );
                let mut log_buf = vec![0u8; log_size];
                clGetProgramBuildInfo(
                    program, self.device, CL_PROGRAM_BUILD_LOG,
                    log_size, log_buf.as_mut_ptr() as *mut _, ptr::null_mut(),
                );
                let log = String::from_utf8_lossy(&log_buf);
                eprintln!("[fpga_ocl] Build log: {}", log);

                clReleaseProgram(program);
                return Err(WasiFpgaError::XrtError(
                    format!("clBuildProgram failed: ret={}", ret)
                ));
            }

            self.program = Some(program);
            eprintln!("[fpga_ocl]  Program loaded and built from '{}'", filename);
            Ok(())
        }
    }

    // ========================================================================
    // KERNEL MANAGEMENT
    // ========================================================================

    /// Crea un kernel a partir del programa cargado
    pub fn create_kernel(&mut self, name: &str) -> Result<()> {
        let program = self.program.ok_or_else(|| {
            WasiFpgaError::XrtError("No program loaded. Call load_xclbin first.".to_string())
        })?;

        unsafe {
            // Release previous kernel if any
            if let Some(kern) = self.kernel.take() {
                clReleaseKernel(kern);
            }

            let c_name = CString::new(name).map_err(|_| WasiFpgaError::InvalidKernelName)?;
            let mut err: cl_int = 0;
            let kernel = clCreateKernel(program, c_name.as_ptr(), &mut err);

            if err != CL_SUCCESS || kernel.is_null() {
                return Err(WasiFpgaError::XrtError(
                    format!("clCreateKernel('{}') failed: err={}", name, err)
                ));
            }

            self.kernel = Some(kernel);
            self.kernel_name = Some(name.to_string());
            eprintln!("[fpga_ocl]  Kernel '{}' created", name);
            Ok(())
        }
    }

    // ========================================================================
    // BUFFER MANAGEMENT
    // ========================================================================

    /// Aloca un buffer OpenCL de `size` bytes. Retorna buffer ID.
    ///
    /// Usa CL_MEM_USE_HOST_PTR con memoria alineada a 4096 bytes.
    /// XRT edge requiere alineación a página para usar el host_ptr
    /// directamente como backing DMA. Sin alineación, XRT crea una
    /// copia interna y los datos del kernel nunca llegan al host_ptr
    /// original, causando lecturas de ceros.
    pub fn alloc(&mut self, size: usize) -> Result<i32> {
        let mut host_data = AlignedBuffer::new(size)?;

        unsafe {
            let mut err: cl_int = 0;
            let mem = clCreateBuffer(
                self.context,
                CL_MEM_READ_WRITE | CL_MEM_USE_HOST_PTR,
                size,
                host_data.as_mut_ptr() as *mut c_void,
                &mut err,
            );

            if err != CL_SUCCESS || mem.is_null() {
                return Err(WasiFpgaError::XrtError(
                    format!("clCreateBuffer({} bytes, USE_HOST_PTR) failed: err={}", size, err)
                ));
            }

            let id = self.next_id;
            self.buffers.insert(id, OclBuffer { mem, size, host_data });
            self.next_id += 1;
            eprintln!("[fpga_ocl] Buffer {} allocated ({} bytes, page-aligned USE_HOST_PTR)", id, size);
            Ok(id)
        }
    }

    /// Escribe datos en un buffer (host memory directo).
    ///
    /// Con CL_MEM_USE_HOST_PTR, el host_ptr ES el backing store del
    /// cl_mem. Escribimos directamente al host_ptr alineado y dejamos
    /// que run_kernel() haga la migración host→device via
    /// clEnqueueMigrateMemObjects. NO usamos clEnqueueWriteBuffer
    /// porque en XRT edge interfiere con la ruta de migración,
    /// causando que el device reciba ceros.
    ///
    /// Este flujo replica exactamente el patrón del host code BCPNN
    /// de referencia (bcpnn_reference): memcpy → migrate → task.
    pub fn write(&mut self, buffer_id: i32, data: &[u8]) -> Result<()> {
        let buf = self.buffers.get_mut(&buffer_id)
            .ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;

        if data.len() > buf.size {
            return Err(WasiFpgaError::OutOfBounds {
                offset: 0,
                length: data.len(),
                buffer_size: buf.size,
            });
        }

        buf.host_data.copy_from_slice(data);
        Ok(())
    }

    /// Lee datos de un buffer (host memory directo).
    ///
    /// Después de run_kernel(), clEnqueueMigrateMemObjects(TO_HOST)
    /// + clFinish() garantizan que el host_ptr tiene los datos más
    /// recientes del device. Leemos directamente del host_ptr en
    /// lugar de usar clEnqueueReadBuffer, que en XRT edge puede
    /// devolver datos obsoletos con USE_HOST_PTR.
    ///
    /// Replica el patrón BCPNN: migrate → finish → memcpy.
    pub fn read(&self, buffer_id: i32, length: usize) -> Result<Vec<u8>> {
        let buf = self.buffers.get(&buffer_id)
            .ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;

        let read_len = length.min(buf.size);
        Ok(buf.host_data.as_slice(read_len).to_vec())
    }

    /// Libera un buffer
    pub fn free(&mut self, buffer_id: i32) -> Result<()> {
        let buf = self.buffers.remove(&buffer_id)
            .ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;
        unsafe {
            clReleaseMemObject(buf.mem);
        }
        eprintln!("[fpga_ocl] Buffer {} freed", buffer_id);
        Ok(())
    }

    // ========================================================================
    // KERNEL ARGUMENT SETTING
    // ========================================================================

    /// Asigna un buffer como argumento del kernel
    pub fn set_arg_buffer(&self, arg_index: u32, buffer_id: i32) -> Result<()> {
        let kernel = self.kernel.ok_or_else(|| {
            WasiFpgaError::XrtError("No kernel created. Call create_kernel first.".to_string())
        })?;
        let buf = self.buffers.get(&buffer_id)
            .ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;

        unsafe {
            let ret = clSetKernelArg(
                kernel,
                arg_index,
                std::mem::size_of::<cl_mem>(),
                &buf.mem as *const cl_mem as *const c_void,
            );
            if ret != CL_SUCCESS {
                return Err(WasiFpgaError::XrtError(
                    format!("clSetKernelArg(idx={}, buf={}) failed: ret={}", arg_index, buffer_id, ret)
                ));
            }
        }
        Ok(())
    }

    /// Asigna un entero como argumento escalar del kernel
    pub fn set_arg_int(&self, arg_index: u32, value: i32) -> Result<()> {
        let kernel = self.kernel.ok_or_else(|| {
            WasiFpgaError::XrtError("No kernel created. Call create_kernel first.".to_string())
        })?;

        unsafe {
            let ret = clSetKernelArg(
                kernel,
                arg_index,
                std::mem::size_of::<i32>(),
                &value as *const i32 as *const c_void,
            );
            if ret != CL_SUCCESS {
                return Err(WasiFpgaError::XrtError(
                    format!("clSetKernelArg(idx={}, int={}) failed: ret={}", arg_index, value, ret)
                ));
            }
        }
        Ok(())
    }

    /// Asigna un float como argumento escalar del kernel
    pub fn set_arg_float(&self, arg_index: u32, value: f32) -> Result<()> {
        let kernel = self.kernel.ok_or_else(|| {
            WasiFpgaError::XrtError("No kernel created. Call create_kernel first.".to_string())
        })?;

        unsafe {
            let ret = clSetKernelArg(
                kernel,
                arg_index,
                std::mem::size_of::<f32>(),
                &value as *const f32 as *const c_void,
            );
            if ret != CL_SUCCESS {
                return Err(WasiFpgaError::XrtError(
                    format!("clSetKernelArg(idx={}, float={}) failed: ret={}", arg_index, value, ret)
                ));
            }
        }
        Ok(())
    }

    // ========================================================================
    // KERNEL EXECUTION (el flujo completo como BCPNN)
    // ========================================================================

    /// Ejecuta el kernel con el patrón completo de BCPNN:
    ///   1. Migrar buffers de entrada al dispositivo
    ///   2. Ejecutar kernel (enqueueTask)
    ///   3. Migrar buffers de salida al host
    ///   4. Finish (esperar completación)
    ///
    /// `input_buf_ids`:  IDs de buffers a migrar host→device ANTES de ejecutar
    /// `output_buf_ids`: IDs de buffers a migrar device→host DESPUÉS de ejecutar
    ///
    /// Returns `RunTimings` with OpenCL profiling data (when `WASI_FPGA_BENCH=1`)
    /// and wall-clock total time.
    pub fn run_kernel(
        &self,
        input_buf_ids: &[i32],
        output_buf_ids: &[i32],
    ) -> Result<RunTimings> {
        let kernel = self.kernel.ok_or_else(|| {
            WasiFpgaError::XrtError("No kernel created.".to_string())
        })?;
        let kernel_name = self.kernel_name.as_deref().unwrap_or("unknown");

        eprintln!("[fpga_ocl] === RUN KERNEL '{}' ===", kernel_name);
        eprintln!("[fpga_ocl]   inputs to migrate: {:?}", input_buf_ids);
        eprintln!("[fpga_ocl]   outputs to migrate: {:?}", output_buf_ids);

        let wall_start = Instant::now();
        let do_profile = bench_enabled();

        // Event handles for profiling (null when profiling disabled)
        let mut evt_migrate_in: cl_event = ptr::null_mut();
        let mut evt_kernel: cl_event = ptr::null_mut();
        let mut evt_migrate_out: cl_event = ptr::null_mut();

        unsafe {
            // --- 1. Migrate input buffers TO device ---
            if !input_buf_ids.is_empty() {
                let input_mems: std::result::Result<Vec<cl_mem>, _> = input_buf_ids
                    .iter()
                    .map(|&id| {
                        self.buffers.get(&id)
                            .map(|b| b.mem)
                            .ok_or(WasiFpgaError::InvalidBufferId(id))
                    })
                    .collect();
                let input_mems = input_mems?;

                let evt_ptr = if do_profile { &mut evt_migrate_in } else { ptr::null_mut() };
                let ret = clEnqueueMigrateMemObjects(
                    self.queue,
                    input_mems.len() as cl_uint,
                    input_mems.as_ptr(),
                    0, // flags=0 → host-to-device
                    0,
                    ptr::null(),
                    evt_ptr,
                );
                if ret != CL_SUCCESS {
                    if do_profile && !evt_migrate_in.is_null() { clReleaseEvent(evt_migrate_in); }
                    return Err(WasiFpgaError::XrtError(
                        format!("clEnqueueMigrateMemObjects(TO_DEVICE) failed: ret={}", ret)
                    ));
                }
                eprintln!("[fpga_ocl]  {} input buffers migrated to device", input_mems.len());
            }

            // --- 2. Execute kernel ---
            let evt_ptr = if do_profile { &mut evt_kernel } else { ptr::null_mut() };
            let ret = clEnqueueTask(
                self.queue,
                kernel,
                0,
                ptr::null(),
                evt_ptr,
            );
            if ret != CL_SUCCESS {
                if do_profile {
                    if !evt_migrate_in.is_null() { clReleaseEvent(evt_migrate_in); }
                    if !evt_kernel.is_null() { clReleaseEvent(evt_kernel); }
                }
                return Err(WasiFpgaError::XrtError(
                    format!("clEnqueueTask('{}') failed: ret={}", kernel_name, ret)
                ));
            }
            eprintln!("[fpga_ocl]  Kernel '{}' enqueued", kernel_name);

            // --- 3. Migrate output buffers FROM device ---
            if !output_buf_ids.is_empty() {
                let output_mems: std::result::Result<Vec<cl_mem>, _> = output_buf_ids
                    .iter()
                    .map(|&id| {
                        self.buffers.get(&id)
                            .map(|b| b.mem)
                            .ok_or(WasiFpgaError::InvalidBufferId(id))
                    })
                    .collect();
                let output_mems = output_mems?;

                let evt_ptr = if do_profile { &mut evt_migrate_out } else { ptr::null_mut() };
                let ret = clEnqueueMigrateMemObjects(
                    self.queue,
                    output_mems.len() as cl_uint,
                    output_mems.as_ptr(),
                    CL_MIGRATE_MEM_OBJECT_HOST,
                    0,
                    ptr::null(),
                    evt_ptr,
                );
                if ret != CL_SUCCESS {
                    if do_profile {
                        if !evt_migrate_in.is_null() { clReleaseEvent(evt_migrate_in); }
                        if !evt_kernel.is_null() { clReleaseEvent(evt_kernel); }
                        if !evt_migrate_out.is_null() { clReleaseEvent(evt_migrate_out); }
                    }
                    return Err(WasiFpgaError::XrtError(
                        format!("clEnqueueMigrateMemObjects(TO_HOST) failed: ret={}", ret)
                    ));
                }
                eprintln!("[fpga_ocl]  {} output buffers migration enqueued", output_mems.len());
            }

            // --- 4. Wait for everything to complete ---
            let ret = clFinish(self.queue);
            if ret != CL_SUCCESS {
                return Err(WasiFpgaError::XrtError(
                    format!("clFinish failed: ret={}", ret)
                ));
            }
            eprintln!("[fpga_ocl]  Kernel '{}' execution complete", kernel_name);

            // --- 5. Collect profiling data and release events ---
            let mut timings = RunTimings {
                total_wall_ns: wall_start.elapsed().as_nanos() as u64,
                ..Default::default()
            };

            if do_profile {
                timings.migrate_in_ns = get_event_duration_ns(evt_migrate_in);
                timings.kernel_ns = get_event_duration_ns(evt_kernel);
                timings.migrate_out_ns = get_event_duration_ns(evt_migrate_out);

                if !evt_migrate_in.is_null() { clReleaseEvent(evt_migrate_in); }
                if !evt_kernel.is_null() { clReleaseEvent(evt_kernel); }
                if !evt_migrate_out.is_null() { clReleaseEvent(evt_migrate_out); }
            }

            Ok(timings)
        }
    }
}

// ============================================================================
// CLEANUP
// ============================================================================

impl Drop for FpgaState {
    fn drop(&mut self) {
        eprintln!("[fpga_ocl] FpgaState Drop — releasing OpenCL resources...");
        unsafe {
            // 1. Free remaining buffers (if any weren't freed manually)
            for (id, buf) in self.buffers.drain() {
                clReleaseMemObject(buf.mem);
                eprintln!("[fpga_ocl] Buffer {} released (in Drop)", id);
            }

            // 2. Release kernel
            if let Some(kernel) = self.kernel.take() {
                clReleaseKernel(kernel);
                eprintln!("[fpga_ocl] Kernel released");
            }

            // 3. Release program
            if let Some(program) = self.program.take() {
                clReleaseProgram(program);
                eprintln!("[fpga_ocl] Program released");
            }

            // 4+5. SKIP releasing queue and context.
            //    XRT/ZOCL on ZCU104 does double-free when releasing
            //    the command queue after buffers are already released.
            //    The OS reclaims all resources when the process exits.
            //    This is safe and is a known workaround for XRT edge.
            eprintln!("[fpga_ocl] Skipping queue/context release (XRT edge workaround)");
        }
        eprintln!("[fpga_ocl] FpgaState Drop complete");
    }
}
