#![no_main]
#![no_std]

use log::info;
use uefi::boot::{self, MemoryType, SearchType}; // MemoryType lives in uefi::boot
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::{Identify, Result};

#[entry]
fn main() -> Status {
    // initialize logger/panic handler (requires corresponding features in Cargo.toml)
    uefi::helpers::init().unwrap();

    // run our demo; map any uefi::Error -> Status using status()
    match print_gop_info_and_pool_demo() {
        Ok(()) => Status::SUCCESS,
        Err(e) => {
            info!("Error: {:?}", e);
            e.status() // convert uefi::Error -> Status
        }
    }
}

fn print_gop_info_and_pool_demo() -> Result {
    // --- DEMO: allocate_pool & free_pool ---
    {
        let size: usize = 64;
        let mem = boot::allocate_pool(MemoryType::LOADER_DATA, size)?;
        info!("Allocated {} bytes at {:p}", size, mem.as_ptr());

        // Safety: allocate_pool returns uninitialized memory; initialize before reading.
        unsafe {
            let buf = core::slice::from_raw_parts_mut(mem.as_ptr(), size);
            for i in 0..size {
                buf[i] = i as u8;
            }
            let print_len = core::cmp::min(16, size);
            info!(
                "First {} bytes in the pool: {:?}",
                print_len,
                &buf[..print_len]
            );
        }

        // free_pool is unsafe, call it inside an unsafe block
        unsafe {
            boot::free_pool(mem)?;
        }
        info!("Freed the allocated pool at {:p}", mem.as_ptr());
    }
    // --- end pool demo ---

    // Find all handles that support the GOP
    let handles = boot::locate_handle_buffer(SearchType::ByProtocol(&GraphicsOutput::GUID))?;

    // Iterate through handles. locate_handle_buffer gives you handles you can copy (Handle is Copy).
    for (idx, handle_ref) in handles.iter().enumerate() {
        let handle = *handle_ref; // deref the &Handle -> Handle
        info!("--- GOP device #{} ---", idx);

        // Open the protocol in exclusive mode (safe, returns ScopedProtocol)
        let gop = match boot::open_protocol_exclusive::<GraphicsOutput>(handle) {
            Ok(g) => g,
            Err(e) => {
                info!(" Failed to open GOP on handle {:?}: {:?}", handle, e);
                continue;
            }
        };

        // Query current mode information
        let current_info = gop.current_mode_info();
        let (cur_w, cur_h) = current_info.resolution();
        let cur_stride = current_info.stride();
        let cur_pixfmt = current_info.pixel_format();
        info!(
            " Current mode: {}x{} stride={} pixel_format={:?}",
            cur_w, cur_h, cur_stride, cur_pixfmt
        );

        for (mode_index, mode) in gop.modes().enumerate() {
            let mi = mode.info();
            let (w, h) = mi.resolution();
            let stride = mi.stride();
            let pixfmt = mi.pixel_format();
            info!(
                " Mode {}: {}x{} stride={} pixel_format={:?}",
                mode_index, w, h, stride, pixfmt
            );
        }
    }

    Ok(())
}
