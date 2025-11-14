#![no_main]
#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::convert::TryInto;
use core::time::Duration;

use uefi::fs::FileSystem;
use uefi::prelude::*;
use uefi::proto::console::gop::{BltOp, BltPixel, BltRegion, GraphicsOutput};
//use uefi::proto::media::fs::SimpleFileSystem;
use uefi::CString16;

#[entry]
fn main() -> Status {
    // init logger + panic handler from the uefi crate (enabled via features)
    uefi::helpers::init().unwrap();

    // small helper to log an error, stall so user can read, then return Status::ABORTED
    fn fail<E: core::fmt::Debug>(msg: &str, e: E) -> Status {
        log::error!("{}: {:?}", msg, e);
        // give the user a moment to read logs
        boot::stall(Duration::from_secs(5));
        Status::ABORTED
    }

    log::info!("UEFI BMP viewer starting");

    // === open GOP ===
    let gop_handle = match boot::get_handle_for_protocol::<GraphicsOutput>() {
        Ok(h) => h,
        Err(e) => return fail("GOP not available", e),
    };

    let mut gop = match boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
        Ok(g) => g,
        Err(e) => return fail("failed to open GOP", e),
    };

    let (screen_w, screen_h) = gop.current_mode_info().resolution();
    log::info!("screen resolution = {} x {}", screen_w, screen_h);

    // === open filesystem for the image that loaded this image (same volume) ===
    let simple_fs = match boot::get_image_file_system(boot::image_handle()) {
        Ok(fs) => fs,
        Err(e) => return fail("failed to get image filesystem", e),
    };

    let mut fs = FileSystem::new(simple_fs);

    // path to image on the same disk as the running .efi (adjust if you prefer another path)
    let path16: CString16 = CString16::try_from("\\image.bmp").unwrap();

    let bmp_bytes = match fs.read(path16.as_ref()) {
        Ok(b) => b,
        Err(e) => return fail("\\image.bmp read failed", e),
    };

    log::info!("read {} bytes from \\image.bmp", bmp_bytes.len());

    // === parse BMP into Vec<BltPixel> ===
    let (bmp_w, bmp_h, pixels) = match parse_bmp_to_blt(&bmp_bytes) {
        Ok(t) => t,
        Err(msg) => {
            log::error!("bmp parse error: {}", msg);
            boot::stall(Duration::from_secs(5));
            return Status::ABORTED;
        }
    };

    log::info!("BMP parsed: {} x {}", bmp_w, bmp_h);

    // compute top-left destination to center the image (use usize)
    let dest_x: usize = if screen_w > bmp_w {
        (screen_w - bmp_w) / 2
    } else {
        0
    };
    let dest_y: usize = if screen_h > bmp_h {
        (screen_h - bmp_h) / 2
    } else {
        0
    };

    // blit to screen (dims expect usize)
    if let Err(e) = gop.blt(BltOp::BufferToVideo {
        buffer: &pixels,
        src: BltRegion::Full,
        dest: (dest_x, dest_y),
        dims: (bmp_w, bmp_h),
    }) {
        return fail("gop.blt failed", e);
    }

    log::info!("image blitted; pausing 10 seconds so you can see it");
    boot::stall(Duration::from_secs(10));

    Status::SUCCESS
}

/// Parse the most common BMP flavors (uncompressed 24-bit and 32-bit).
/// Returns (width, height, Vec<BltPixel>) with pixels in top-to-bottom order,
/// left-to-right (row-major), suitable for `gop.blt(BufferToVideo { buffer: &pixels, ... })`.
fn parse_bmp_to_blt(data: &[u8]) -> Result<(usize, usize, Vec<BltPixel>), &'static str> {
    // need at least 54 bytes for BITMAPFILEHEADER + BITMAPINFOHEADER
    if data.len() < 54 {
        return Err("data too small for BMP headers");
    }

    // check 'BM'
    if &data[0..2] != b"BM" {
        return Err("not a BMP (missing BM header)");
    }

    let bf_off = u32::from_le_bytes(data[10..14].try_into().unwrap()) as usize;
    let bi_size = u32::from_le_bytes(data[14..18].try_into().unwrap());
    if bi_size < 40 {
        return Err("unsupported BMP info header size");
    }

    let width = i32::from_le_bytes(data[18..22].try_into().unwrap());
    let height_raw = i32::from_le_bytes(data[22..26].try_into().unwrap());
    let height = height_raw.unsigned_abs() as usize;
    let top_down = height_raw < 0;

    let planes = u16::from_le_bytes(data[26..28].try_into().unwrap());
    if planes != 1 {
        return Err("invalid BMP: planes != 1");
    }

    let bit_count = u16::from_le_bytes(data[28..30].try_into().unwrap());
    let compression = u32::from_le_bytes(data[30..34].try_into().unwrap());
    if compression != 0 {
        return Err("unsupported BMP compression (only BI_RGB)");
    }

    let width_usize = if width >= 0 {
        width as usize
    } else {
        return Err("invalid BMP width");
    };

    // bounds check offset
    if bf_off >= data.len() {
        return Err("pixel data offset out-of-bounds");
    }

    // Only support 24-bit and 32-bit uncompressed BMP here.
    let mut pixels: Vec<BltPixel> = Vec::with_capacity(width_usize * height);

    match bit_count {
        24 => {
            // Rows are padded to a 4-byte boundary
            let row_bytes_unpadded = width_usize * 3;
            let row_stride = row_bytes_unpadded.div_ceil(4) * 4;

            for row in 0..height {
                // BMP stores rows bottom-up if height positive; compute source row index:
                let src_row = if top_down { row } else { height - 1 - row };
                let row_start = bf_off + src_row * row_stride;
                if row_start + row_bytes_unpadded > data.len() {
                    return Err("BMP data truncated (24-bit)");
                }

                for x in 0..width_usize {
                    let px = row_start + x * 3;
                    let b = data[px];
                    let g = data[px + 1];
                    let r = data[px + 2];
                    let mut bp = BltPixel::new(0, 0, 0);
                    bp.red = r;
                    bp.green = g;
                    bp.blue = b;
                    pixels.push(bp);
                }
            }
        }

        32 => {
            // each pixel is 4 bytes (B,G,R,A)
            let row_stride = width_usize * 4;
            for row in 0..height {
                let src_row = if top_down { row } else { height - 1 - row };
                let row_start = bf_off + src_row * row_stride;
                if row_start + row_stride > data.len() {
                    return Err("BMP data truncated (32-bit)");
                }

                for x in 0..width_usize {
                    let px = row_start + x * 4;
                    let b = data[px];
                    let g = data[px + 1];
                    let r = data[px + 2];
                    let mut bp = BltPixel::new(0, 0, 0);
                    bp.red = r;
                    bp.green = g;
                    bp.blue = b;
                    pixels.push(bp);
                }
            }
        }

        _ => return Err("unsupported BMP bit depth (only 24 or 32)"),
    }

    Ok((width_usize, height, pixels))
}
