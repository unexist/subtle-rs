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
use anyhow::{Context, Result};
use easy_min_max::max;
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
        const FONT = 1 << 0; // Style has custom font
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

impl Side {
    pub(crate) fn inherit(&mut self, other_side: &Side, merge: bool) {
        if -1 == self.top || (merge && -1 != other_side.top) {
            self.top = other_side.top;
        }

        if -1 == self.right || (merge && -1 != other_side.right) {
            self.right = other_side.right;
        }

        if -1 == self.bottom || (merge && -1 != other_side.bottom) {
            self.bottom = other_side.bottom;
        }

        if -1 == self.left || (merge && -1 != other_side.left) {
            self.left = other_side.left;
        }
    }

    pub(crate) fn reset(&mut self, default_value: i16) {
        // Set values
        self.top = default_value;
        self.right = default_value;
        self.bottom = default_value;
        self.left = default_value;
    }
}

#[derive(Default, Debug)]
pub(crate) struct Style {
    pub(crate) flags: StyleFlags,

    pub(crate) name: String,

    pub(crate) min_width: i16,

    pub(crate) fg: i32,
    pub(crate) bg: i32,
    pub(crate) icon: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    pub(crate) left: i32,

    pub(crate) border: Side,
    pub(crate) padding: Side,
    pub(crate) margin: Side,

    pub(crate) font_id: isize,

    pub(crate) sep_width: i16,
    pub(crate) sep_string: Option<String>,
}

impl Style {
    pub(crate) fn new() -> Self {
        Style {
            fg: -1,
            bg: -1,
            icon: -1,
            top: -1,
            right: -1,
            bottom: -1,
            left: -1,
            font_id: -1,
            sep_width: -1,
            ..Self::default()
        }
    }

    pub(crate) fn calc_side(&self, side: CalcSide) -> i16 {
        match side {
            CalcSide::Top => self.border.top + self.padding.top + self.margin.top,
            CalcSide::Right => self.border.right + self.padding.right + self.margin.right,
            CalcSide::Bottom => self.border.bottom + self.padding.bottom + self.margin.bottom,
            CalcSide::Left => self.border.left + self.padding.left + self.margin.left,
        }
    }

    pub(crate) fn inherit(&mut self, other_style: &Style) {
        // Inherit values if unset
        if -1 == self.fg {
            self.fg = other_style.fg;
        }

        if -1 == self.bg {
            self.bg = other_style.bg;
        }

        if -1 == self.icon {
            self.icon = other_style.icon;
        }

        if -1 == self.top {
            self.top = other_style.top;
        }

        if -1 == self.right {
            self.right = other_style.right;
        }

        if -1 == self.bottom {
            self.bottom = other_style.bottom;
        }

        if -1 == self.left {
            self.left = other_style.left;
        }

        // Inherit unset border, padding, margin
        self.border.inherit(&other_style.border, false);
        self.padding.inherit(&other_style.padding, false);
        self.margin.inherit(&other_style.margin, false);

        // Inherit font
        if -1 == self.font_id {
            self.font_id = other_style.font_id;
        }
    }

    pub(crate) fn reset(&mut self, default_value: i32) {
        // Set values
        self.fg = default_value;
        self.bg = default_value;
        self.right = default_value;
        self.bottom = default_value;
        self.left = default_value;

        self.border.reset(default_value as i16);
        self.padding.reset(default_value as i16);
        self.margin.reset(default_value as i16);

        // Force values to prevent inheriting of 0 value from all
        self.icon = -1;
        self.font_id = -1;
        self.sep_width = -1;
    }
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(top={}, right={}, bottom={}, left={})",
               self.top, self.right, self.bottom, self.left)
    }
}

macro_rules! scale {
    ($val:expr, $div:expr, $mul:expr) => {
        if 0 < $val {
            (($val as f32 / $div as f32) * $mul as f32) as u16
        } else {
            0
        }
    };
}

fn alloc_color(conn: &RustConnection, color_str: &str, cmap: Colormap) -> Result<i32> {
    let hex_color = HexColor::parse(color_str)?;

    Ok(conn.alloc_color(cmap,
                        scale!(hex_color.r, 255, 65535),
                        scale!(hex_color.g, 255, 65535),
                        scale!(hex_color.b, 255, 65535))?.reply()?.pixel as i32)
}

fn parse_side(mixed_val: &MixedConfigVal, side: &mut Side) {
    match mixed_val {
        MixedConfigVal::I(val) => {
            side.top = *val as i16;
            side.right = *val as i16;
            side.left = *val as i16;
            side.bottom = *val as i16;
        },
        MixedConfigVal::VI(val) => {
            match val.len() {
                2 => {
                    side.top = val[0] as i16;
                    side.right = val[1] as i16;
                    side.left = val[1] as i16;
                    side.bottom = val[0] as i16;
                },
                3 => {
                    side.top = val[0] as i16;
                    side.right = val[1] as i16;
                    side.left = val[1] as i16;
                    side.bottom = val[2] as i16;
                }
                4 => {
                    side.top = val[0] as i16;
                    side.right = val[1] as i16;
                    side.left = val[2] as i16;
                    side.bottom = val[3] as i16;
                }
                _ => warn!("Too many values for style"),
            }
        }
        _ => warn!("Invalid type for style"),
    }
}

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    for style_values in config.styles.iter() {
        let style: &mut Style;

        if let Some(MixedConfigVal::S(kind))  = style_values.get("kind") {
            match kind.as_str() {
                "all" => {
                    style = &mut subtle.all_style;
                }
                "clients" => {
                    // We exploit some unused style variables here:
                    // border-top <-> client border
                    // border-right <-> title length
                    // margin <-> client gap
                    // padding <-> client strut

                    // Set border color and width
                    if let Some(MixedConfigVal::S(color_str)) = style_values.get("active") {
                        subtle.clients_style.fg = alloc_color(conn, color_str, default_screen.default_colormap)?;
                    }

                    if let Some(MixedConfigVal::S(color_str)) = style_values.get("inactive") {
                        subtle.clients_style.bg = alloc_color(conn, color_str, default_screen.default_colormap)?;
                    }

                    if let Some(MixedConfigVal::I(width)) = style_values.get("border_width") {
                        subtle.clients_style.border.top = *width as i16;
                    }

                    // Set client strut
                    if let Some(val) = style_values.get("strut") {
                        parse_side(val, &mut subtle.clients_style.padding);
                    }

                    style = &mut subtle.clients_style;
                },
                "title" => {
                    if let Some(MixedConfigVal::I(width)) = style_values.get("title_width") {
                        subtle.title_width = *width as u16;
                    }

                    style = &mut subtle.title_style;
                }
                _ => {
                    warn!("Unknown style name: {}", kind);
                    continue;
                },
            }

            // Common values of all styles
            if let Some(MixedConfigVal::S(color_str)) = style_values.get("fg") {
                style.fg = alloc_color(conn, color_str, default_screen.default_colormap)?;
            }

            if let Some(MixedConfigVal::S(color_str)) = style_values.get("bg") {
                style.bg = alloc_color(conn, color_str, default_screen.default_colormap)?;
            }

            if let Some(margin) = style_values.get("margin") {
                parse_side(margin, &mut style.margin);
            }

            if let Some(padding) = style_values.get("padding") {
                parse_side(padding, &mut style.padding);
            }

            if let Some(MixedConfigVal::S(font_name)) = style_values.get("font") {
                let font = Font::new(conn, font_name)?;

                // Update panel height
                let height = (style.calc_side(CalcSide::Top)
                    + style.calc_side(CalcSide::Bottom)) as u16 + font.height;

                subtle.panel_height = max!(subtle.panel_height, height);

                style.font_id = subtle.fonts.len() as isize;
                style.flags.insert(StyleFlags::FONT);

                subtle.fonts.push(font);
            }
        }
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn update(subtle: &mut Subtle) {
    // Inherit styles
    subtle.views_style.inherit(&subtle.all_style);
    subtle.title_style.inherit(&subtle.all_style);
    subtle.panels_style.inherit(&subtle.all_style);
    subtle.top_panel_style.inherit(&subtle.all_style);
    subtle.bottom_panel_style.inherit(&subtle.all_style);
    // TODO tray

    // Check fonts
    if !subtle.title_style.flags.contains(StyleFlags::FONT) {}

    debug!("{}", function_name!());
}
