#![no_main]
#![no_std]

use core::time::Duration;
use log::{error, info};
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;

#[entry]
fn main() -> Status {
    // initialise helper plumbing (logger + panic handler hooks provided by the uefi crate features)
    uefi::helpers::init().unwrap();

    // Try to silence the watchdog (optional, ignore error)
    let _ = boot::set_watchdog_timer(0, 0, None);

    info!("UEFI GOP demo starting");

    // 1) find a handle implementing the Graphics Output Protocol (GOP)
    let gop_handle = match boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(h) => h,
        Err(_) => {
            error!("GraphicsOutput (GOP) not available on this system");
            return Status::UNSUPPORTED;
        }
    };

    // 2) open the protocol exclusively (ScopedProtocol ensures closure on drop)
    let mut gop = match boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
        Ok(g) => g,
        Err(e) => {
            error!("failed to open GOP: {:?}", e);
            return Status::DEVICE_ERROR;
        }
    };

    // 3) get direct access to the frame buffer
    let mut fb = gop.frame_buffer();
    let fb_size = fb.size(); // bytes

    info!("framebuffer size = {} bytes", fb_size);

    // 4) write a simple pattern across the framebuffer.
    //    We write in 4-byte pixel elements (common UEFI modes use 32 bits per pixel).
    //    This is an unsafe operation because we must obey stride/pixel format; it's OK here
    //    for a demonstration where we fill the whole buffer with a visible pattern.
    unsafe {
        // iterate 4 bytes at a time
        let mut pixel_index: usize = 0;
        while pixel_index + 3 < fb_size {
            // create a simple color pattern (blue, green, red, alpha/reserved)
            let b = ((pixel_index / 4) & 0xFF) as u8;
            let g = (((pixel_index / 4) >> 8) & 0xFF) as u8;
            let r = (((pixel_index / 4) >> 16) & 0xFF) as u8;
            let a = 0u8; // reserved / alpha byte

            fb.write_byte(pixel_index, b);
            fb.write_byte(pixel_index + 1, g);
            fb.write_byte(pixel_index + 2, r);
            fb.write_byte(pixel_index + 3, a);

            pixel_index += 4;
        }
    }

    info!("framebuffer written; pausing so you can see the result");

    // wait 5 seconds (UEFI stall takes microseconds)
    boot::stall(Duration::from_secs(5));

    Status::SUCCESS
}
