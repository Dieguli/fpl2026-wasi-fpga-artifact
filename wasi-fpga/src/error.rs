//! Error types for WASI FPGA extensions (OpenCL backend)

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WasiFpgaError {
    #[error("FPGA device not found (no OpenCL accelerator)")]
    DeviceNotFound,

    #[error("FPGA device already initialized")]
    AlreadyInitialized,

    #[error("FPGA device not initialized - call fpga_init() first")]
    NotInitialized,

    #[error("Invalid buffer ID: {0}")]
    InvalidBufferId(i32),

    #[error("Buffer size out of range: {0} (max: 1GB)")]
    InvalidBufferSize(usize),

    #[error("Buffer operation out of bounds: offset={offset}, length={length}, buffer_size={buffer_size}")]
    OutOfBounds {
        offset: usize,
        length: usize,
        buffer_size: usize,
    },

    #[error("OpenCL/XRT error: {0}")]
    XrtError(String),

    #[error("Memory allocation failed")]
    AllocationFailed,

    #[error("Invalid kernel name")]
    InvalidKernelName,

    #[error("No program loaded (call load_xclbin first)")]
    NoProgramLoaded,

    #[error("No kernel created (call create_kernel first)")]
    NoKernelCreated,
}

pub type Result<T> = std::result::Result<T, WasiFpgaError>;
