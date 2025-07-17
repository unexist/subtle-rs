///
/// @package subtle-rs
///
/// @file Style functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::bitflags;
use anyhow::Result;
use hex_color::HexColor;
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Colormap, ConnectionExt};
use x11rb::rust_connection::RustConnection;
use crate::config::{Config, MixedConfigVal};
use crate::font::Font;
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct StyleFlags: u32 {
        const FONT= 1 << 0; // Style has custom font
        const SEPARATOR = 1 << 1; // Style has separator
    }
}

pub(crate) enum CalcSide {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Default, Debug, Copy, Clone)]
pub(crate) struct Side {
    pub(crate) top: i16,
    pub(crate) right: i16,
    pub(crate) bottom: i16,
    pub(crate) left: i16,
}

#[derive(Default, Debug, Clone)]
pub(crate) struct Separator {
    pub(crate) string: String,
    pub(crate) width: u16,
}

#[derive(Default, Debug)]
pub(crate) struct Style {
    pub(crate) flags: StyleFlags,

    pub(crate) name: String,

    pub(crate) min_width: i16,

    pub(crate) fg: u32,
    pub(crate) bg: u32,
    pub(crate) icon: u32,
    pub(crate) top: u32,
    pub(crate) right: u32,
    pub(crate) bottom: u32,
    pub(crate) left: u32,

    pub(crate) border: Side,
    pub(crate) padding: Side,
    pub(crate) margin: Side,

    pub(crate) font: Font,
    pub(crate) separator: Separator,
}

impl Style {
    pub(crate) fn calc_side(&self, side: CalcSide) -> i16 {
        match side {
            CalcSide::Top => self.border.top + self.padding.top + self.margin.top,
            CalcSide::Right => self.border.right + self.padding.right + self.margin.right,
            CalcSide::Bottom => self.border.bottom + self.padding.bottom + self.margin.bottom,
            CalcSide::Left => self.border.left + self.padding.left + self.margin.left,
        }
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(top={}, right={}, bottom={}, left={})",
               self.top, self.right, self.bottom, self.left)
    }
}

impl fmt::Display for Separator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(string={}, width={})", self.string, self.width)
    }
}

fn parse_color(conn: &RustConnection, color: &str, cmap: Colormap) -> Result<u32> {
    let hex_color = HexColor::parse(color)?;

    Ok(conn.alloc_color(cmap, hex_color.r as u16, hex_color.g as u16, hex_color.b as u16)?.reply()?.pixel)
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];

    for (name, values) in config.styles.iter() {
        match name.as_str() {
            "clients" => {
                if let Some(MixedConfigVal::S(color)) = values.get("active") {
                    subtle.styles.clients.fg = parse_color(conn, color, screen.default_colormap)?;
                }

                if let Some(MixedConfigVal::S(color)) = values.get("inactive") {
                    subtle.styles.clients.bg = parse_color(conn, color, screen.default_colormap)?;
                }

                if let Some(MixedConfigVal::I(bw)) = values.get("border_width") {
                    subtle.styles.clients.border.top = *bw as i16;
                }
            }
            _ => warn!("Unhandled style: {}", name),
        }
    }

    debug!("{}", function_name!());

    Ok(())
}
