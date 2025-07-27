///
/// @package subtle-rs
///
/// @file Font functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Char2b, ConnectionExt};
use crate::subtle::Subtle;

#[derive(Default, Debug, Clone)]
pub(crate) struct Font {
    pub(crate) fontable: u32,
    pub(crate) y: u16,
    pub(crate) height: u16,
}

impl Font {
    pub(crate) fn new(subtle: &Subtle, name: &str) -> Result<Self> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let mut font = Self {
            fontable: conn.generate_id()?,
            ..Default::default()
        };

        // Open font and calculate specs
        conn.open_font(font.fontable, name.as_ref())?.check()?;

        let reply = conn.query_font(font.fontable)?.reply()?;

        font.height = (reply.font_ascent + reply.font_descent + 2) as u16;
        font.y = (font.height - 2 + reply.font_ascent as u16) / 2;

        Ok(font)
    }

    pub(crate) fn calc_text_width(&self, subtle: &Subtle, text: &str, center: bool) -> Result<(u16, u16, u16)> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let text_char2b: Vec<Char2b> = text.as_bytes()
            .to_vec()
            .iter()
            .map(|b| Char2b {
                byte1: *b,
                byte2: *b
            }).collect();

        let reply = conn.query_text_extents(self.fontable, &text_char2b)?.reply()?;

        Ok(((if center {
            reply.overall_width - (reply.overall_left - reply.overall_right).abs()
        } else {
            reply.overall_width
        }) as u16, reply.overall_left as u16, reply.overall_right as u16))
    }

    pub(crate) fn kill(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        conn.close_font(self.fontable)?.check()?;

        Ok(())
    }
}

impl fmt::Display for Font {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(y={}, height={})",
               self.y, self.height)
    }
}
