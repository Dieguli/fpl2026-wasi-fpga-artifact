use std::fs;

fn main() {
    let xclbin_path = std::env::args().nth(1).unwrap_or_else(|| "test_register_return.xclbin".to_string());
    
    println!("[TEST] Testing kernel with return value via s_axilite...");
    
    // Load XClbin
    let bitstream = fs::read(&xclbin_path).expect("Failed to read xclbin");
    println!("[TEST] Loaded XClbin: {} bytes", bitstream.len());
    
    // Initialize XRT
    let device_id = 0;
    let xcl_device = unsafe {
        xrt_sys::xclOpen(device_id, std::ffi::CStr::from_bytes_with_nul_unchecked(b"".as_ref()), xrt_sys::XCL_DEVICE_MODE::XCL_DEVICE_FPGA as u32)
    };
    println!("[TEST] Device opened");
    
    // Load bitstream
    let ret = unsafe {
        xrt_sys::xclLoadXclBin(xcl_device, bitstream.as_ptr() as *const _ as *mut xrt_sys::axlf)
    };
    println!("[TEST] xclLoadXclBin returned: {}", ret);
    
    // Allocate input buffer (10 integers: 1,2,3,...,10)
    let size = 10;
    let buf_size_bytes = size * 4;
    let input_handle = unsafe {
        xrt_sys::xclAllocBO(xcl_device, buf_size_bytes as u64, 1, 4)
    };
    println!("[TEST] Allocated input buffer: handle={}", input_handle);
    
    // Map and fill input buffer
    let input_ptr = unsafe {
        xrt_sys::xclMapBO(xcl_device, input_handle, true) as *mut i32
    };
    unsafe {
        for i in 0..size {
            *input_ptr.add(i) = (i as i32 + 1);
        }
    }
    println!("[TEST] Filled input buffer: [1,2,3,...,10]");
    
    // Sync input to device
    unsafe {
        xrt_sys::xclSyncBO(xcl_device, input_handle, xrt_sys::XCL_BO_SYNC_DIRECTION::XCL_BO_SYNC_BO_TO_DEVICE, buf_size_bytes as u64, 0);
    }
    println!("[TEST] Synced input buffer to device");
    
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
    let cu_base = unsafe {
        xrt_sys::xclGetAddressInfo(xcl_device, cu_index as u32)
    };
    println!("[TEST] CU base address: {}", cu_base);
    
    // Get input buffer physical address
    let input_addr = unsafe { xrt_sys::xclGetBOProperties(xcl_device, input_handle, std::ptr::null_mut()) } as u64;
    println!("[TEST] Input buffer physical address: 0x{:x}", input_addr);
    
    // Write input pointer and size
    unsafe {
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x10, input_addr as u32);  // input[0:31]
        xrt_sys::xclRegWrite(xcl_device, cu_base + 0x14, (input_addr >> 32) as u32);  // input[32:63]
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
    
    // Read return value from register 0x3C (or 0x08 depending on AXI spec)
    let return_value = unsafe { 
        let mut val = 0u32;
        xrt_sys::xclRegRead(xcl_device, cu_base + 0x08, &mut val);
        val
    };
    
    println!("[TEST] Return value from kernel: 0x{:08x} ({} decimal)", return_value, return_value);
    println!("[TEST] Expected: sum of [1..10] = 55");
    
    if return_value == 55 {
        println!("[SUCCESS] Kernel return value via s_axilite works!");
    } else {
        println!("[FAILURE] Got {} instead of 55", return_value);
    }
    
    // Cleanup
    unsafe {
        xrt_sys::xclUnmapBO(xcl_device, input_handle, input_ptr as *mut libc::c_void);
        xrt_sys::xclFreeBO(xcl_device, input_handle);
        xrt_sys::xclClose(xcl_device);
    }
}
