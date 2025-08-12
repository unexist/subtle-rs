///
/// @package subtle-rs
///
/// @file Xbm functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::{fmt, fs};
use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, CreateGCAux, ImageFormat, Pixmap};
use crate::subtle::Subtle;

#[derive(Default, Debug, Clone)]
pub(crate) struct Icon {
    pub(crate) pixmap: Pixmap,
    pub(crate) width: u16,
    pub(crate) height: u16,
}

/*#define black_diamond_with_question_mark_width 9
#define black_diamond_with_question_mark_height 9
static unsigned char black_diamond_with_question_mark_bits[] = {
   0x10, 0x00, 0x38, 0x00, 0x44, 0x00, 0xd6, 0x00, 0xdf, 0x01, 0xee, 0x00,
   0x7c, 0x00, 0x28, 0x00, 0x10, 0x00 };*/

fn load_from_file(subtle: &Subtle, bits_per_pixel: usize, filename: &str) -> Result<(Vec<u8>, u16, u16)> {
    let text = fs::read_to_string(filename)?;

    // Extract width & height
    let width= text
        .lines()
        .find(|l| l.contains("_width"))
        .and_then(|l| l.split_whitespace().last())
        .unwrap()
        .parse::<usize>()?;

    let height= text
        .lines()
        .find(|l| l.contains("_height"))
        .and_then(|l| l.split_whitespace().last())
        .unwrap()
        .parse::<usize>()?;

    // Extract the pixel bytes inside {...}
    let start = text.find('{').unwrap() + 1;
    let end = text.find('}').unwrap();
    let hex_data = &text[start..end];

    let bits: Vec<u8> = hex_data
        .split(',')
        .filter_map(|token| {
            let token = token.trim();
            if token.is_empty() {
                None
            } else {
                // Strip "0x" and parse
                let t = token.trim_start_matches("0x");
                Some(u8::from_str_radix(t, 16).unwrap())
            }
        })
        .collect();

    // Get display info
    let conn = subtle.conn.get().unwrap();

    let bytes_per_pixel = bits_per_pixel / 8;
    let stride = ((width * bits_per_pixel + 31) / 32) * 4;

    // Allocate RGB buffer
    let mut img_data = vec![0u8; height * stride];

    for y in 0..height {
        for x in 0..width {
            let byte_index = y * ((width + 7) / 8) + (x / 8);
            let bit = (bits[byte_index] >> (x % 8)) & 1;

            let pixel_offset = y * stride + x * bytes_per_pixel;
            let pixel = &mut img_data[pixel_offset..];

            if 0 != bit {
                // Black
                pixel[0] = 0; // B

                if bytes_per_pixel > 1 { pixel[1] = 0; } // G
                if bytes_per_pixel > 2 { pixel[2] = 0; } // R
            } else {
                // White
                pixel[0] = 255; // B

                if bytes_per_pixel > 1 { pixel[1] = 255; } // G
                if bytes_per_pixel > 2 { pixel[2] = 255; } // R
            }
        }
    }

    Ok((img_data, width as u16, height as u16))
}

impl Icon {
    pub(crate) fn new(subtle: &Subtle, file_path: &str) -> Result<Icon> {
        let conn = subtle.conn.get().unwrap();
        let default_screen = &conn.setup().roots[subtle.screen_num];

        // Find pixmap format for default depth
        let formats = &conn.setup().pixmap_formats;
        let fmt = formats.iter()
            .find(|f| f.depth == default_screen.root_depth)
            .context("Failed to find pixmap format for depth")?;
        let bits_per_pixel = fmt.bits_per_pixel as usize;

        let (img_data, width, height) = load_from_file(subtle,
                                                       bits_per_pixel, file_path)?;

        let pixmap = conn.generate_id()?;

        conn.create_pixmap(default_screen.root_depth, pixmap, default_screen.root,
                           width, height)?.check()?;

        let icon_gc = conn.generate_id()?;

        conn.create_gc(icon_gc, pixmap, &CreateGCAux::default())?.check()?;

        conn.put_image(ImageFormat::Z_PIXMAP, pixmap, icon_gc, width,
            height, 0, 0, 0, default_screen.root_depth, &img_data)?.check()?;

        conn.free_gc(icon_gc)?.check()?;

        Ok(Self {
            pixmap,
            width,
            height,
        })
    }
}

impl fmt::Display for Icon {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(pixmap={}, width={:?}, height={:?})", self.pixmap, self.width, self.height)
    }
}
