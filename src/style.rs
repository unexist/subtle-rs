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
use crate::spacing::Spacing;
use crate::subtle::Subtle;

const DEFAULT_FONT_NAME: &str = "-*-*-*-*-*-*-14-*-*-*-*-*-*-*";

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct StyleFlags: u32 {
        const FONT = 1 << 0; // Style has custom font
        const SEPARATOR = 1 << 1; // Style has separator
    }
}

pub(crate) enum CalcSpacing {
    Top,
    Right,
    Bottom,
    Left,
    Width,
    Height,
}

#[derive(Debug)]
pub(crate) struct Style {
    pub(crate) flags: StyleFlags,

    pub(crate) min_width: i16,

    pub(crate) fg: i32,
    pub(crate) bg: i32,
    pub(crate) icon: i32,
    pub(crate) top: i32,
    pub(crate) right: i32,
    pub(crate) bottom: i32,
    pub(crate) left: i32,

    pub(crate) border: Spacing,
    pub(crate) padding: Spacing,
    pub(crate) margin: Spacing,

    pub(crate) font_id: isize,
}

impl Style {
    pub(crate) fn calc_spacing(&self, spacing: CalcSpacing) -> i16 {
        match spacing {
            CalcSpacing::Top => self.border.top + self.padding.top + self.margin.top,
            CalcSpacing::Right => self.border.right + self.padding.right + self.margin.right,
            CalcSpacing::Bottom => self.border.bottom + self.padding.bottom + self.margin.bottom,
            CalcSpacing::Left => self.border.left + self.padding.left + self.margin.left,
            CalcSpacing::Width => self.calc_spacing(CalcSpacing::Left)
                + self.calc_spacing(CalcSpacing::Right),
            CalcSpacing::Height => self.calc_spacing(CalcSpacing::Top)
                + self.calc_spacing(CalcSpacing::Bottom),
        }
    }

    pub(crate) fn inherit(&mut self, other_style: &Style) {
        // Inherit unset values
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

        // Ensure sane value for min_width
        self.min_width = max!(0, self.min_width);
    }

    pub(crate) fn reset(&mut self, default_value: i32) {
        // Set values
        self.fg = default_value;
        self.bg = default_value;
        self.top = default_value;
        self.right = default_value;
        self.bottom = default_value;
        self.left = default_value;

        self.border.reset(default_value as i16);
        self.padding.reset(default_value as i16);
        self.margin.reset(default_value as i16);

        // Force values to prevent inheriting of 0 value from all
        self.icon = -1;
        self.font_id = -1;
    }

    pub(crate) fn get_font<'a>(&self, subtle: &'a Subtle) -> Option<&'a Font> {
        if -1 != self.font_id {
            return subtle.fonts.get(self.font_id as usize);
        }

        None
    }
}

impl Default for Style {
    fn default() -> Self {
        Style {
            flags: StyleFlags::empty(),
            min_width: -1,
            fg: -1,
            bg: -1,
            icon: -1,

            top: -1,
            right: -1,
            bottom: -1,
            left: -1,

            border: Default::default(),
            padding: Default::default(),
            margin: Default::default(),

            font_id: -1,
        }
    }
}

macro_rules! scale_value {
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
                        scale_value!(hex_color.r, 255, 65535),
                        scale_value!(hex_color.g, 255, 65535),
                        scale_value!(hex_color.b, 255, 65535))?.reply()?.pixel as i32)
}

macro_rules! set_border_color {
    ($conn:expr, $values:expr, $style:expr, $field:ident, $cmap:expr) => {
        if let Some(MixedConfigVal::S(color_str)) = $values.get(concat!("border_", stringify!($field), "_color")) {
            $style.$field = alloc_color($conn, color_str, $cmap)?;
        }
    };
}

macro_rules! set_border_width {
    ($conn:expr, $values:expr, $style:expr, $field:ident) => {
        if let Some(MixedConfigVal::I(border_width)) = $values.get(concat!("border_", stringify!($field), "_width")) {
            $style.border.$field = *border_width as i16;
        }
    };
}

/// Check config and init all style related options
///
/// # Arguments
///
/// * `config` - Config values read either from args or config file
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    // Reset styles
    subtle.all_style.reset(0); // Ensure sane base values
    subtle.views_style.reset(-1);
    subtle.views_active_style.reset(-1);
    subtle.views_occupied_style.reset(-1);
    subtle.views_visible_style.reset(-1);
    subtle.title_style.reset(-1);
    subtle.urgent_style.reset(-1);
    subtle.separator_style.reset(-1);
    subtle.clients_style.reset(0);
    subtle.tray_style.reset(0);
    subtle.top_panel_style.reset(-1);
    subtle.bottom_panel_style.reset(-1);

    for style_values in config.styles.iter() {
        let style: &mut Style;

        if let Some(MixedConfigVal::S(kind))  = style_values.get("kind") {
            match kind.as_str() {
                "all" => style = &mut subtle.all_style,
                "views" => style = &mut subtle.views_style,
                "active_views" => style = &mut subtle.views_active_style,
                "occupied_views" => style = &mut subtle.views_occupied_style,
                "visible_views" => style = &mut subtle.views_visible_style,
                "separator" => style = &mut subtle.separator_style,
                "top_panel" => style = &mut subtle.top_panel_style,
                "bottom_panel" => style = &mut subtle.bottom_panel_style,
                "tray" => style = &mut subtle.tray_style,
                "urgent" => style = &mut subtle.urgent_style,
                "clients" => {
                    // We exploit some unused style variables here:
                    // margin <-> client gap
                    // padding <-> client strut

                    // Set client border color and width
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
                        subtle.clients_style.padding = Spacing::try_from(val)?;
                    }

                    style = &mut subtle.clients_style;
                },
                "title" => {
                    if let Some(MixedConfigVal::I(width)) = style_values.get("title_width") {
                        subtle.title_style.min_width = *width as i16;
                    }

                    style = &mut subtle.title_style;
                },
                _ => {
                    warn!("Unknown style name: {}", kind);

                    continue;
                },
            }

            // Handle colors
            if let Some(MixedConfigVal::S(color_str)) = style_values.get("foreground") {
                style.fg = alloc_color(conn, color_str, default_screen.default_colormap)?;
            }

            if let Some(MixedConfigVal::S(color_str)) = style_values.get("background") {
                style.bg = alloc_color(conn, color_str, default_screen.default_colormap)?;
            }

            // Handle border
            if let Some(MixedConfigVal::S(color_str)) = style_values.get("border_color") {
                style.top = alloc_color(conn, color_str, default_screen.default_colormap)?;
                style.right = style.top;
                style.bottom = style.top;
                style.left = style.top;
            }

            set_border_color!(conn, style_values, style, top, default_screen.default_colormap);
            set_border_color!(conn, style_values, style, right, default_screen.default_colormap);
            set_border_color!(conn, style_values, style, bottom, default_screen.default_colormap);
            set_border_color!(conn, style_values, style, left, default_screen.default_colormap);

            if let Some(MixedConfigVal::I(border_width)) = style_values.get("border_width") {
                style.border.top = *border_width as i16;
                style.border.right = style.border.top;
                style.border.bottom = style.border.top;
                style.border.left = style.border.top;
            }

            set_border_width!(conn, style_values, style, top);
            set_border_width!(conn, style_values, style, right);
            set_border_width!(conn, style_values, style, bottom);
            set_border_width!(conn, style_values, style, left);

            // Handle padding and margin
            if let Some(padding) = style_values.get("padding") {
                style.padding = Spacing::try_from(padding)?;
            }

            if let Some(margin) = style_values.get("margin") {
                style.margin = Spacing::try_from(margin)?;
            }

            // Handle font
            if let Some(MixedConfigVal::S(font_name)) = style_values.get("font") {
                let font = Font::new(conn, font_name)?;

                style.font_id = subtle.fonts.len() as isize;
                style.flags.insert(StyleFlags::FONT);

                subtle.fonts.push(font);
            }
        }
    }

    // Enforce sane defaults
    if -1 == subtle.title_style.min_width {
        subtle.title_style.min_width = 50;
    }

    debug!("{}", function_name!());

    Ok(())
}

macro_rules! update_panel_height {
    ($subtle:expr, $style:ident) => {
        if -1 != $subtle.$style.font_id {
            if let Some(font) = $subtle.fonts.get($subtle.$style.font_id as usize) {
                let new_height = $subtle.$style.calc_spacing(CalcSpacing::Height) as u16 + font.height;

                $subtle.panel_height = max!($subtle.panel_height, new_height);
            }
        }
    };
}

pub(crate) fn update(subtle: &mut Subtle) -> Result<()> {
    // Inherit styles
    subtle.views_style.inherit(&subtle.all_style);
    subtle.views_active_style.inherit(&subtle.views_style);
    subtle.views_occupied_style.inherit(&subtle.views_style);
    subtle.views_visible_style.inherit(&subtle.views_style);
    subtle.title_style.inherit(&subtle.all_style);
    subtle.tray_style.inherit(&subtle.all_style);
    subtle.urgent_style.inherit(&subtle.all_style);
    subtle.separator_style.inherit(&subtle.all_style);
    subtle.top_panel_style.inherit(&subtle.all_style);
    subtle.bottom_panel_style.inherit(&subtle.all_style);

    // Check fonts
    if !subtle.title_style.flags.contains(StyleFlags::FONT) {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let font = Font::new(conn, DEFAULT_FONT_NAME)?;

        subtle.title_style.font_id = subtle.fonts.len() as isize;
        subtle.title_style.flags.insert(StyleFlags::FONT);

        subtle.fonts.push(font);
    }

    // Update panel heights
    update_panel_height!(subtle, views_style);
    update_panel_height!(subtle, views_active_style);
    update_panel_height!(subtle, views_occupied_style);
    update_panel_height!(subtle, views_visible_style);
    update_panel_height!(subtle, title_style);
    update_panel_height!(subtle, tray_style);
    update_panel_height!(subtle, urgent_style);
    update_panel_height!(subtle, separator_style);
    update_panel_height!(subtle, top_panel_style);
    update_panel_height!(subtle, bottom_panel_style);

    debug!("{}", function_name!());

    Ok(())
}
