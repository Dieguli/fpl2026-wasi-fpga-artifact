// use crate::xrt::{Device, Buffer, xclBOProperties, xclRegWrite, xclRegRead, xclGetBOProperties, xclLoadXclBin};
// use crate::error::{WasiFpgaError, Result};
// use std::collections::HashMap;

// pub struct BufferManager {
//     device: Device,
//     buffers: HashMap<i32, Buffer>,
//     next_id: i32,
// }

// impl BufferManager {
//     pub fn new(device_index: u32) -> Result<Self> {
//         let device = Device::open(device_index)?;
//         Ok(BufferManager { device, buffers: HashMap::new(), next_id: 1 })
//     }
    
//     pub fn load_xclbin(&mut self, filename: &str) -> Result<()> {
//         let bin = std::fs::read(filename).map_err(|e| WasiFpgaError::XrtError(format!("Error: {}", e)))?;
//         unsafe { xclLoadXclBin(self.device.handle(), bin.as_ptr() as *const _); }
//         eprintln!("[wasi_fpga]  FPGA programada con nuevo bitstream.");
//         Ok(())
//     }

//     pub fn alloc(&mut self, size: usize) -> Result<i32> {
//         let buffer = Buffer::alloc(self.device.handle(), size)?;
//         let id = self.next_id;
//         self.buffers.insert(id, buffer);
//         self.next_id += 1;
//         Ok(id)
//     }

//     pub fn write(&mut self, buffer_id: i32, data: &[u8]) -> Result<()> {
//         self.buffers.get(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?.write(data, 0)
//     }

//     pub fn read(&mut self, buffer_id: i32, length: usize) -> Result<Vec<u8>> {
//         self.buffers.get(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?.read(length, 0)
//     }

//     pub fn free(&mut self, buffer_id: i32) -> Result<()> {
//         self.buffers.remove(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;
//         Ok(())
//     }

//     pub fn execute_kernel(&mut self, _name: &str, buffer_ids: Vec<i32>) -> Result<()> {
//         let h = self.device.handle();

//         eprintln!("[wasi_fpga] === KERNEL EXECUTION START ===");

//         // --- 1. Sync input buffers TO device and collect physical addresses ---
//         eprintln!("[wasi_fpga] Step 1: Syncing buffers to device...");
//         let mut paddrs: Vec<u64> = Vec::new();
//         for &id in &buffer_ids {
//             let buf = self.buffers.get(&id).ok_or(WasiFpgaError::InvalidBufferId(id))?;
//             eprintln!("[wasi_fpga]   Syncing buffer {} ({} bytes)", id, buf.size());
//             buf.sync_to_device()?;
//             unsafe {
//                 let mut props = std::mem::zeroed::<xclBOProperties>();
//                 xclGetBOProperties(h, buf.handle(), &mut props);
//                 paddrs.push(props.paddr);
//                 eprintln!("[wasi_fpga]   Buffer {} physical address: 0x{:016X}", id, props.paddr);
//             }
//         }
        
//         // Small delay to ensure synchronization completes
//         eprintln!("[wasi_fpga] Waiting for DMA sync to complete...");
//         std::thread::sleep(std::time::Duration::from_millis(10));
//         eprintln!("[wasi_fpga] ✓ All buffers synced");

//         unsafe {
//             // Register offsets for kernel ABI: A=0x10, B=0x1C, RES=0x28, SIZE=0x34
//             let reg_offsets: [u32; 3] = [0x10, 0x1C, 0x28];
//             let size_reg_offset: u32 = 0x34; 
//             let mut chosen_cu: Option<u32> = None;

//             eprintln!("[wasi_fpga] Step 2: Finding active Compute Unit (CU)...");
//             // --- 2. Find active Compute Unit (CU) ---
//             for test_cu in 0..4u32 {
//                 let mut ok = true;
//                 for (i, &paddr) in paddrs.iter().take(3).enumerate() {
//                     let reg = reg_offsets[i];
//                     let low = (paddr & 0xFFFFFFFF) as u32;
//                     let high = (paddr >> 32) as u32;
//                     xclRegWrite(h, test_cu, reg, low);
//                     xclRegWrite(h, test_cu, reg + 4, high);
//                     let mut rlow = 0u32;
//                     xclRegRead(h, test_cu, reg, &mut rlow);
//                     if rlow != low { 
//                         ok = false; 
//                         eprintln!("[wasi_fpga]   CU {} write test failed (wrote 0x{:08X}, read 0x{:08X})", test_cu, low, rlow);
//                     }
//                 }
//                 if ok {
//                     chosen_cu = Some(test_cu);
//                     eprintln!("[wasi_fpga] ✓ Selected CU index {} for execution", test_cu);
//                     break;
//                 }
//             }
//             let cu = chosen_cu.unwrap_or(0);

//             eprintln!("[wasi_fpga] Step 3: Programming kernel registers...");
//             // --- 3. Program kernel registers with real DMA buffer addresses ---
//             for (i, &paddr) in paddrs.iter().take(3).enumerate() {
//                 let reg = reg_offsets[i];
//                 let low = (paddr & 0xFFFFFFFF) as u32;
//                 let high = (paddr >> 32) as u32;
//                 xclRegWrite(h, cu, reg, low);
//                 xclRegWrite(h, cu, reg + 4, high);
                
//                 // Verify the writes
//                 let mut rlow = 0u32;
//                 let mut rhigh = 0u32;
//                 xclRegRead(h, cu, reg, &mut rlow);
//                 xclRegRead(h, cu, reg + 4, &mut rhigh);
//                 let read_paddr = ((rhigh as u64) << 32) | (rlow as u64);
                
//                 eprintln!("[wasi_fpga] REG 0x{:02X} (low)  = 0x{:08X} (wrote 0x{:08X}) - {}", reg, rlow, low, if rlow == low { "✓" } else { "" });
//                 eprintln!("[wasi_fpga] REG 0x{:02X} (high) = 0x{:08X} (wrote 0x{:08X}) - {}", reg+4, rhigh, high, if rhigh == high { "✓" } else { "" });
//                 eprintln!("[wasi_fpga] Full paddr: wrote 0x{:016X}, read 0x{:016X}", paddr, read_paddr);
//             }

//             // --- 4. Write size register and start kernel ---
//             eprintln!("[wasi_fpga] Step 4: Writing size parameter...");
//             let num_elements = if let Some(&first_id) = buffer_ids.first() {
//                 let buf = self.buffers.get(&first_id).unwrap();
//                 (buf.size() / 4) as u32 // int elements = bytes / sizeof(int)
//             } else { 0u32 };
            
//             eprintln!("[wasi_fpga] Buffer size: {} bytes, elements: {}", 
//                 if let Some(&first_id) = buffer_ids.first() { 
//                     self.buffers.get(&first_id).unwrap().size() 
//                 } else { 0 }, 
//                 num_elements);
            
//             // Write size parameter at offset 0x34
//             xclRegWrite(h, cu, size_reg_offset, num_elements);
//             eprintln!("[wasi_fpga] Wrote to REG 0x{:02X} (size) = 0x{:08X} ({} elements)", size_reg_offset, num_elements, num_elements);
            
//             // Small delay to ensure all register writes propagate
//             std::thread::sleep(std::time::Duration::from_millis(5));
            
//             // Verify size was written
//             let mut read_size = 0u32;
//             xclRegRead(h, cu, size_reg_offset, &mut read_size);
//             eprintln!("[wasi_fpga] Read from REG 0x{:02X} (size) = 0x{:08X} ({} elements) - {}", 
//                 size_reg_offset, read_size, read_size, if read_size == num_elements { "✓" } else { " MISMATCH!" });
            
//             if read_size != num_elements {
//                 eprintln!("[wasi_fpga]   Size mismatch: wrote {}, read {}", num_elements, read_size);
//                 return Err(WasiFpgaError::XrtError(format!("Size register write failed: wrote {}, read {}", num_elements, read_size)));
//             }

//             eprintln!("[wasi_fpga] Step 5: Starting kernel...");
//             xclRegWrite(h, cu, 0x00, 0x01); // AP_START
//             eprintln!("[wasi_fpga] Kernel started: CU={}", cu);

//             // --- 5. Poll for completion ---
//             eprintln!("[wasi_fpga] Step 6: Polling for kernel completion...");
//             let mut status = 0u32;
//             let mut iterations = 0u32;
//             let mut first_status_printed = false;
            
//             for _ in 0..10_000_000u32 {
//                 iterations += 1;
//                 xclRegRead(h, cu, 0x00, &mut status);
                
//                 // Print first few status values for debugging
//                 if iterations <= 5 || (iterations % 1000000 == 0) {
//                     eprintln!("[wasi_fpga] Poll iter {} - Status: 0x{:08X} (AP_DONE={}, AP_IDLE={}, AP_READY={})", 
//                         iterations, status,
//                         (status >> 1) & 1,  // bit 1 = AP_DONE
//                         (status >> 2) & 1,  // bit 2 = AP_IDLE  
//                         (status >> 3) & 1); // bit 3 = AP_READY
//                 }
                
//                 if (status & 0x02) != 0 { 
//                     eprintln!("[wasi_fpga] AP_DONE detected at iteration {}", iterations);
//                     break; 
//                 } // AP_DONE
//             }
//             eprintln!("[wasi_fpga] Kernel done after {} iterations. Status=0x{:X} (AP_DONE={}, AP_IDLE={}, AP_READY={})", 
//                 iterations, status,
//                 (status >> 1) & 1,
//                 (status >> 2) & 1,
//                 (status >> 3) & 1);

//             if (status & 0x02) == 0 {
//                 eprintln!("[wasi_fpga]  ERROR: Kernel did not complete (timeout)");
//                 return Err(WasiFpgaError::XrtError("Kernel execution timeout".to_string()));
//             }
//         }

//         // --- 6. Sync output buffer FROM device ---
//         eprintln!("[wasi_fpga] Step 7: Syncing output buffer from device...");
//         if let Some(&output_id) = buffer_ids.last() {
//             let buf = self.buffers.get(&output_id)
//                 .ok_or(WasiFpgaError::InvalidBufferId(output_id))?;
//             eprintln!("[wasi_fpga] Syncing output buffer {} from device", output_id);
//             buf.sync_from_device()?;
//             eprintln!("[wasi_fpga] ✓ Output buffer {} synced from device", output_id);
//         }

//         eprintln!("[wasi_fpga] === KERNEL EXECUTION COMPLETE ===");
//         Ok(())
//     }
// }

use crate::xrt::{
    Device, Buffer, XclUuid, xclBOProperties,
    xclRegWrite, xclRegRead, xclGetBOProperties, xclLoadXclBin,
    xclOpenContext, xclCloseContext, extract_xclbin_uuid,
};
use crate::error::{WasiFpgaError, Result};
use std::collections::HashMap;

pub struct BufferManager {
    device: Device,
    buffers: HashMap<i32, Buffer>,
    next_id: i32,
    /// UUID of the loaded xclbin, needed for xclCloseContext on cleanup
    xclbin_uuid: Option<XclUuid>,
}

impl BufferManager {
    pub fn new(device_index: u32) -> Result<Self> {
        let device = Device::open(device_index)?;
        Ok(BufferManager { device, buffers: HashMap::new(), next_id: 1, xclbin_uuid: None })
    }

    pub fn load_xclbin(&mut self, filename: &str) -> Result<()> {
        let bin = std::fs::read(filename).map_err(|e| WasiFpgaError::XrtError(format!("Error: {}", e)))?;
        eprintln!("[wasi_fpga] Loading xclbin: {} ({} bytes)", filename, bin.len());

        // Extract UUID from axlf header BEFORE loading (we need it for xclOpenContext)
        let uuid = extract_xclbin_uuid(&bin).ok_or_else(|| {
            WasiFpgaError::XrtError("Failed to extract UUID from xclbin (invalid format?)".to_string())
        })?;
        eprintln!("[wasi_fpga] xclbin UUID: {:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            uuid[0], uuid[1], uuid[2], uuid[3],
            uuid[4], uuid[5], uuid[6], uuid[7],
            uuid[8], uuid[9], uuid[10], uuid[11],
            uuid[12], uuid[13], uuid[14], uuid[15]);

        // Load xclbin into device (programs FPGA)
        let ret = unsafe { xclLoadXclBin(self.device.handle(), bin.as_ptr() as *const _) };
        if ret != 0 {
            eprintln!("[wasi_fpga] ERROR: xclLoadXclBin returned {}", ret);
            return Err(WasiFpgaError::XrtError(format!("xclLoadXclBin failed: {}", ret)));
        }
        eprintln!("[wasi_fpga] FPGA programmed with new bitstream.");

        // CRITICAL FIX: Open exclusive CU context
        // Without this, xclRegWrite/xclRegRead may appear to work (register values
        // read back correctly) but the CU's AXI master port is NOT configured by
        // the ZOCL driver/SMMU — kernel DMA writes to DDR are silently dropped.
        // See: https://xilinx.github.io/XRT/2020.2/html/xrt_kernel_executions.html
        let ret = unsafe { xclOpenContext(self.device.handle(), uuid.as_ptr(), 0, true) };
        if ret != 0 {
            eprintln!("[wasi_fpga] ERROR: xclOpenContext returned {} (CU 0, exclusive)", ret);
            return Err(WasiFpgaError::XrtError(format!("xclOpenContext failed: {}", ret)));
        }
        eprintln!("[wasi_fpga] CU context opened (exclusive, ipIndex=0)");

        self.xclbin_uuid = Some(uuid);
        Ok(())
    }

    pub fn alloc(&mut self, size: usize) -> Result<i32> {
        let buffer = Buffer::alloc(self.device.handle(), size)?;
        let id = self.next_id;
        self.buffers.insert(id, buffer);
        self.next_id += 1;
        Ok(id)
    }

    pub fn write(&mut self, buffer_id: i32, data: &[u8]) -> Result<()> {
        self.buffers.get(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?.write(data, 0)
    }

    pub fn read(&mut self, buffer_id: i32, length: usize) -> Result<Vec<u8>> {
        self.buffers.get(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?.read(length, 0)
    }

    pub fn free(&mut self, buffer_id: i32) -> Result<()> {
        self.buffers.remove(&buffer_id).ok_or(WasiFpgaError::InvalidBufferId(buffer_id))?;
        Ok(())
    }

    pub fn execute_kernel(&mut self, _name: &str, buffer_ids: Vec<i32>) -> Result<()> {
        let h = self.device.handle();
        // CU index = 0 (the one we opened context for in load_xclbin)
        let cu: u32 = 0;

        // --- 1. Sync input buffers TO device and collect physical addresses ---
        let mut paddrs: Vec<u64> = Vec::new();
        for &id in &buffer_ids {
            let buf = self.buffers.get(&id).ok_or(WasiFpgaError::InvalidBufferId(id))?;
            buf.sync_to_device()?;
            unsafe {
                let mut props = std::mem::zeroed::<xclBOProperties>();
                xclGetBOProperties(h, buf.handle(), &mut props);
                paddrs.push(props.paddr);
                eprintln!("[wasi_fpga] Buffer {} paddr=0x{:016X} size={}", id, props.paddr, buf.size());
            }
        }

        // --- DIAGNOSTIC: Readback input buffers to verify data is in DDR ---
        for &id in &buffer_ids {
            let buf = self.buffers.get(&id).ok_or(WasiFpgaError::InvalidBufferId(id))?;
            if let Ok(data) = buf.read(std::cmp::min(buf.size(), 40), 0) {
                let ints: Vec<i32> = data.chunks_exact(4)
                    .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                eprintln!("[wasi_fpga] DIAG buffer {} readback (first {} ints): {:?}",
                    id, ints.len(), ints);
            }
        }

        unsafe {
            // Register offsets for kernel ABI: A=0x10, B=0x1C, RES=0x28
            let reg_offsets: [u32; 3] = [0x10, 0x1C, 0x28];

            // --- 2. Program kernel registers with buffer physical addresses ---
            for (i, &paddr) in paddrs.iter().take(3).enumerate() {
                let reg = reg_offsets[i];
                let low = (paddr & 0xFFFFFFFF) as u32;
                let high = (paddr >> 32) as u32;
                let ret_lo = xclRegWrite(h, cu, reg, low);
                let ret_hi = xclRegWrite(h, cu, reg + 4, high);
                eprintln!("[wasi_fpga] REG 0x{:02X} = paddr 0x{:016X} (ret={}/{})", reg, paddr, ret_lo, ret_hi);
            }

            // --- 3. Write size register and start kernel ---
            // Register map (from HLS s_axilite auto-allocation):
            //   0x00: AP_CTRL    0x10: a    0x1C: b    0x28: res    0x34: size
            let num_elements = if let Some(&first_id) = buffer_ids.first() {
                let buf = self.buffers.get(&first_id).unwrap();
                (buf.size() / 4) as u32
            } else { 0u32 };
            xclRegWrite(h, cu, 0x34, num_elements);
            // Readback to verify register offset is correct
            let mut size_readback = 0u32;
            xclRegRead(h, cu, 0x34, &mut size_readback);
            eprintln!("[wasi_fpga] REG 0x34 (size) = {} (readback={})", num_elements, size_readback);
            if size_readback != num_elements {
                eprintln!("[wasi_fpga] WARNING: size readback mismatch! Register offset 0x34 may be WRONG for this xclbin.");
                eprintln!("[wasi_fpga]   Run: xclbinutil --input your.xclbin --dump-section EMBEDDED_METADATA:RAW:metadata.xml");
                eprintln!("[wasi_fpga]   Then: cat metadata.xml | grep -A5 'arg name'  to find correct offsets");
            }

            xclRegWrite(h, cu, 0x00, 0x01); // AP_START
            eprintln!("[wasi_fpga] Kernel started: CU={}", cu);

            // --- 4. Poll for completion ---
            let mut status = 0u32;
            for _ in 0..10_000_000u32 {
                xclRegRead(h, cu, 0x00, &mut status);
                if (status & 0x02) != 0 { break; } // AP_DONE
            }
            eprintln!("[wasi_fpga] Kernel done. Status=0x{:X}", status);

            if (status & 0x02) == 0 {
                eprintln!("[wasi_fpga] WARNING: Kernel did not complete (timeout)");
                return Err(WasiFpgaError::XrtError("Kernel execution timeout".to_string()));
            }
        }

        // --- 5. Sync ALL buffers FROM device and show diagnostics ---
        for &id in &buffer_ids {
            let buf = self.buffers.get(&id)
                .ok_or(WasiFpgaError::InvalidBufferId(id))?;
            buf.sync_from_device()?;
            if let Ok(data) = buf.read(std::cmp::min(buf.size(), 40), 0) {
                let ints: Vec<i32> = data.chunks_exact(4)
                    .map(|b| i32::from_le_bytes(b.try_into().unwrap()))
                    .collect();
                eprintln!("[wasi_fpga] DIAG buffer {} AFTER kernel (first {} ints): {:?}",
                    id, ints.len(), ints);
            }
        }
        eprintln!("[wasi_fpga] All buffers synced from device");

        Ok(())
    }
}

impl Drop for BufferManager {
    fn drop(&mut self) {
        // 1. Free all buffers first (while device is still open)
        self.buffers.clear();

        // 2. Close CU context (before device close)
        if let Some(uuid) = &self.xclbin_uuid {
            unsafe {
                xclCloseContext(self.device.handle(), uuid.as_ptr(), 0);
            }
            eprintln!("[wasi_fpga] CU context closed");
        }

        // 3. Device drops automatically after this (xclClose)
    }
}