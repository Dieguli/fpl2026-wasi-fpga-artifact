// use crate::error::{WasiFpgaError, Result};
// use std::os::raw::{c_void, c_int, c_char, c_uint, c_ulong};
// use std::ptr;
// use std::fs::OpenOptions;
// use memmap2::MmapOptions;

// // Tipos C
// pub type XclDeviceHandle = *mut c_void;
// pub type XclBufferHandle = c_uint;

// #[repr(C)]
// pub struct xclBOProperties {
//     pub domain: c_uint,
//     pub flags: c_uint,
//     pub size: c_ulong,
//     pub paddr: c_ulong, 
// }

// // Bindings a la librería XRT (libxrt_core.so)
// extern "C" {
//     pub fn xclOpen(deviceIndex: c_uint, logFileName: *const c_char, level: c_int) -> XclDeviceHandle;
//     pub fn xclClose(handle: XclDeviceHandle);
//     pub fn xclAllocBO(handle: XclDeviceHandle, size: usize, domain: c_int, flags: c_uint) -> XclBufferHandle;
//     pub fn xclFreeBO(handle: XclDeviceHandle, boHandle: XclBufferHandle);
//     pub fn xclSyncBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, dir: c_int, size: usize, offset: usize) -> c_int;
//     pub fn xclRegWrite(handle: XclDeviceHandle, cu_index: c_uint, offset: c_uint, value: c_uint) -> c_int;
//     pub fn xclRegRead(handle: XclDeviceHandle, cu_index: c_uint, offset: c_uint, datap: *mut c_uint) -> c_int;
//     pub fn xclGetBOProperties(handle: XclDeviceHandle, boHandle: XclBufferHandle, properties: *mut xclBOProperties) -> c_int;
//     pub fn xclLoadXclBin(handle: XclDeviceHandle, buffer: *const c_void) -> c_int;
//     pub fn xclMapBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, write: bool) -> *mut c_void;
//     pub fn xclUnmapBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, addr: *mut c_void) -> c_int;
// }

// pub struct Device { handle: XclDeviceHandle }
// unsafe impl Send for Device {}
// unsafe impl Sync for Device {}

// impl Device {
//     pub fn open(index: u32) -> Result<Self> {
//         let handle = unsafe { xclOpen(index, ptr::null(), 0) };
//         if handle.is_null() { Err(WasiFpgaError::DeviceNotFound) } else { Ok(Device { handle }) }
//     }
//     pub fn handle(&self) -> XclDeviceHandle { self.handle }
// }
// impl Drop for Device { fn drop(&mut self) { unsafe { xclClose(self.handle); } } }

// pub struct Buffer { device: XclDeviceHandle, handle: XclBufferHandle, size: usize }
// unsafe impl Send for Buffer {}
// unsafe impl Sync for Buffer {}

// impl Buffer {
//     /// Memory bank index matching the kernel's gmem connectivity.
//     /// From xclbinutil: Kernel matmul → M_AXI_GMEM → HP0 (Index 4)
//     const HP0_BANK_INDEX: c_uint = 4;

//     pub fn alloc(device: XclDeviceHandle, size: usize) -> Result<Self> {
//         // Domain 1 = XCL_BO_NORMAL (DDR system memory, required for AXI-accessible buffers)
//         // Domain 0 = XCL_BO_DEVICE_RAM (BRAM on-chip, not AXI-accessible from external logic)
//         let handle = unsafe { xclAllocBO(device, size, 1, Self::HP0_BANK_INDEX) };
        
//         if handle == 0xFFFFFFFF { 
//             return Err(WasiFpgaError::AllocationFailed);
//         }
//         Ok(Buffer { device, handle, size })
//     }

//     pub fn handle(&self) -> XclBufferHandle { self.handle }
//     pub fn size(&self) -> usize { self.size }

//     pub fn write(&self, data: &[u8], offset: usize) -> Result<()> {
//         unsafe {
//             let ptr = xclMapBO(self.device, self.handle, true);
//             if ptr.is_null() { return Err(WasiFpgaError::AllocationFailed); }
//             std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset) as *mut u8, data.len());
//             xclUnmapBO(self.device, self.handle, ptr);
//             self.sync_to_device()
//         }
//     }

//     pub fn read(&self, len: usize, offset: usize) -> Result<Vec<u8>> {
//         unsafe {
//             self.sync_from_device()?; 
//             let mut buf = vec![0u8; len];
//             let ptr = xclMapBO(self.device, self.handle, false);
//             if ptr.is_null() { return Err(WasiFpgaError::AllocationFailed); }
//             std::ptr::copy_nonoverlapping(ptr.add(offset) as *const u8, buf.as_mut_ptr(), len);
//             xclUnmapBO(self.device, self.handle, ptr);
//             Ok(buf)
//         }
//     }

//     pub fn sync_to_device(&self) -> Result<()> {
//         unsafe { xclSyncBO(self.device, self.handle, 0, self.size, 0); Ok(()) }
//     }
//     pub fn sync_from_device(&self) -> Result<()> {
//         unsafe { xclSyncBO(self.device, self.handle, 1, self.size, 0); Ok(()) }
//     }
// }
// impl Drop for Buffer {
//     fn drop(&mut self) { if self.handle != 0 { unsafe { xclFreeBO(self.device, self.handle); } } }
// }

// // Estructura para acceso directo a memoria física (Bypass XRT)
// pub struct PhysMem {
//     mmap: memmap2::MmapMut,
// }

// impl PhysMem {
//     pub fn new(base_addr: u64, size: usize) -> std::io::Result<Self> {
//         let file = OpenOptions::new()
//             .read(true)
//             .write(true)
//             // .sync(true)
//             .open("/dev/phys_map")?;

//         // Mapeamos la memoria física al espacio virtual
//         let mmap = unsafe {
//             MmapOptions::new()
//                 .offset(0)
//                 .len(size)
//                 .map_mut(&file)?
//         };

//         Ok(PhysMem { mmap })
//     }

//     pub fn write_u32(&mut self, offset_bytes: usize, val: u32) {
//         let ptr = self.mmap.as_mut_ptr();
//         unsafe {
//             *(ptr.add(offset_bytes) as *mut u32) = val;
//         }
//     }

//     pub fn read_u32(&self, offset_bytes: usize) -> u32 {
//         let ptr = self.mmap.as_ptr();
//         unsafe {
//             *(ptr.add(offset_bytes) as *const u32)
//         }
//     }
// }


use crate::error::{WasiFpgaError, Result};
use std::os::raw::{c_void, c_int, c_char, c_uint, c_ulong};
use std::ptr;
use std::fs::OpenOptions;
use memmap2::MmapOptions;

// Tipos C
pub type XclDeviceHandle = *mut c_void;
pub type XclBufferHandle = c_uint;

#[repr(C)]
pub struct xclBOProperties {
    pub domain: c_uint,
    pub flags: c_uint,
    pub size: c_ulong,
    pub paddr: c_ulong, 
}

// xuid_t is a 16-byte UUID (unsigned char[16] on Linux)
pub type XclUuid = [u8; 16];

// Bindings a la librería XRT (libxrt_core.so)
extern "C" {
    pub fn xclOpen(deviceIndex: c_uint, logFileName: *const c_char, level: c_int) -> XclDeviceHandle;
    pub fn xclClose(handle: XclDeviceHandle);
    pub fn xclAllocBO(handle: XclDeviceHandle, size: usize, domain: c_int, flags: c_uint) -> XclBufferHandle;
    pub fn xclFreeBO(handle: XclDeviceHandle, boHandle: XclBufferHandle);
    pub fn xclSyncBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, dir: c_int, size: usize, offset: usize) -> c_int;
    pub fn xclRegWrite(handle: XclDeviceHandle, cu_index: c_uint, offset: c_uint, value: c_uint) -> c_int;
    pub fn xclRegRead(handle: XclDeviceHandle, cu_index: c_uint, offset: c_uint, datap: *mut c_uint) -> c_int;
    pub fn xclGetBOProperties(handle: XclDeviceHandle, boHandle: XclBufferHandle, properties: *mut xclBOProperties) -> c_int;
    pub fn xclLoadXclBin(handle: XclDeviceHandle, buffer: *const c_void) -> c_int;
    pub fn xclMapBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, write: bool) -> *mut c_void;
    pub fn xclUnmapBO(handle: XclDeviceHandle, boHandle: XclBufferHandle, addr: *mut c_void) -> c_int;
    // CU context management — REQUIRED for xclRegWrite/xclRegRead and kernel AXI master access
    pub fn xclOpenContext(handle: XclDeviceHandle, xclbinId: *const u8, ipIndex: c_uint, shared: bool) -> c_int;
    pub fn xclCloseContext(handle: XclDeviceHandle, xclbinId: *const u8, ipIndex: c_uint) -> c_int;
}

/// Offset of the UUID field within the axlf (xclbin) binary format.
/// Layout: axlf.m_header.uuid is at byte 416 from file start.
/// Verified against XRT source: xrt/detail/xclbin.h
pub const AXLF_UUID_OFFSET: usize = 416;
pub const AXLF_UUID_SIZE: usize = 16;
pub const AXLF_MAGIC: &[u8] = b"xclbin2\0";

/// Extract UUID from an xclbin binary blob
pub fn extract_xclbin_uuid(xclbin_data: &[u8]) -> Option<XclUuid> {
    if xclbin_data.len() < AXLF_UUID_OFFSET + AXLF_UUID_SIZE {
        return None;
    }
    // Verify magic header
    if &xclbin_data[0..8] != AXLF_MAGIC {
        return None;
    }
    let mut uuid = [0u8; 16];
    uuid.copy_from_slice(&xclbin_data[AXLF_UUID_OFFSET..AXLF_UUID_OFFSET + AXLF_UUID_SIZE]);
    Some(uuid)
}

pub struct Device { handle: XclDeviceHandle }
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    pub fn open(index: u32) -> Result<Self> {
        let handle = unsafe { xclOpen(index, ptr::null(), 0) };
        if handle.is_null() { Err(WasiFpgaError::DeviceNotFound) } else { Ok(Device { handle }) }
    }
    pub fn handle(&self) -> XclDeviceHandle { self.handle }
}
impl Drop for Device { fn drop(&mut self) { unsafe { xclClose(self.handle); } } }

pub struct Buffer { device: XclDeviceHandle, handle: XclBufferHandle, size: usize }
unsafe impl Send for Buffer {}
unsafe impl Sync for Buffer {}

impl Buffer {
    /// Memory bank index for xclAllocBO.
    /// HP0 = 4 on ZCU104. Set to 0 to use platform default bank.
    /// IMPORTANT: This MUST match the kernel's memory connectivity in the xclbin.
    /// Check with: xclbinutil --input your.xclbin --info (look at Memory column)
    const MEMORY_BANK: c_uint = 0;  // 0=default, 4=HP0

    pub fn alloc(device: XclDeviceHandle, size: usize) -> Result<Self> {
        eprintln!("[wasi_fpga] xclAllocBO: size={} flags={}", size, Self::MEMORY_BANK);
        let handle = unsafe { xclAllocBO(device, size, 0, Self::MEMORY_BANK) };

        if handle == 0xFFFFFFFF {
            eprintln!("[wasi_fpga] ERROR: xclAllocBO failed (handle=0xFFFFFFFF)");
            return Err(WasiFpgaError::AllocationFailed);
        }
        eprintln!("[wasi_fpga] xclAllocBO: handle={}", handle);
        Ok(Buffer { device, handle, size })
    }

    pub fn handle(&self) -> XclBufferHandle { self.handle }
    pub fn size(&self) -> usize { self.size }

    pub fn write(&self, data: &[u8], offset: usize) -> Result<()> {
        unsafe {
            let ptr = xclMapBO(self.device, self.handle, true);
            if ptr.is_null() { return Err(WasiFpgaError::AllocationFailed); }
            std::ptr::copy_nonoverlapping(data.as_ptr(), ptr.add(offset) as *mut u8, data.len());
            xclUnmapBO(self.device, self.handle, ptr);
            self.sync_to_device()
        }
    }

    pub fn read(&self, len: usize, offset: usize) -> Result<Vec<u8>> {
        unsafe {
            self.sync_from_device()?; 
            let mut buf = vec![0u8; len];
            let ptr = xclMapBO(self.device, self.handle, false);
            if ptr.is_null() { return Err(WasiFpgaError::AllocationFailed); }
            std::ptr::copy_nonoverlapping(ptr.add(offset) as *const u8, buf.as_mut_ptr(), len);
            xclUnmapBO(self.device, self.handle, ptr);
            Ok(buf)
        }
    }

    pub fn sync_to_device(&self) -> Result<()> {
        unsafe { xclSyncBO(self.device, self.handle, 0, self.size, 0); Ok(()) }
    }
    pub fn sync_from_device(&self) -> Result<()> {
        unsafe { xclSyncBO(self.device, self.handle, 1, self.size, 0); Ok(()) }
    }
}
impl Drop for Buffer {
    fn drop(&mut self) { if self.handle != 0 { unsafe { xclFreeBO(self.device, self.handle); } } }
}

// Estructura para acceso directo a memoria física (Bypass XRT)
pub struct PhysMem {
    mmap: memmap2::MmapMut,
}

impl PhysMem {
    pub fn new(base_addr: u64, size: usize) -> std::io::Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            // .sync(true)
            .open("/dev/phys_map")?;

        // Mapeamos la memoria física al espacio virtual
        let mmap = unsafe {
            MmapOptions::new()
                .offset(0)
                .len(size)
                .map_mut(&file)?
        };

        Ok(PhysMem { mmap })
    }

    pub fn write_u32(&mut self, offset_bytes: usize, val: u32) {
        let ptr = self.mmap.as_mut_ptr();
        unsafe {
            *(ptr.add(offset_bytes) as *mut u32) = val;
        }
    }

    pub fn read_u32(&self, offset_bytes: usize) -> u32 {
        let ptr = self.mmap.as_ptr();
        unsafe {
            *(ptr.add(offset_bytes) as *const u32)
        }
    }
}