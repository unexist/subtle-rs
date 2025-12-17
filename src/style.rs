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
use std::collections::HashMap;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Colormap, ConnectionExt};
use x11rb::rust_connection::RustConnection;
use crate::config::{Config, MixedConfigVal};
use crate::font::Font;
use crate::spacing::Spacing;
use crate::subtle::Subtle;

bitflags! {
    /// Config and state-flags for [`Style`]
    #[derive(Default, Debug, Clone)]
    pub(crate) struct StyleFlags: u32 {
        /// Style has custom font
        const FONT = 1 << 0;
        /// Style has separator
        const SEPARATOR = 1 << 1;
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

#[derive(Debug, Clone)]
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
    /// Calculate the spacing of the style for the given dimension
    ///
    /// # Arguments
    ///
    /// * `spacing` - Spacing dimension
    ///
    /// # Returns
    ///
    /// Pixel width of the style for the dimension
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

    /// Inherit style values from other style
    ///
    /// # Arguments
    ///
    /// * `other_style` - The other style
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

    /// Reset style values to the given default value
    ///
    /// # Arguments
    ///
    /// * `default_value` - Default value to set
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

    /// Helper to get the font of this style if any
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Option`] with either [`Some`] on success or otherwise [`None`]
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

/// Helper macro to set color of border side in a style
///
/// # Arguments
///
/// * `conn` - Connection to X11
/// * `values` - Values to evaluate
/// * `style` - Style to update
/// * `field` - Field to set
/// * `colormap` - Colormap to use
macro_rules! set_border_color {
    ($conn:expr, $values:expr, $style:expr, $field:ident, $colormap:expr) => {
        if let Some(MixedConfigVal::S(color_str)) = $values.get(concat!("border_", stringify!($field), "_color")) {
            $style.$field = alloc_color($conn, color_str, $colormap)?;
        }
    };
}

/// Helper macro to set width of border side in a style
///
/// # Arguments
///
/// * `values` - Values to evaluate
/// * `style` - Style to update
/// * `field` - Field to set
macro_rules! set_border_width {
    ($values:expr, $style:expr, $field:ident) => {
        if let Some(MixedConfigVal::I(border_width)) = $values.get(concat!("border_", stringify!($field), "_width")) {
            $style.border.$field = *border_width as i16;
        }
    };
}

/// Helper macro to scale value from range to new range
///
/// # Arguments
///
/// * `value` - Value to scale
/// * `old_range` - Divisor
/// * `new_range` - Multiplicator
///
/// # Returns
///
/// Either [`u16`] on success or otherwise if value is equal or less zero [`0`]
macro_rules! scale_value {
    ($value:expr, $old_range:expr, $new_range:expr) => {
        if 0 < $value {
            (($value as f32 / $old_range as f32) * $new_range as f32) as u16
        } else {
            0
        }
    };
}

/// Allocate color based on hex string for given colormap
///
/// # Arguments
///
/// * `conn` - X11 connection
/// * `color_str` - Hex color string like #000000
///
/// # Returns
///
/// A [`Result`] with either [`i32`] on success or otherwise [`anyhow::Error`]
fn alloc_color(conn: &RustConnection, color_str: &str, cmap: Colormap) -> Result<i32> {
    let hex_color = HexColor::parse(color_str)?;

    Ok(conn.alloc_color(cmap,
                        scale_value!(hex_color.r, 255, 65535),
                        scale_value!(hex_color.g, 255, 65535),
                        scale_value!(hex_color.b, 255, 65535))?.reply()?.pixel as i32)
}

fn parse(subtle: &mut Subtle, style_values: &HashMap<String, MixedConfigVal>, default_value: i32) -> Result<Style> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    // Create new style with default value
    let mut style = Style::default();

    style.reset(default_value);

    // We exploit some unused style variables here:
    // margin <-> client gap
    // padding <-> client strut

    // Set client border color and width
    if let Some(MixedConfigVal::S(color_str)) = style_values.get("active") {
        style.fg = alloc_color(conn, color_str, default_screen.default_colormap)?;
    }

    if let Some(MixedConfigVal::S(color_str)) = style_values.get("inactive") {
        style.bg = alloc_color(conn, color_str, default_screen.default_colormap)?;
    }

    if let Some(MixedConfigVal::I(width)) = style_values.get("border_width") {
        style.border.top = *width as i16;
    }

    // Set client strut
    if let Some(val) = style_values.get("strut") {
        style.padding = Spacing::try_from(val)?;
    }

    if let Some(MixedConfigVal::I(width)) = style_values.get("title_width") {
        style.min_width = *width as i16;
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

    set_border_width!(style_values, style, top);
    set_border_width!(style_values, style, right);
    set_border_width!(style_values, style, bottom);
    set_border_width!(style_values, style, left);

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

    Ok(style)
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
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {

    for style_values in config.styles.iter() {
        if let Some(MixedConfigVal::S(kind)) = style_values.get("kind") {
            match kind.as_str() {
                "all" => subtle.all_style = parse(subtle, style_values, 0)?, // Ensure sane base values
                "views" => subtle.views_style = parse(subtle, style_values, -1)?,
                "active_views" => subtle.views_active_style = parse(subtle, style_values, -1)?,
                "occupied_views" => subtle.views_occupied_style = parse(subtle, style_values, -1)?,
                "visible_views" => subtle.views_visible_style = parse(subtle, style_values, -1)?,
                "separator" => subtle.separator_style = parse(subtle, style_values, -1)?,
                "top_panel" => subtle.top_panel_style = parse(subtle, style_values, -1)?,
                "bottom_panel" => subtle.bottom_panel_style = parse(subtle, style_values, -1)?,
                "tray" => subtle.tray_style = parse(subtle, style_values, 0)?,
                "urgent" => subtle.urgent_style = parse(subtle, style_values, -1)?,
                "clients" => subtle.clients_style = parse(subtle, style_values, 0)?,
                "title" => subtle.title_style = parse(subtle, style_values, -1)?,
                _ => warn!("Unknown style kind `{}`", kind),
            }
        }
    }

    debug!("{}", function_name!());

    Ok(())
}

/// Helper macro to update spacing
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `style` - Style to use
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

/// Update all styles
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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

    println!("all_style={:?}", subtle.all_style);
    println!("views_style={:?}", subtle.views_style);
    //println!("active_style={:?}", subtle.views_active_style);
    //println!("occupied_style={:?}", subtle.views_occupied_style);
    //println!("visible_style={:?}", subtle.views_visible_style);

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
