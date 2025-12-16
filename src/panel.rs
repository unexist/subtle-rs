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
use log::debug;
use anyhow::{anyhow, Context, Result};
use easy_min_max::max;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ChangeGCAux, ConnectionExt, Drawable, Rectangle};
use crate::client::ClientFlags;
use crate::icon::Icon;
use crate::screen::Screen;
use crate::style::{CalcSpacing, Style};
use crate::subtle::Subtle;
use crate::tagging::Tagging;
use crate::tray::TrayFlags;
use crate::view::{View, ViewFlags};

bitflags! {
    /// Config and state-flags for [`Panel`]
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct PanelFlags: u32 {
        /// Title type
        const TITLE = 1 << 0;
        /// Views type
        const VIEWS = 1 << 1;
        /// Tray type
        const TRAY = 1 << 2;
        /// Icon type
        const ICON = 1 << 3;
        /// Plugin type
        const PLUGIN = 1 << 4;
        /// Separator type
        const SEPARATOR = 1 << 5;
        /// Copy type
        const COPY = 1 << 6;

        /// Bottom marker
        const BOTTOM_START_MARKER = 1 << 7;

        /// Left position
        const LEFT_POS = 1 << 8;
        /// Center position
        const CENTER_POS = 1 << 9;
        /// Right position
        const RIGHT_POS = 1 << 10;

        /// Hidden panel
        const HIDDEN = 1 << 11;

        /// Mouse down action
        const MOUSE_DOWN = 1 << 12;
        /// Mouse over action
        const MOUSE_OVER = 1 << 13;
        /// Mouse out action
        const MOUSE_OUT = 1 << 14;
    }
}

impl From<&String> for PanelFlags {
    fn from(value: &String) -> Self {
        let mut pos_flags = PanelFlags::empty();

        // Handle positional flags
        if 1 < value.len() {
            pos_flags = match value.chars().next() {
                Some('<') => PanelFlags::LEFT_POS,
                Some('=') => PanelFlags::CENTER_POS,
                Some('>') => PanelFlags::RIGHT_POS,
                _ => PanelFlags::empty(),
            }
        }

        // Handle actual types
        let idx = if pos_flags.is_empty() { 0 } else { 1 };

        match &value[idx..] {
            "title" => PanelFlags::TITLE | pos_flags,
            "views" => PanelFlags::VIEWS | pos_flags,
            "tray" => PanelFlags::TRAY | pos_flags,
            panel if panel.starts_with("$") => PanelFlags::PLUGIN | pos_flags,
            _ => PanelFlags::SEPARATOR | pos_flags
        }
    }
}

pub(crate) enum PanelAction {
    MouseOver(i16, i16),
    MouseDown(i16, i16, i8),
    MouseOut,
}

#[derive(Default, Clone, Copy, Debug)]
struct PanelPlacement {
    offset_x: i16,
    width: u16,
}

#[derive(Default, Debug)]
pub(crate) struct Panel {
    pub(crate) flags: PanelFlags,
    pub(crate) x: i16,
    pub(crate) width: u16,
    pub(crate) screen_id: usize,
    pub(crate) plugin_id: usize,
    pub(crate) text: Option<String>,
    pub(crate) text_widths: Vec<u16>,
}

impl Panel {
    /// Pick relevant style for drawing
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `style` - Style to use
    /// * `view_idx` - View index
    /// * `view` - View to use
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn pick_style(&self, subtle: &Subtle, style: &mut Style, view_idx: usize, view: &View) {
        style.reset(-1);

        // Pick base style
        if let Some(current_screen) = subtle.screens.get(self.screen_id) {
            if current_screen.view_idx.get() == view_idx as isize {
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

    /// Draw rect on panel
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `drawable` - Drawable to use
    /// * `offset_x` - X offset on panel
    /// * `width` - Width of the rectable
    /// * `style` - Style to use
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn draw_rect(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16,
                 width: u16, style: &Style) -> Result<()>
    {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        if 0 >= self.width {
            return Ok(());
        }

        let margin_width = style.margin.left + style.margin.right;
        let margin_height: i16 = style.margin.top + style.margin.bottom;

        // Filling
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.bg as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: width - margin_width as u16,
            height: subtle.panel_height - margin_height as u16,
        }])?.check()?;

        // Borders: Top
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.top as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: (self.x as u16 + style.margin.left as u16 + offset_x) as i16,
            y: style.margin.top,
            width: width - margin_width as u16,
            height: style.border.top as u16,
        }])?.check()?;

        // Borders: Right
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.right as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + width as i16 - style.border.right - style.margin.right + offset_x as i16,
            y: style.margin.top,
            width: style.border.right as u16,
            height: subtle.panel_height - margin_height as u16,
        }])?.check()?;

        // Borders: Bottom
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.bottom as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + style.margin.left + offset_x as i16,
            y: subtle.panel_height as i16 - style.border.bottom - style.margin.bottom,
            width: width - margin_width as u16,
            height: style.border.bottom as u16,
        }])?.check()?;

        // Borders: Left
        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.left as u32))?.check()?;
        conn.poly_fill_rectangle(drawable, subtle.draw_gc, &[Rectangle {
            x: self.x + style.margin.left + offset_x as i16,
            y: style.margin.top,
            width: style.border.left as u16,
            height: subtle.panel_height - margin_height as u16,
        }])?.check()?;

        Ok(())
    }

    /// Draw text on panel
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `drawable` - Drawable to use
    /// * `offset_x` - X offset on panel
    /// * `text` - Text to draw
    /// * `style` - Style to use
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn draw_text(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16,
                 text: &String, style: &Style) -> Result<()>
    {
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

    /// Draw icon on panel
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `icon` - Icon to draw
    /// * `drawable` - Drawable to use
    /// * `offset_x` - X offset on panel
    /// * `style` - Style to use
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    fn draw_icon(&self, subtle: &Subtle, icon: &Icon, drawable: Drawable,
                 offset_x: u16, style: &Style) -> Result<()>
    {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default()
            .foreground(style.fg as u32)
            .background(style.bg as u32))?.check()?;

        conn.copy_plane(icon.pixmap, drawable, subtle.draw_gc, 0, 0,
                        self.x + offset_x as i16 + style.calc_spacing(CalcSpacing::Left),
                        ((subtle.panel_height - icon.height) / 2) as i16,
                        icon.width, icon.height, 1)?.check()?;

        Ok(())
    }

    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `flags` - Panel flags to set
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Panel`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn new(name: &String) -> Result<Self> {
        let mut panel = Self {
            flags: PanelFlags::from(name),
            ..Self::default()
        };

        // Handle panel types
        if panel.flags.intersects(PanelFlags::SEPARATOR) {
            panel.text_widths.resize(1, Default::default());

            // Separator use its name as a value
            let idx = if panel.flags.intersects(PanelFlags::LEFT_POS
                | PanelFlags::CENTER_POS
                | PanelFlags::RIGHT_POS) { 1 } else { 0 };

            panel.text = Some(name[idx..].to_string());
        } else if panel.flags.intersects(PanelFlags::TITLE) {
            panel.text_widths.resize(2, Default::default());
        } else if panel.flags.intersects(PanelFlags::PLUGIN) {
            panel.text_widths.resize(1, Default::default());
        } else if panel.flags.intersects(PanelFlags::VIEWS) {
            panel.flags.insert(PanelFlags::MOUSE_DOWN);
        } else if !panel.flags.intersects(PanelFlags::TRAY) {
            debug!("Unhandled panel type: {:?}", panel.flags);

            return Err(anyhow!("Unhandled panel type"));
        }

        debug!("{}: panel={}", function_name!(), panel);

        Ok(panel)
    }

    /// Render the panel
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn update(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Handle panel item type
        if self.flags.intersects(PanelFlags::PLUGIN) {
            if let Some(plugin) = subtle.plugins.get(self.plugin_id) {
                if let Ok(res) = plugin.update() {
                    if let Some(font) = subtle.views_style.get_font(subtle) {
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &res, false) {
                            self.text_widths[0] = width;
                        }
                    }

                    // Finally update actual length
                    self.width = self.text_widths[0]
                        + subtle.views_style.calc_spacing(CalcSpacing::Width) as u16;

                    self.text = Some(res);
                }
            }
        } else if self.flags.intersects(PanelFlags::SEPARATOR) {
            if let Some(text) = &self.text {
                if let Some(font) = subtle.separator_style.get_font(subtle) {
                    if let Ok((width, _, _)) = font.calc_text_width(conn, &text, false) {
                        self.text_widths[0] = width;
                    }
                }

                // Finally update actual length
                self.width = self.text_widths[0]
                    + subtle.separator_style.calc_spacing(CalcSpacing::Width) as u16;
            }
        } else if self.flags.intersects(PanelFlags::TRAY) {
            self.width = subtle.tray_style.calc_spacing(CalcSpacing::Width) as u16;
            self.flags.remove(PanelFlags::HIDDEN);

            if let Ok(trays) = subtle.trays.try_borrow() && !trays.is_empty() {
                for tray_idx in 0..trays.len() {
                    let tray = trays.get(tray_idx).unwrap();

                    if tray.flags.intersects(TrayFlags::DEAD) {
                        continue;
                    }

                    tray.resize(subtle, self.width as i32)?;

                    self.width += tray.width;
                }
            } else {
                conn.unmap_window(subtle.tray_win)?.check()?;

                self.flags.insert(PanelFlags::HIDDEN);
            }
        } else if self.flags.intersects(PanelFlags::TITLE) {
            self.width = 0;

            // Find focus window
            if let Some(focus_client) = subtle.find_focus_client() {
                if focus_client.is_alive() && focus_client.is_visible(subtle)
                    && !focus_client.flags.intersects(ClientFlags::TYPE_DESKTOP)
                {
                    let mode_str = focus_client.mode_string();

                    // Font offset, panel border and padding
                    if let Some(font) = subtle.title_style.get_font(subtle) {
                        // Cache length of mode string
                        if let Ok((width, _, _)) = font.calc_text_width(conn,
                                                                        &mode_str, false)
                        {
                            self.text_widths[0] = width;
                        }

                        // Cache length of actual title
                        if let Ok((width, _, _)) = font.calc_text_width(conn,
                                                                        &focus_client.name, false)
                        {
                            self.text_widths[1] = width;
                        }

                        // Finally update actual length
                        self.width = self.text_widths[0]
                            + if self.text_widths[1] > subtle.clients_style.right as u16 {
                                subtle.clients_style.right as u16 } else { self.text_widths[1]
                            }
                            + subtle.title_style.calc_spacing(CalcSpacing::Width) as u16;
                    }

                    // Ensure min-width
                    self.width = max!(subtle.title_style.min_width as u16, self.width);
                }
            }
        } else if self.flags.intersects(PanelFlags::VIEWS) {
            self.width = 0;

            // Resize in case the length has changed
            if self.text_widths.capacity() != subtle.views.len() {
                self.text_widths.resize(subtle.views.len(), Default::default());
            }

            let mut style = Style::default();

            for (view_idx, view) in subtle.views.iter().enumerate() {
                // Skip dynamic
                if view.flags.intersects(ViewFlags::MODE_DYNAMIC)
                    && !subtle.client_tags.get().intersects(view.tags)
                {
                    continue;
                }

                self.pick_style(subtle, &mut style, view_idx, view);

                // Update view width
                let mut view_width = style.calc_spacing(CalcSpacing::Width) as u16;

                if view.flags.intersects(ViewFlags::MODE_ICON_ONLY)
                    && let Some(icon) = view.icon.as_ref()
                {
                    view_width += icon.width;
                } else {
                    if let Some(font) = style.get_font(subtle) {
                        // Cache length of view name
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &view.name, false) {
                            self.text_widths[view_idx] = width;
                        }

                        view_width += self.text_widths[view_idx]
                            + style.calc_spacing(CalcSpacing::Left) as u16;

                        if view.flags.intersects(ViewFlags::MODE_ICON)
                            && let Some(icon) = view.icon.as_ref()
                        {
                            view_width += icon.width + style.calc_spacing(CalcSpacing::Left) as u16;
                        }
                    }
                }

                // Ensure min-width
                self.width += max!(style.min_width as u16, view_width);
            }

            // TODO Add width of view separator if any
            //if subtle.views_style.sep_string.is_some() {
            //    self.width += (subtle.views.len() - 1) as u16 * subtle.views_style.sep_width as u16;
            //}
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }

    /// Render the panel
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn render(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Handle panel item type
        if self.flags.intersects(PanelFlags::ICON) {
            todo!(); // TODO icon
        } else if self.flags.intersects(PanelFlags::PLUGIN) {
            self.draw_rect(subtle, subtle.panel_double_buffer,0, self.width, &subtle.views_style)?;

            if let Some(text) = &self.text {
                self.draw_text(subtle, subtle.panel_double_buffer, 0, &text, &subtle.views_style)?;
            }
        } else if self.flags.intersects(PanelFlags::SEPARATOR) {
            self.draw_rect(subtle, subtle.panel_double_buffer,0, self.width, &subtle.separator_style)?;

            if let Some(text) = &self.text {
                self.draw_text(subtle, subtle.panel_double_buffer, 0, &text, &subtle.separator_style)?;
            }

        } else if self.flags.intersects(PanelFlags::TRAY) {
            self.draw_rect(subtle, subtle.panel_double_buffer, 0, self.width, &subtle.tray_style)?;
        } else if self.flags.intersects(PanelFlags::TITLE) {
            // Find focus window
            if let Some(focus_client) = subtle.find_focus_client() {
                if focus_client.is_alive() && focus_client.is_visible(subtle)
                    && !focus_client.flags.intersects(ClientFlags::TYPE_DESKTOP)
                {
                    let mut offset_x = subtle.title_style.calc_spacing(CalcSpacing::Left) as u16;

                    // Set window background and border
                    self.draw_rect(subtle, subtle.panel_double_buffer, 0,
                                   self.width, &subtle.title_style)?;

                    // Draw modes and title
                    let mode_str= focus_client.mode_string();

                    self.draw_text(subtle, subtle.panel_double_buffer, 0,
                                   &mode_str, &subtle.title_style)?;

                    // TODO: CACHE!
                    if let Some(font) = subtle.title_style.get_font(subtle) {
                        // Cache length of mode string
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &mode_str, false) {
                            offset_x += width;
                        }
                    }

                    self.draw_text(subtle, subtle.panel_double_buffer, offset_x,
                                   &focus_client.name, &subtle.title_style)?;
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

                self.pick_style(subtle, &mut style, view_idx, view);

                // Calculate view width
                let mut view_width = style.calc_spacing(CalcSpacing::Width) as u16;

                // Add space between icon and text
                if view.flags.intersects(ViewFlags::MODE_ICON_ONLY)
                    && let Some(icon) = view.icon.as_ref()
                {
                    view_width += icon.width;
                } else {
                    view_width += self.text_widths[view_idx]
                        + style.calc_spacing(CalcSpacing::Left) as u16;

                    if view.flags.intersects(ViewFlags::MODE_ICON)
                        && let Some(icon) = view.icon.as_ref()
                    {
                        view_width += icon.width + style.calc_spacing(CalcSpacing::Left) as u16;
                        offset_x += style.calc_spacing(CalcSpacing::Left) as u16;
                    }
                }

                // Draw window background and borders
                self.draw_rect(subtle, subtle.panel_double_buffer, offset_x, view_width, &style)?;

                // Draw icon
                if view.flags.intersects(ViewFlags::MODE_ICON)
                    && let Some(icon) = view.icon.as_ref()
                {
                    self.draw_icon(subtle, icon, subtle.panel_double_buffer, offset_x, &style)?;
                }

                // Draw text if necessary
                if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                    let mut icon_offset_x = 0;

                    // Add space between icon and text
                    if view.flags.intersects(ViewFlags::MODE_ICON)
                        && let Some(icon) = view.icon.as_ref()
                    {
                        icon_offset_x += icon.width
                            + style.calc_spacing(CalcSpacing::Left) as u16;
                    }

                    self.draw_text(subtle, subtle.panel_double_buffer,
                                   offset_x + icon_offset_x, &view.name, &style)?;
                }

                offset_x += max!(style.min_width as u16, view_width);

                // TODO Draw view separator if any
                //if subtle.views_style.sep_string.is_some() && view_idx < subtle.views.len() - 1 {
                //    self.draw_separator(subtle, subtle.panel_double_buffer, offset_x, &style)?;
                //
                //    offset_x += subtle.views_style.sep_width as u16;
                //}
            }
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }

    /// Handle the panel action
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `action` - Action to handle
    /// * `is_bottom` - Whether the panel is at the bottom
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
    pub(crate) fn handle_action(&self, subtle: &Subtle, action: &PanelAction, is_bottom: bool) -> Result<()> {
        if let &PanelAction::MouseDown(x, y, button) = action {

            // Check if x is in boundry box of panel
            if x >= self.x && x <= self.x + self.width as i16 {

                // Handle panel type
                if self.flags.intersects(PanelFlags::VIEWS) {
                    let mut offset_x = self.x;

                    let mut style = Style::default();

                    for (view_idx, view) in subtle.views.iter().enumerate() {
                        // Skip dynamic views
                        if view.flags.intersects(ViewFlags::MODE_DYNAMIC)
                            && !subtle.client_tags.get().intersects(view.tags)
                        {
                            continue;
                        }

                        self.pick_style(subtle, &mut style, view_idx, view);

                        let mut view_width = style.calc_spacing(CalcSpacing::Width);

                        // Add space between icon and text
                        if view.flags.intersects(ViewFlags::MODE_ICON)
                            && let Some(icon) = view.icon.as_ref()
                        {
                            view_width += icon.width as i16 + style.calc_spacing(CalcSpacing::Left);
                        }

                        if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                            view_width += self.text_widths[view_idx] as i16;
                        }


                        // Check if x is in view rect
                        if x >= offset_x && x <= offset_x + view_width {
                            view.focus(subtle, self.screen_id, true, false)?;

                            break;
                        }

                        // TODO Add view separator width if any
                        //if subtle.views_style.sep_string.is_some() {
                        //    view_width += subtle.views_style.sep_width;
                        //}

                        offset_x += view_width;
                    }
                }
            }
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }
}

impl fmt::Display for Panel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "x={}, width={}, screen_id={}, text={:?}, text_width={:?}, flags={:?})",
               self.x, self.width, self.screen_id, self.text, self.text_widths, self.flags)
    }
}

/// Clear the double buffer and init from style
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `screen` - Screen for drawing
/// * `style` - Style for clearing
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
fn clear_double_buffer(subtle: &Subtle, screen: &Screen, style: &Style) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    conn.change_gc(subtle.draw_gc, &ChangeGCAux::default().foreground(style.bg as u32))?.check()?;

    // Clear drawable
    conn.poly_fill_rectangle(subtle.panel_double_buffer, subtle.draw_gc, &[Rectangle {
        x: 0,
        y: 0,
        width: screen.base.width,
        height: subtle.panel_height
    }])?.check()?;

    Ok(())
}

/// Resize the double buffer e.g. on screen size changes
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn resize_double_buffer(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Mirror mirror: Who is the widest of them all?
    let mut width = 0;

    for screen in subtle.screens.iter() {
        if screen.base.width > width {
            width = screen.base.width;
        }
    }

    if 0 != subtle.panel_double_buffer {
        // We ignore errors here
        let _= conn.free_pixmap(subtle.panel_double_buffer);
    }

    let default_screen = &conn.setup().roots[subtle.screen_num];

    conn.create_pixmap(default_screen.root_depth, subtle.panel_double_buffer, default_screen.root,
                       width, subtle.panel_height)?.check()?;

    Ok(())
}

/// Update all panels
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn update(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    // Update screens
    for screen in subtle.screens.iter() {
        let mut selected_panel_num = 0;

        let mut default_pos = [PanelPlacement::default(); 2];
        let mut left_pos = [PanelPlacement::default(); 2];
        let mut center_pos = [PanelPlacement::default(); 2];
        let mut right_pos = [PanelPlacement::default(); 2];

        // Pass 1: Update panel items and collect width of positioned ones (left, center, right)
        for panel_idx in 0..screen.panels.len() {
            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {

                // Switch index to bottom panel
                if mut_panel.flags.intersects(PanelFlags::BOTTOM_START_MARKER) {
                    selected_panel_num = 1;
                }

                mut_panel.update(subtle)?;

                // Collect width based on position
                if mut_panel.flags.intersects(PanelFlags::LEFT_POS) {
                    left_pos[selected_panel_num].width += mut_panel.width;
                } else if mut_panel.flags.intersects(PanelFlags::CENTER_POS) {
                    center_pos[selected_panel_num].width += mut_panel.width;
                } else if mut_panel.flags.intersects(PanelFlags::RIGHT_POS) {
                    right_pos[selected_panel_num].width += mut_panel.width;
                }
            }
        }

        // Reset values before next pass
        selected_panel_num = 0;

        // Calculate start positions
        default_pos[0].offset_x = left_pos[0].width as i16;
        default_pos[1].offset_x = left_pos[1].width as i16;

        center_pos[0].offset_x = ((screen.base.width - center_pos[0].width) / 2) as i16;
        center_pos[1].offset_x = ((screen.base.width - center_pos[1].width) / 2) as i16;

        right_pos[0].offset_x = (screen.base.width - right_pos[0].width) as i16;
        right_pos[1].offset_x = (screen.base.width - right_pos[1].width) as i16;

        // Pass 2: Move and resize items
        for panel_idx in 0..screen.panels.len() {
            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {

                // Switch index to bottom panel
                if mut_panel.flags.intersects(PanelFlags::BOTTOM_START_MARKER) {
                    selected_panel_num = 1;
                }

                // Check flags only in pass 2 to allow panel updates to change flags *after* bottom toggle
                if mut_panel.flags.intersects(PanelFlags::HIDDEN) {
                    continue;
                }

                // Set panel x position
                if mut_panel.flags.intersects(PanelFlags::LEFT_POS) {
                    mut_panel.x = left_pos[selected_panel_num].offset_x;

                    left_pos[selected_panel_num].offset_x += mut_panel.width as i16;
                } else if mut_panel.flags.intersects(PanelFlags::CENTER_POS) {
                    mut_panel.x = center_pos[selected_panel_num].offset_x;

                    center_pos[selected_panel_num].offset_x += mut_panel.width as i16;
                } else if mut_panel.flags.intersects(PanelFlags::RIGHT_POS) {
                    mut_panel.x = right_pos[selected_panel_num].offset_x;

                    right_pos[selected_panel_num].offset_x += mut_panel.width as i16;
                } else {
                    mut_panel.x = default_pos[selected_panel_num].offset_x;

                    default_pos[selected_panel_num].offset_x += mut_panel.width as i16;
                };

                // Special aftercare
                if mut_panel.flags.intersects(PanelFlags::TRAY) {

                    // FIXME: Last one wins if used multiple times
                    let selected_panel_win = if 0 == selected_panel_num {
                        screen.top_panel_win
                    } else {
                        screen.bottom_panel_win
                    };

                    subtle.update_tray_win(selected_panel_win,
                                           mut_panel.x as i32, mut_panel.width as u32)?;
                }
            }
        }
    }

    debug!("{}", function_name!());

    Ok(())
}

/// Render all panels
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn render(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    // Update screens
    for screen in subtle.screens.iter() {
        let mut panel_win = screen.top_panel_win;

        clear_double_buffer(subtle, &screen, &subtle.top_panel_style)?;

        // Render panel items
        for (panel_idx, panel) in screen.panels.iter().enumerate() {

            // Switch to bottom panel
            if panel.flags.intersects(PanelFlags::BOTTOM_START_MARKER) {
                conn.copy_area(subtle.panel_double_buffer, panel_win, subtle.draw_gc,
                               0, 0, 0, 0,
                               screen.base.width, subtle.panel_height
                )?.check()?;

                clear_double_buffer(subtle, &screen, &subtle.bottom_panel_style)?;

                panel_win = screen.bottom_panel_win;
            }

            // Check hidden *after* bottom toggle
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            drop(panel);

            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {
                mut_panel.render(subtle)?;
            }
        }

        conn.copy_area(subtle.panel_double_buffer, panel_win, subtle.draw_gc,
                       0, 0, 0, 0,
                       screen.base.width, subtle.panel_height)?.check()?;
    }

    conn.flush()?;

    debug!("{}", function_name!());

    Ok(())
}
