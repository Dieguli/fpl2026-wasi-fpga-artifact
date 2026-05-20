//! OpenCL C API bindings para Xilinx FPGA (ZCU104)
//!
//! Bindings mínimos de la API OpenCL C que usa XRT como backend.
//! Usamos estos en vez de las funciones xclRegWrite/xclRegRead de bajo nivel,
//! porque el flujo OpenCL (cl::Buffer + enqueueMigrateMemObjects) es el que
//! funciona con los PAC (Platform Assets Containers) de las demos BCPNN.

#![allow(non_camel_case_types, dead_code)]

use std::os::raw::{c_void, c_char};
use std::ptr;

// ============================================================================
// TIPOS FUNDAMENTALES
// ============================================================================

pub type cl_int = i32;
pub type cl_uint = u32;
pub type cl_ulong = u64;
pub type cl_bool = cl_uint;

// Handles opacos
pub type cl_platform_id = *mut c_void;
pub type cl_device_id = *mut c_void;
pub type cl_context = *mut c_void;
pub type cl_command_queue = *mut c_void;
pub type cl_program = *mut c_void;
pub type cl_kernel = *mut c_void;
pub type cl_mem = *mut c_void;
pub type cl_event = *mut c_void;

// Flags
pub type cl_mem_flags = cl_ulong;
pub type cl_device_type = cl_ulong;
pub type cl_command_queue_properties = cl_ulong;
pub type cl_mem_migration_flags = cl_ulong;
pub type cl_context_properties = isize;
pub type cl_program_info = cl_uint;
pub type cl_program_build_info = cl_uint;
pub type cl_device_info = cl_uint;

// ============================================================================
// CONSTANTES
// ============================================================================

// Return codes
pub const CL_SUCCESS: cl_int = 0;
pub const CL_INVALID_VALUE: cl_int = -30;
pub const CL_INVALID_PLATFORM: cl_int = -32;
pub const CL_INVALID_DEVICE: cl_int = -33;
pub const CL_INVALID_CONTEXT: cl_int = -34;
pub const CL_INVALID_PROGRAM: cl_int = -44;

// Device types
pub const CL_DEVICE_TYPE_DEFAULT: cl_device_type = 1;
pub const CL_DEVICE_TYPE_CPU: cl_device_type = 2;
pub const CL_DEVICE_TYPE_GPU: cl_device_type = 4;
pub const CL_DEVICE_TYPE_ACCELERATOR: cl_device_type = 8;
pub const CL_DEVICE_TYPE_ALL: cl_device_type = 0xFFFFFFFF;

// Memory flags
pub const CL_MEM_READ_WRITE: cl_mem_flags = 1 << 0;
pub const CL_MEM_WRITE_ONLY: cl_mem_flags = 1 << 1;
pub const CL_MEM_READ_ONLY: cl_mem_flags = 1 << 2;
pub const CL_MEM_USE_HOST_PTR: cl_mem_flags = 1 << 3;
pub const CL_MEM_ALLOC_HOST_PTR: cl_mem_flags = 1 << 4;
pub const CL_MEM_COPY_HOST_PTR: cl_mem_flags = 1 << 5;

// Migration flags
pub const CL_MIGRATE_MEM_OBJECT_HOST: cl_mem_migration_flags = 1 << 0;
pub const CL_MIGRATE_MEM_OBJECT_CONTENT_UNDEFINED: cl_mem_migration_flags = 1 << 1;

// Command queue properties
pub const CL_QUEUE_PROFILING_ENABLE: cl_command_queue_properties = 1 << 1;

// Profiling info (for clGetEventProfilingInfo)
pub type cl_profiling_info = cl_uint;
pub const CL_PROFILING_COMMAND_QUEUED: cl_profiling_info = 0x1280;
pub const CL_PROFILING_COMMAND_SUBMIT: cl_profiling_info = 0x1281;
pub const CL_PROFILING_COMMAND_START:  cl_profiling_info = 0x1282;
pub const CL_PROFILING_COMMAND_END:    cl_profiling_info = 0x1283;

// Blocking flags
pub const CL_TRUE: cl_bool = 1;
pub const CL_FALSE: cl_bool = 0;

// Device info
pub const CL_DEVICE_NAME: cl_device_info = 0x102B;
pub const CL_DEVICE_VENDOR: cl_device_info = 0x102C;

// Program build info
pub const CL_PROGRAM_BUILD_LOG: cl_program_build_info = 0x1183;

// ============================================================================
// FUNCIONES OPENCL C API
// ============================================================================

extern "C" {
    // --- Platform ---
    pub fn clGetPlatformIDs(
        num_entries: cl_uint,
        platforms: *mut cl_platform_id,
        num_platforms: *mut cl_uint,
    ) -> cl_int;

    // --- Device ---
    pub fn clGetDeviceIDs(
        platform: cl_platform_id,
        device_type: cl_device_type,
        num_entries: cl_uint,
        devices: *mut cl_device_id,
        num_devices: *mut cl_uint,
    ) -> cl_int;

    pub fn clGetDeviceInfo(
        device: cl_device_id,
        param_name: cl_device_info,
        param_value_size: usize,
        param_value: *mut c_void,
        param_value_size_ret: *mut usize,
    ) -> cl_int;

    // --- Context ---
    pub fn clCreateContext(
        properties: *const cl_context_properties,
        num_devices: cl_uint,
        devices: *const cl_device_id,
        pfn_notify: *const c_void,
        user_data: *mut c_void,
        errcode_ret: *mut cl_int,
    ) -> cl_context;

    pub fn clReleaseContext(context: cl_context) -> cl_int;

    // --- Command Queue ---
    pub fn clCreateCommandQueue(
        context: cl_context,
        device: cl_device_id,
        properties: cl_command_queue_properties,
        errcode_ret: *mut cl_int,
    ) -> cl_command_queue;

    pub fn clFinish(command_queue: cl_command_queue) -> cl_int;

    pub fn clReleaseCommandQueue(command_queue: cl_command_queue) -> cl_int;

    // --- Program ---
    pub fn clCreateProgramWithBinary(
        context: cl_context,
        num_devices: cl_uint,
        device_list: *const cl_device_id,
        lengths: *const usize,
        binaries: *const *const u8,
        binary_status: *mut cl_int,
        errcode_ret: *mut cl_int,
    ) -> cl_program;

    pub fn clBuildProgram(
        program: cl_program,
        num_devices: cl_uint,
        device_list: *const cl_device_id,
        options: *const c_char,
        pfn_notify: *const c_void,
        user_data: *mut c_void,
    ) -> cl_int;

    pub fn clReleaseProgram(program: cl_program) -> cl_int;

    pub fn clGetProgramBuildInfo(
        program: cl_program,
        device: cl_device_id,
        param_name: cl_program_build_info,
        param_value_size: usize,
        param_value: *mut c_void,
        param_value_size_ret: *mut usize,
    ) -> cl_int;

    // --- Kernel ---
    pub fn clCreateKernel(
        program: cl_program,
        kernel_name: *const c_char,
        errcode_ret: *mut cl_int,
    ) -> cl_kernel;

    pub fn clSetKernelArg(
        kernel: cl_kernel,
        arg_index: cl_uint,
        arg_size: usize,
        arg_value: *const c_void,
    ) -> cl_int;

    pub fn clReleaseKernel(kernel: cl_kernel) -> cl_int;

    // --- Buffer ---
    pub fn clCreateBuffer(
        context: cl_context,
        flags: cl_mem_flags,
        size: usize,
        host_ptr: *mut c_void,
        errcode_ret: *mut cl_int,
    ) -> cl_mem;

    pub fn clReleaseMemObject(memobj: cl_mem) -> cl_int;

    // --- Enqueue Operations ---
    pub fn clEnqueueWriteBuffer(
        command_queue: cl_command_queue,
        buffer: cl_mem,
        blocking_write: cl_bool,
        offset: usize,
        size: usize,
        ptr: *const c_void,
        num_events_in_wait_list: cl_uint,
        event_wait_list: *const cl_event,
        event: *mut cl_event,
    ) -> cl_int;

    pub fn clEnqueueReadBuffer(
        command_queue: cl_command_queue,
        buffer: cl_mem,
        blocking_read: cl_bool,
        offset: usize,
        size: usize,
        ptr: *mut c_void,
        num_events_in_wait_list: cl_uint,
        event_wait_list: *const cl_event,
        event: *mut cl_event,
    ) -> cl_int;

    pub fn clEnqueueTask(
        command_queue: cl_command_queue,
        kernel: cl_kernel,
        num_events_in_wait_list: cl_uint,
        event_wait_list: *const cl_event,
        event: *mut cl_event,
    ) -> cl_int;

    pub fn clEnqueueMigrateMemObjects(
        command_queue: cl_command_queue,
        num_mem_objects: cl_uint,
        mem_objects: *const cl_mem,
        flags: cl_mem_migration_flags,
        num_events_in_wait_list: cl_uint,
        event_wait_list: *const cl_event,
        event: *mut cl_event,
    ) -> cl_int;

    // --- Event Profiling ---
    pub fn clGetEventProfilingInfo(
        event: cl_event,
        param_name: cl_profiling_info,
        param_value_size: usize,
        param_value: *mut c_void,
        param_value_size_ret: *mut usize,
    ) -> cl_int;

    pub fn clWaitForEvents(
        num_events: cl_uint,
        event_list: *const cl_event,
    ) -> cl_int;

    pub fn clReleaseEvent(event: cl_event) -> cl_int;
}

// ============================================================================
// HELPER: Obtener nombre del dispositivo
// ============================================================================

pub unsafe fn get_device_name(device: cl_device_id) -> String {
    let mut name_buf = vec![0u8; 256];
    let mut name_len: usize = 0;
    let ret = clGetDeviceInfo(
        device,
        CL_DEVICE_NAME,
        name_buf.len(),
        name_buf.as_mut_ptr() as *mut c_void,
        &mut name_len,
    );
    if ret == CL_SUCCESS && name_len > 0 {
        name_buf.truncate(name_len.saturating_sub(1)); // Remove null terminator
        String::from_utf8_lossy(&name_buf).to_string()
    } else {
        format!("unknown (err={})", ret)
    }
}

/// Extracts the duration (END - START) from an OpenCL profiling event, in nanoseconds.
/// Returns None if the event is null or the query fails.
pub unsafe fn get_event_duration_ns(event: cl_event) -> Option<u64> {
    if event.is_null() {
        return None;
    }
    let mut start: cl_ulong = 0;
    let mut end: cl_ulong = 0;
    let r1 = clGetEventProfilingInfo(
        event,
        CL_PROFILING_COMMAND_START,
        std::mem::size_of::<cl_ulong>(),
        &mut start as *mut cl_ulong as *mut c_void,
        ptr::null_mut(),
    );
    let r2 = clGetEventProfilingInfo(
        event,
        CL_PROFILING_COMMAND_END,
        std::mem::size_of::<cl_ulong>(),
        &mut end as *mut cl_ulong as *mut c_void,
        ptr::null_mut(),
    );
    if r1 == CL_SUCCESS && r2 == CL_SUCCESS && end >= start {
        Some(end - start)
    } else {
        None
    }
}
