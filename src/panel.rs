///
/// @package subtle-rs
///
/// @file View functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use bitflags::{bitflags, bitflags_match};
use log::{debug, warn};
use anyhow::{Context, Result};
use easy_min_max::{max, min};
use stdext::function_name;
use x11rb::protocol::xproto::{ChangeGCAux, ConnectionExt, Drawable, Rectangle};
use crate::client::ClientFlags;
use crate::style::{CalcSide, Style, StyleFlags};
use crate::subtle::Subtle;

bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct PanelFlags: u32 {
        const SUBLET = 1 << 0;          // Panel sublet type
        const COPY = 1 << 1;            // Panel copy type
        const VIEWS = 1 << 2;           // Panel views type
        const TITLE = 1 << 3;           // Panel title type
        const KEYCHAIN = 1 << 4;        // Panel keychain type
        const TRAY = 1 << 5;            // Panel tray type
        const ICON = 1 << 6;            // Panel icon type

        const SPACER1 = 1 << 7;          // Panel spacer1
        const SPACER2 = 1 << 8;          // Panel spacer2
        const SEPARATOR_BEFORE = 1 << 9; // Panel separator before item
        const SEPARATOR_AFTER = 1 << 10; // Panel separator after item
        const BOTTOM = 1 << 11;          // Panel bottom
        const HIDDEN = 1 << 12;          // Panel hidden
        const CENTER = 1 << 13;          // Panel center
        const SUBLETS = 1 << 14;         // Panel sublets

        const MOUSE_DOWN = 1 << 15;      // Panel mouse down
        const MOUSE_OVER = 1 << 16;      // Panel mouse over
        const MOUSE_OUT = 1 << 17;       // Panel mouse out
    }
}

#[derive(Default, Debug)]
pub(crate) struct Panel {
    pub(crate) flags: PanelFlags,
    pub(crate) x: i16,
    pub(crate) width: u16,
    pub(crate) screen_id: usize,
}

impl Panel {
    pub(crate) fn new(subtle: &Subtle, flag: PanelFlags) -> Self {
        let mut panel = Self {
            flags: flag,
            ..Self::default()
        };

        bitflags_match!(flag, {
            PanelFlags::ICON => {}, // TODO icon
            PanelFlags::SUBLETS => {}, // TODO sublets
            PanelFlags::VIEWS => {
                panel.flags.insert(PanelFlags::MOUSE_DOWN);
            },
            _ => warn!("Unknown panel flag {:?}", flag),
        });

        debug!("{}: {}", function_name!(), panel);

        panel
    }

    pub(crate) fn update(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Handle panel item type
        if self.flags.intersects(PanelFlags::TRAY) {
            // TODO tray
        } else if self.flags.intersects(PanelFlags::ICON) {
            // TODO icon
        } else if self.flags.intersects(PanelFlags::TITLE) {
            self.width = subtle.clients_style.min_width as u16;

            // Find focus window
            if let Some(focus) = subtle.find_focus_client() {
                if !focus.is_alive() {
                    return Ok(());
                }

                if !focus.flags.contains(ClientFlags::TYPE_DESKTOP) {
                    if let Ok(mode_str) = focus.format_modes() {
                        // Font offset, panel border and padding
                        if -1 != subtle.title_style.font_id
                            && let Some(font) = subtle.fonts.get(subtle.title_style.font_id as usize)
                        {
                            if let Ok((width, _, _)) = font.calc_text_width(conn, &*focus.name, false) {
                                self.width = min!(subtle.clients_style.right as u16, width) + mode_str.len() as u16;
                            }
                        }
                    }

                    // Ensure min-width
                    self.width = max!(subtle.clients_style.min_width as u16, self.width);
                }
            }
        } else if self.flags.intersects(PanelFlags::SEPARATOR_BEFORE | PanelFlags::SEPARATOR_AFTER) {
            if -1 != subtle.separator_style.font_id
                && let Some(font) = subtle.fonts.get(subtle.separator_style.font_id as usize)
            {
                if let Ok((width, _, _)) = font.calc_text_width(conn,
                                                                subtle.separator_style.sep_string.clone().unwrap().as_str(), false)
                {
                    self.width = width;
                }
            }
        } else if self.flags.intersects(PanelFlags::VIEWS) {
            // TODO views
        }

        debug!("{}: {}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn draw_rect(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if 0 >= self.width {
            return Ok(());
        }

        let min_width = style.margin.left + style.margin.right;
        let min_height: i16 = style.margin.top + style.margin.bottom;

        // Filling
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.bg as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: self.width - min_width as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        // Borders: Top
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.top as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: self.width - min_width as u16,
            height: style.border.top as u16,
        }])?.check()?;

        // Borders: Right
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.right as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + self.width as u16 - style.border.right as u16 - style.margin.right as u16 + offset_x) as i16,
            y: style.margin.top,
            width: style.border.right as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        // Borders: Bottom
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.bottom as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: subtle.panel_height as i16 - style.border.bottom - style.margin.bottom,
            width: self.width - min_width as u16,
            height: style.border.bottom as u16,
        }])?.check()?;

        // Borders: Left
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.left as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: style.border.left as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        Ok(())
    }

    pub(crate) fn draw_text(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, text: &String, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if -1 != style.font_id && let Some(font) = subtle.fonts.get(style.font_id as usize) {
            conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
                .foreground(style.fg as u32)
                .background(style.bg as u32))?.check()?;
            conn.poly_text16(drawable, subtle.draw_gc,
                             (self.x as u16 + style.calc_side(CalcSide::Left) as u16 + offset_x) as i16,
                             font.y as i16 + style.calc_side(CalcSide::Top),
                             text.as_bytes())?.check()?;
        }

        Ok(())
    }

    pub(crate) fn draw_separator(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if style.flags.intersects(StyleFlags::SEPARATOR) {
            self.draw_rect(subtle, drawable, offset_x, &subtle.separator_style)?;
            self.draw_text(subtle, drawable, offset_x, &style.sep_string.clone().unwrap(), style )?;
        }

        Ok(())
    }

    pub(crate) fn render(&mut self, subtle: &Subtle, drawable: Drawable) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Draw separator before panel item
        if self.flags.intersects(PanelFlags::SEPARATOR_BEFORE)
            && subtle.separator_style.flags.intersects(StyleFlags::SEPARATOR)
        {
            self.draw_separator(subtle, drawable, -subtle.separator_style.sep_width as u16,
                                &subtle.separator_style)?;
        }

        // Handle panel item type
        if self.flags.intersects(PanelFlags::TRAY) {
            // TODO tray
        } else if self.flags.intersects(PanelFlags::ICON) {
            // TODO icon
        } else if self.flags.intersects(PanelFlags::TITLE) {
            // Find focus window
            if let Some(focus) = subtle.find_focus_client() {
                if !focus.is_alive() {
                    return Ok(());
                }

                if !focus.flags.contains(ClientFlags::TYPE_DESKTOP) && focus.is_visible(subtle) {

                    // Set window background and border
                    self.draw_rect(subtle, drawable, 0, &subtle.title_style)?;

                    let mut offset_x = 0;

                    // Draw modes and title
                    if let Ok(mode_str) = focus.format_modes() {
                        self.draw_text(subtle, drawable, 0, &mode_str, &subtle.title_style)?;

                        offset_x += mode_str.len();
                    }

                    self.draw_text(subtle, drawable, offset_x as u16, &focus.name, &subtle.title_style)?;
                }
            }
        }

        // Draw separator after panel item
        if self.flags.intersects(PanelFlags::SEPARATOR_AFTER)
            && subtle.separator_style.flags.intersects(StyleFlags::SEPARATOR)
        {
            self.draw_separator(subtle, drawable, self.width, &subtle.separator_style)?;
        }

        debug!("{}: {}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x={}, width={}, screen_id={})", self.x, self.width, self.screen_id)
    }
}
