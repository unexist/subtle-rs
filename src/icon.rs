///
/// @package subtle-rs
///
/// @file Xbm functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::{fmt, fs};
use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConnectionExt, ImageFormat, Pixmap};
use crate::subtle::Subtle;

#[derive(Default, Debug, Clone)]
pub(crate) struct Icon {
    /// Icon pixmap
    pub(crate) pixmap: Pixmap,
    /// Width of the icon
    pub(crate) width: u16,
    /// Height of the icon
    pub(crate) height: u16,
}

// Example:
//#define black_diamond_with_question_mark_width 9
//#define black_diamond_with_question_mark_height 9
//static unsigned char black_diamond_with_question_mark_bits[] = {
//   0x10, 0x00, 0x38, 0x00, 0x44, 0x00, 0xd6, 0x00, 0xdf, 0x01, 0xee, 0x00,
//   0x7c, 0x00, 0x28, 0x00, 0x10, 0x00 };

// See here: https://www.collabora.com/news-and-blog/blog/2016/02/16/a-programmers-view-on-digital-images-the-essentials/

/// Load icon from file
///
/// # Arguments
///
/// * `bits_per_pixel` - Number of bits per pixel
/// * `file_path` - Path to icon file
///
/// # Returns
///
/// A [`Result`] with either [`(Vec<u8>, u16, u16)`] on success or otherwise [`anyhow::Error`]
fn load_from_file(bits_per_pixel: usize, file_path: &str) -> Result<(Vec<u8>, u16, u16)> {
    let mut width = 0;
    let mut height = 0;
    let mut bits: Vec<u8> = vec![];

    for line in fs::read_to_string(file_path)?.lines() {
        // Extract width & height
        if line.contains("_width") {
            width = line.split_whitespace().last()
                .context("Failed to find width field")?
                .parse::<usize>()?;
        } else if line.contains("_height") {
            height = line.split_whitespace().last()
                .context("Failed to find height field")?
                .parse::<usize>()?;

        // Extract the pixel bytes inside {...}
        } else if line.contains("0x") {
            bits.append(&mut line.split(',').filter_map(|token| {
                // Strip "0x" and parse
                let token = token.trim_matches(|c| c == ' ' || c == ';' || c == '}')
                    .trim_start_matches("0x");

                u8::from_str_radix(token, 16).ok()
            }).collect());
        }
    }

    // Calculate display bytes and stride
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

            // Set colors if required
            let color = if 0 != bit { 255 } else { 0 };

            // Blue
            pixel[0] = color;

            // Green
            if bytes_per_pixel > 1 {
                pixel[1] = color;
            }

            // Red
            if bytes_per_pixel > 2 {
                pixel[2] = color;
            }
        }
    }

    Ok((img_data, width as u16, height as u16))
}

impl Icon {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `file_path` - Path to icon file
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Icon`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(subtle: &Subtle, file_path: &str) -> Result<Icon> {
        let conn = subtle.conn.get().unwrap();
        let default_screen = &conn.setup().roots[subtle.screen_num];

        // Find pixmap format for default depth
        let formats = &conn.setup().pixmap_formats;
        let fmt = formats.iter()
            .find(|f| f.depth == default_screen.root_depth)
            .context("Failed to find pixmap format for depth")?;
        let bits_per_pixel = fmt.bits_per_pixel as usize;

        let (img_data, width, height) =
            load_from_file(bits_per_pixel, file_path)?;

        // Create pixmap and put image
        let pixmap = conn.generate_id()?;

        conn.create_pixmap(default_screen.root_depth, pixmap, default_screen.root,
                           width, height)?.check()?;

        conn.put_image(ImageFormat::Z_PIXMAP, pixmap, subtle.draw_gc, width,
            height, 0, 0, 0, default_screen.root_depth, &img_data)?.check()?;

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
