use std::fs;

fn main() {
    let xclbin_path = std::env::args().nth(1).unwrap_or_else(|| "test_const_write.xclbin".to_string());
    
    println!("[TEST] Testing constant write kernel...");
    
    // Load XClbin
    let bitstream = fs::read(&xclbin_path).expect("Failed to read xclbin");
    println!("[TEST] Loaded XClbin: {} bytes", bitstream.len());
    
    // Initialize XRT
    let device_id = 0;
    let xcl_device = unsafe {
        xrt_sys::xclOpen(device_id, std::ffi::CStr::from_bytes_with_nul_unchecked(b"".as_ref()), xrt_sys::XCL_DEVICE_MODE::XCL_DEVICE_FPGA as u32)
    };
    
    println!("[TEST] Device opened: {:?}", xcl_device);
    
    // Load bitstream
    let ret = unsafe {
        xrt_sys::xclLoadXclBin(xcl_device, bitstream.as_ptr() as *const _ as *mut xrt_sys::axlf)
    };
    println!("[TEST] xclLoadXclBin returned: {}", ret);
    
    // Allocate output buffer (10 integers)
    let size = 10;
    let buf_size_bytes = size * 4;
    let output_handle = unsafe {
        xrt_sys::xclAllocBO(xcl_device, buf_size_bytes as u64, 1, 4)  // domain=1, bank=4
    };
    println!("[TEST] Allocated output buffer: handle={}", output_handle);
    
    // Get buffer map
    let output_ptr = unsafe {
        xrt_sys::xclMapBO(xcl_device, output_handle, true) as *mut i32
    };
    println!("[TEST] Mapped output buffer: {:p}", output_ptr);
    
    // Clear output buffer
    unsafe {
        std::ptr::write_bytes(output_ptr, 0, size);
    }
    println!("[TEST] Cleared output buffer");
    
    // Sync buffer to device
    let ret = unsafe {
        xrt_sys::xclSyncBO(xcl_device, output_handle, xrt_sys::XCL_BO_SYNC_DIRECTION::XCL_BO_SYNC_BO_TO_DEVICE, buf_size_bytes as u64, 0)
    };
    println!("[TEST] xclSyncBO TO_DEVICE returned: {}", ret);
    
    // Find CU
    let mut cu_index = -1i32;
    for cu_id in 0..4 {
        let mut name = [0u8; 128];
        let ret = unsafe {
            xrt_sys::xclGetComputeUnitInfo(xcl_device, cu_id, name.as_mut_ptr() as *mut i8)
        };
        if ret == 0 {
            cu_index = cu_id;
            println!("[TEST] Found CU at index: {}", cu_id);
            break;
        }
    }
    
    if cu_index < 0 {
        println!("[ERROR] No CU found!");
        return;
    }
    
    // Execute kernel
    println!("[TEST] Executing kernel...");
    
    // Get CU base address
    let cu_base = unsafe {
        xrt_sys::xclGetAddressInfo(xcl_device, cu_index as u32)
    };
    println!("[TEST] CU base address: {}", cu_base);
    
    // Write output pointer and size
    let output_addr = unsafe { xrt_sys::xclGetBOProperties(xcl_device, output_handle, std::ptr::null_mut()) } as u64;
    println!("[TEST] Output buffer physical address: 0x{:x}", output_addr);
    
    // Register writes for parameter passing
    unsafe {
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x10, output_addr as u32);  // output[0:31]
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x14, (output_addr >> 32) as u32);  // output[32:63]
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x1C, size as u32);  // size
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x00, 0x01);  // AP_START
    }
    
    println!("[TEST] Registers written, waiting for AP_DONE...");
    
    // Poll for AP_DONE
    let mut done = false;
    for i in 0..10_000_000 {
        let status = unsafe { xrt_sys::xclRegRead(xcl_device, cu_base + 0x00) };
        if (status & 0x02) != 0 {
            done = true;
            println!("[TEST] AP_DONE asserted at iteration {}", i);
            break;
        }
        if i % 1_000_000 == 0 && i > 0 {
            println!("[TEST] Poll iter {} - Status: 0x{:08x}", i, status);
        }
    }
    
    if !done {
        println!("[ERROR] Kernel did not complete (timeout)");
    }
    
    // Sync buffer from device
    let ret = unsafe {
        xrt_sys::xclSyncBO(xcl_device, output_handle, xrt_sys::XCL_BO_SYNC_DIRECTION::XCL_BO_SYNC_BO_FROM_DEVICE, buf_size_bytes as u64, 0)
    };
    println!("[TEST] xclSyncBO FROM_DEVICE returned: {}", ret);
    
    // Read results
    let results: &[i32] = unsafe { std::slice::from_raw_parts(output_ptr, size) };
    println!("[TEST] Output buffer: {:?}", results);
    
    // Verify results
    let expected = vec![42; size];
    if results == expected.as_slice() {
        println!("[SUCCESS] Kernel wrote constant 42 to all elements!");
    } else {
        println!("[FAILURE] Expected [42, 42, ...], got {:?}", results);
    }
    
    // Cleanup
    unsafe {
        xrt_sys::xclUnmapBO(xcl_device, output_handle, output_ptr as *mut libc::c_void);
        xrt_sys::xclFreeBO(xcl_device, output_handle);
        xrt_sys::xclClose(xcl_device);
    }
}
