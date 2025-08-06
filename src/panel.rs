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
use bitflags::bitflags;
use log::{debug, warn};
use anyhow::{Context, Result};
use easy_min_max::{max, min};
use stdext::function_name;
use x11rb::protocol::xproto::{ChangeGCAux, ConnectionExt, Drawable, Rectangle};
use crate::client::ClientFlags;
use crate::style::{CalcSpacing, Style, StyleFlags};
use crate::subtle::Subtle;
use crate::tagging::Tagging;
use crate::view::{View, ViewFlags};

bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct PanelFlags: u32 {
        const TITLE = 1 << 0;            // Panel title type
        const VIEWS = 1 << 1;            // Panel views type
        const TRAY = 1 << 2;             // Panel tray type
        const ICON = 1 << 3;             // Panel icon type
        const SCRIPT = 1 << 4;           // Panel script type

        const COPY = 1 << 5;             // Panel copy type

        const SPACER_BEFORE = 1 << 6;    // Panel spacer before item
        const SPACER_AFTER = 1 << 7;     // Panel spacer after item
        const SEPARATOR_BEFORE = 1 << 8; // Panel separator before item
        const SEPARATOR_AFTER = 1 << 9;  // Panel separator after item
        const BOTTOM_MARKER = 1 << 10;   // Panel bottom marker
        const HIDDEN = 1 << 11;          // Panel hidden
        const CENTER = 1 << 12;          // Panel center
        const SUBLETS = 1 << 13;         // Panel sublets

        const MOUSE_DOWN = 1 << 14;      // Panel mouse down
        const MOUSE_OVER = 1 << 15;      // Panel mouse over
        const MOUSE_OUT = 1 << 16;       // Panel mouse out
    }
}

pub(crate) enum PanelAction {
    MouseOver(i16, i16),
    MouseDown(i16, i16, i8),
    MouseOut,
}

#[derive(Default, Debug)]
pub(crate) struct Panel {
    pub(crate) flags: PanelFlags,
    pub(crate) x: i16,
    pub(crate) width: u16,
    pub(crate) screen_id: usize,
}

impl Panel {
    fn pick_style(&mut self, subtle: &&Subtle, style: &mut Style, view_idx: usize, view: &View) {
        // Pick base style
        if let Some(current_screen) = subtle.screens.get(self.screen_id) {
            if view_idx as isize == current_screen.view_id {
                style.inherit(&subtle.views_active_style);
            } else if subtle.client_tags.get().intersects(view.tags) {
                style.inherit(&subtle.views_occupied_style);
            }
        }

        style.inherit(&subtle.views_style);

        // Apply modifier styles
        if subtle.urgent_tags.get().intersects(view.tags) {
            style.inherit(&subtle.urgent_style);
        }

        if subtle.visible_views.get().intersects(Tagging::from_bits_retain(1 << (view_idx + 1))) {
            style.inherit(&subtle.views_visible_style);
        }
    }

    fn draw_rect(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, width: u16, style: &Style) -> Result<()> {
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
            width: width - min_width as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        // Borders: Top
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.top as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: width - min_width as u16,
            height: style.border.top as u16,
        }])?.check()?;

        // Borders: Right
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.right as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + self.width  as i16 - style.border.right - style.margin.right + offset_x as i16,
            y: style.margin.top,
            width: style.border.right as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        // Borders: Bottom
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.bottom as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + style.margin.left + offset_x as i16,
            y: subtle.panel_height as i16 - style.border.bottom - style.margin.bottom,
            width: self.width - min_width as u16,
            height: style.border.bottom as u16,
        }])?.check()?;

        // Borders: Left
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.left as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + style.margin.left + offset_x as i16,
            y: style.margin.top,
            width: style.border.left as u16,
            height: subtle.panel_height - min_height as u16,
        }])?.check()?;

        Ok(())
    }

    fn draw_text(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, text: &String, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if let Some(font) = style.get_font(subtle) {
            conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
                .font(font.fontable)
                .foreground(style.fg as u32)
                .background(style.bg as u32))?.check()?;

            conn.image_text8(drawable, subtle.draw_gc,
                             (self.x as u16 + style.calc_spacing(CalcSpacing::Left) as u16 + offset_x) as i16,
                             font.y as i16 + style.calc_spacing(CalcSpacing::Top),
                             text.as_bytes())?.check()?;
        }

        Ok(())
    }

    fn draw_separator(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if style.flags.intersects(StyleFlags::SEPARATOR) {
            self.draw_rect(subtle, drawable, offset_x, self.width, &subtle.separator_style)?;
            self.draw_text(subtle, drawable, offset_x, &style.sep_string.clone().unwrap(), style )?;
        }

        Ok(())
    }

    pub(crate) fn new(flags: PanelFlags) -> Option<Self> {
        let mut panel = Self {
            flags,
            ..Self::default()
        };

        if flags.intersects(PanelFlags::TITLE) {
            // TODO title
        } else if flags.intersects(PanelFlags::VIEWS) {
            panel.flags.insert(PanelFlags::MOUSE_DOWN);
        } else {
            debug!("Unhandled panel type: {:?}", flags);

            return None
        }

        debug!("{}: panel={}", function_name!(), panel);

        Some(panel)
    }

    pub(crate) fn update(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Handle panel item type
        if self.flags.intersects(PanelFlags::TRAY) {
            // TODO tray
        } else if self.flags.intersects(PanelFlags::ICON) {
            // TODO icon
        } else if self.flags.intersects(PanelFlags::TITLE) {
            self.width = subtle.title_style.min_width as u16;

            // Find focus window
            if let Some(focus) = subtle.find_focus_client() {
                if focus.is_alive() && !focus.flags.intersects(ClientFlags::TYPE_DESKTOP) {
                    if let Ok(mode_str) = focus.format_modes() {
                        // Font offset, panel border and padding
                        if let Some(font) = subtle.title_style.get_font(subtle) {
                            if let Ok((width, _, _)) = font.calc_text_width(conn, &focus.name, false) {
                                self.width = min!(subtle.clients_style.right as u16, width) + mode_str.len() as u16
                                    + subtle.title_style.calc_spacing(CalcSpacing::Width) as u16;
                            }
                        }
                    }

                    // Ensure min-width
                    self.width = max!(subtle.title_style.min_width as u16, self.width);
                }
            }
        } else if self.flags.intersects(PanelFlags::VIEWS) {
            self.width = subtle.views_style.min_width as u16;

            let mut style = Style::default();

            for (view_idx, view) in subtle.views.iter().enumerate() {
                // Skip dynamic
                if view.flags.intersects(ViewFlags::MODE_DYNAMIC) && !subtle.client_tags.get().intersects(view.tags) {
                    continue;
                }

                self.pick_style(&subtle, &mut style, view_idx, view);

                // Update view width
                let mut view_width = 0u16;

                if view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                    // TODO icons
                } else {
                    if let Some(font) = style.get_font(subtle) {
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &view.name, false) {
                            view_width = width + style.calc_spacing(CalcSpacing::Width) as u16; // TODO icons
                        }
                    }
                }

                self.width += max!(style.min_width, view_width as i16) as u16;
            }

            // Add width of view separator if any
            if subtle.views_style.sep_string.is_some() {
                self.width += (subtle.views.len() - 1) as u16 * subtle.views_style.sep_width as u16;
            }
        }

        debug!("{}: panel={}", function_name!(), self);

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

        println!("render panel={}", self);

        // Handle panel item type
        if self.flags.intersects(PanelFlags::TRAY) {
            // TODO tray
        } else if self.flags.intersects(PanelFlags::ICON) {
            // TODO icon
        } else if self.flags.intersects(PanelFlags::TITLE) {
            // Find focus window
            if let Some(focus) = subtle.find_focus_client() {
                if focus.is_alive() && focus.is_visible(subtle)
                    && !focus.flags.intersects(ClientFlags::TYPE_DESKTOP)
                {
                    let mut offset_x = subtle.title_style.calc_spacing(CalcSpacing::Left) as u16;

                    // Set window background and border
                    self.draw_rect(subtle, drawable, 0, self.width, &subtle.title_style)?;

                    // Draw modes and title
                    if let Ok(mode_str) = focus.format_modes() {
                        self.draw_text(subtle, drawable, 0, &mode_str, &subtle.title_style)?;

                        offset_x += mode_str.len() as u16;
                    }

                    self.draw_text(subtle, drawable, offset_x, &focus.name, &subtle.title_style)?;
                }
            }
        } else if self.flags.intersects(PanelFlags::VIEWS) {
            let mut style = Style::default();
            let mut offset_x = 0;

            for (view_idx, view) in subtle.views.iter().enumerate() {

                // Skip dynamic
                if view.flags.intersects(ViewFlags::MODE_DYNAMIC)
                    && !subtle.client_tags.get().intersects(view.tags)
                {
                    continue;
                }

                self.pick_style(&subtle, &mut style, view_idx, view);

                // Draw icon and/or text
                if view.flags.intersects(ViewFlags::MODE_ICON) {
                    // TODO icons
                }

                let mut view_width= style.calc_spacing(CalcSpacing::Width) as u16; // TODO icons

                if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                    // Add space between icon and text
                    if view.flags.intersects(ViewFlags::MODE_ICON) {
                        // TODO icons
                    }

                    if let Some(font) = style.get_font(subtle) {
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &view.name, false) {
                            view_width += width;
                        }
                    }
                }

                offset_x += style.calc_spacing(CalcSpacing::Left) as u16;

                // Set window background and border
                self.draw_rect(subtle, drawable, offset_x, view_width, &style)?;

                if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                    // Add space between icon and text
                    if view.flags.intersects(ViewFlags::MODE_ICON) {
                        // TODO icons
                    }

                    self.draw_text(subtle, drawable, offset_x, &view.name, &style)?;
                }

                offset_x += max!(style.min_width as u16, view_width);

                // Draw view separator if any
                if subtle.views_style.sep_string.is_some() && view_idx < subtle.views.len() - 1 {
                    self.draw_separator(subtle, drawable, offset_x, &style)?;

                    offset_x += subtle.views_style.sep_width as u16;
                }
            }
        }

        // Draw separator after panel item
        if self.flags.intersects(PanelFlags::SEPARATOR_AFTER)
            && subtle.separator_style.flags.intersects(StyleFlags::SEPARATOR)
        {
            self.draw_separator(subtle, drawable, self.width, &subtle.separator_style)?;
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn handle_action(&self, subtle: &Subtle, action: PanelAction, is_bottom: bool) -> Result<()> {
        if self.flags.intersects(PanelFlags::VIEWS)
            && let PanelAction::MouseDown(x, y, button) = action
        {
            let mut offset_x = self.x;

            for view in subtle.views.iter() {
                // Skip dynamic views
                if view.flags.intersects(ViewFlags::MODE_DYNAMIC)
                    && !(subtle.client_tags.get().intersects(view.tags)) {
                    continue;
                }

                // Check if x is in view rect
                if x >= offset_x && x <= offset_x + view.text_width as i16 {
                    view.focus(subtle)?;
                }

                // Add view separator width if any
                if subtle.views_style.sep_string.is_some() {
                    offset_x += view.text_width as i16 + subtle.views_style.sep_width;
                } else {
                    offset_x += view.text_width as i16;
                }
            }
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x={}, width={}, screen_id={}, flags={:?})", self.x, self.width, self.screen_id, self.flags)
    }
}
