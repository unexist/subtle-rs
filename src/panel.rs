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
use anyhow::{anyhow, Context, Result};
use easy_min_max::max;
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ChangeGCAux, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, Drawable, Rectangle, StackMode};
use crate::client::ClientFlags;
use crate::icon::Icon;
use crate::screen::{Screen, ScreenFlags};
use crate::style::{CalcSpacing, Style, StyleFlags};
use crate::subtle::Subtle;
use crate::tagging::Tagging;
use crate::tray::TrayFlags;
use crate::view::{View, ViewFlags};

const ICON_TEXT_SPACING: u16 = 3;

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

impl TryFrom<&String> for PanelFlags {
    type Error = anyhow::Error;

    fn try_from(value: &String) -> Result<PanelFlags, Self::Error> {
        match value.as_str() {
            "title" => Ok(PanelFlags::TITLE),
            "views" => Ok(PanelFlags::VIEWS),
            "tray" => Ok(PanelFlags::TRAY),
            _ => Err(anyhow!("Invalid type for panel")),
        }
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
    pub(crate) text_widths: Vec<u16>,
}

impl Panel {
    fn pick_style(&self, subtle: &&Subtle, style: &mut Style, view_idx: usize, view: &View) {
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

    fn draw_rect(&self, subtle: &Subtle, drawable: Drawable, offset_x: u16, width: u16, style: &Style) -> Result<()> {
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

    fn draw_icon(&self, subtle: &Subtle, icon: &Icon, drawable: Drawable, offset_x: u16, style: &Style) -> Result<()>
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

    pub(crate) fn new(flags: PanelFlags) -> Option<Self> {
        let mut panel = Self {
            flags,
            ..Self::default()
        };

        if flags.intersects(PanelFlags::TITLE) {
            panel.text_widths.resize(2, Default::default());
        } else if flags.intersects(PanelFlags::VIEWS) {
            panel.flags.insert(PanelFlags::MOUSE_DOWN);
        } else if !flags.intersects(PanelFlags::TRAY) {
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
            self.width = subtle.tray_style.calc_spacing(CalcSpacing::Width) as u16;

            // Resize every tray
            if let Ok(mut trays) = subtle.trays.try_borrow_mut() && !trays.is_empty() {
                for tray_idx in 0..trays.len() {
                    let tray = trays.get_mut(tray_idx).unwrap();

                    if tray.flags.intersects(TrayFlags::DEAD) {
                        continue;
                    }

                    conn.map_window(tray.win)?.check()?;

                    let aux = &ConfigureWindowAux::default()
                        .x(self.width as i32)
                        .y(0i32)
                        .width(max!(1, tray.width) as u32)
                        .height(max!(1, subtle.panel_height as i16
                            - subtle.tray_style.calc_spacing(CalcSpacing::Height)) as u32)
                        .stack_mode(StackMode::ABOVE);

                    conn.configure_window(tray.win, &aux)?.check()?;

                    self.width += tray.width;
                }
            } else {
                conn.unmap_window(subtle.tray_win)?.check()?;
            }
        } else if self.flags.intersects(PanelFlags::TITLE) {

            // Find focus window
            if let Some(focus_client) = subtle.find_focus_client() {
                if focus_client.is_alive() && !focus_client.flags.intersects(ClientFlags::TYPE_DESKTOP) {
                    let mode_str = focus_client.mode_string();

                    // Font offset, panel border and padding
                    if let Some(font) = subtle.title_style.get_font(subtle) {
                        // Cache length of mode string
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &mode_str, false) {
                            self.text_widths[0] = width;
                        }

                        // Cache length of actual title
                        if let Ok((width, _, _)) = font.calc_text_width(conn, &focus_client.name, false) {
                            self.text_widths[1] = width;
                        }

                        // Finally update actual length
                        self.width = self.text_widths[0]
                            + max!(subtle.clients_style.right as u16, self.text_widths[1])
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

                self.pick_style(&subtle, &mut style, view_idx, view);

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

                        view_width += self.text_widths[view_idx];

                        if view.flags.intersects(ViewFlags::MODE_ICON)
                            && let Some(icon) = view.icon.as_ref()
                        {
                            view_width += icon.width + ICON_TEXT_SPACING;
                        }
                    }
                }

                // Ensure min-width
                self.width += max!(style.min_width as u16, view_width);
            }

            // Add width of view separator if any
            if subtle.views_style.sep_string.is_some() {
                self.width += (subtle.views.len() - 1) as u16 * subtle.views_style.sep_width as u16;
            }
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn render(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        // Draw separator before panel item
        if self.flags.intersects(PanelFlags::SEPARATOR_BEFORE)
            && subtle.separator_style.flags.intersects(StyleFlags::SEPARATOR)
        {
            self.draw_separator(subtle, subtle.panel_double_buffer,
                                subtle.separator_style.sep_width as u16, &subtle.separator_style)?;
        }

        // Handle panel item type
        if self.flags.intersects(PanelFlags::ICON) {
            todo!(); // TODO icon
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
                    self.draw_rect(subtle, subtle.panel_double_buffer, 0, self.width, &subtle.title_style)?;

                    // Draw modes and title
                    let mode_str= focus_client.mode_string();

                    self.draw_text(subtle, subtle.panel_double_buffer, 0, &mode_str, &subtle.title_style)?;

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

                self.pick_style(&subtle, &mut style, view_idx, view);

                // Calculate view width
                let mut view_width= style.calc_spacing(CalcSpacing::Width) as u16;

                // Add space between icon and text
                if view.flags.intersects(ViewFlags::MODE_ICON)
                    && let Some(icon) = view.icon.as_ref()
                {
                    view_width += icon.width + ICON_TEXT_SPACING;
                }

                if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                    view_width += self.text_widths[view_idx];
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
                        icon_offset_x += icon.width + ICON_TEXT_SPACING;
                    }

                    self.draw_text(subtle, subtle.panel_double_buffer, offset_x + icon_offset_x,
                                   &view.name, &style)?;
                }

                offset_x += max!(style.min_width as u16, view_width);

                // Draw view separator if any
                if subtle.views_style.sep_string.is_some() && view_idx < subtle.views.len() - 1 {
                    self.draw_separator(subtle, subtle.panel_double_buffer, offset_x, &style)?;

                    offset_x += subtle.views_style.sep_width as u16;
                }
            }
        }

        // Draw separator after panel item
        if self.flags.intersects(PanelFlags::SEPARATOR_AFTER)
            && subtle.separator_style.flags.intersects(StyleFlags::SEPARATOR)
        {
            self.draw_separator(subtle, subtle.panel_double_buffer,
                                self.width, &subtle.separator_style)?;
        }

        debug!("{}: panel={}", function_name!(), self);

        Ok(())
    }

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

                        self.pick_style(&subtle, &mut style, view_idx, view);

                        let mut view_width = style.calc_spacing(CalcSpacing::Width);

                        // Add space between icon and text
                        if view.flags.intersects(ViewFlags::MODE_ICON)
                            && let Some(icon) = view.icon.as_ref()
                        {
                            view_width += (icon.width + ICON_TEXT_SPACING) as i16;
                        }

                        if !view.flags.intersects(ViewFlags::MODE_ICON_ONLY) {
                            view_width += self.text_widths[view_idx] as i16;
                        }


                        // Check if x is in view rect
                        if x >= offset_x && x <= offset_x + view_width {
                            view.focus(subtle, self.screen_id, true, false)?;

                            break;
                        }

                        // Add view separator width if any
                        if subtle.views_style.sep_string.is_some() {
                            view_width += subtle.views_style.sep_width;
                        }

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
        write!(f, "x={}, width={}, screen_id={}, flags={:?})",
               self.x, self.width, self.screen_id, self.flags)
    }
}


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

pub(crate) fn parse(screen: &mut Screen, panel_list: &Vec<String>, is_bottom: bool) {
    if !panel_list.is_empty() {
        let mut flags = PanelFlags::empty();
        let mut last_panel_idx = -1;

        // Add bottom marker to first panel on bottom panel in linear vec
        if is_bottom {
            flags = PanelFlags::BOTTOM_MARKER;
        }

        for (panel_idx, panel_name) in panel_list.iter().enumerate() {
            // Lookbehind to previous panel
            if flags.intersects(PanelFlags::SPACER_BEFORE | PanelFlags::SEPARATOR_BEFORE) {
                if -1 != last_panel_idx &&
                    let Some(mut last_panel) = screen.panels.borrow_mut(last_panel_idx as usize)
                {
                    // Add spacer after panel item
                    if flags.intersects(PanelFlags::SPACER_BEFORE) {
                        last_panel.flags.insert(PanelFlags::SPACER_BEFORE);
                        flags.remove(PanelFlags::SPACER_BEFORE);
                    }

                    // Add separator after panel item
                    if flags.intersects(PanelFlags::SEPARATOR_BEFORE) {
                        last_panel.flags.insert(PanelFlags::SEPARATOR_BEFORE);
                        flags.remove(PanelFlags::SEPARATOR_BEFORE);
                    }
                }
            }

            // Handle panel type
            match panel_name.as_str() {
                "spacer" => flags.insert(PanelFlags::SPACER_BEFORE),
                "separator" => flags.insert(PanelFlags::SEPARATOR_BEFORE),
                "center" => flags.insert(PanelFlags::CENTER),
                "title" | "views" | "tray" => {
                    if let Some(panel) = Panel::new(
                        PanelFlags::try_from(panel_name).unwrap_or_default() | flags)
                    {
                        if is_bottom {
                            screen.flags.insert(ScreenFlags::BOTTOM_PANEL);
                        } else {
                            screen.flags.insert(ScreenFlags::TOP_PANEL);
                        }

                        screen.panels.push(panel);
                        flags.remove(PanelFlags::BOTTOM_MARKER);

                        last_panel_idx += 1;
                    }
                },
                _ => warn!("Unknown panel type: {}", panel_name),
            }
        }

        // Add flags to last item
        if -1 != last_panel_idx
            && let Some(mut last_panel) = screen.panels.borrow_mut(last_panel_idx as usize)
        {
            if flags.intersects(PanelFlags::SPACER_BEFORE) {
                last_panel.flags.insert(PanelFlags::SPACER_AFTER);
            }

            if flags.intersects(PanelFlags::SEPARATOR_BEFORE) {
                last_panel.flags.insert(PanelFlags::SEPARATOR_AFTER);
            }
        }
    }
}

pub(crate) fn update(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    // Update screens
    for screen in subtle.screens.iter() {
        let mut is_centered = false;
        let mut selected_panel_num = 0;
        let mut offset = 0;

        let mut x = [0; 4];
        let mut width = [0; 4];
        let mut nspacer = [0; 4];
        let mut spacer_width = [0; 4];
        let mut rounding_fix = [0; 4];
        let mut spacer = [0; 4];

        // Pass 1: Collect width for spacer sizes
        for (panel_idx, panel) in screen.panels.iter().enumerate() {

            // Check flags
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            if 0 == selected_panel_num && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                selected_panel_num = 1;
                is_centered = false;
            }

            if panel.flags.intersects(PanelFlags::CENTER) {
                is_centered = !is_centered;
            }

            // Offset selects panel variables for either center or not
            offset = if is_centered { selected_panel_num + 2 } else { selected_panel_num };

            if panel.flags.intersects(PanelFlags::SPACER_BEFORE) {
                spacer[offset] += 1;
            }

            if panel.flags.intersects(PanelFlags::SPACER_AFTER) {
                spacer[offset] += 1;
            }

            if panel.flags.intersects(PanelFlags::SEPARATOR_BEFORE) {
                width[offset] += subtle.separator_style.sep_width as u16;
            }

            if panel.flags.intersects(PanelFlags::SEPARATOR_AFTER) {
                width[offset] += subtle.separator_style.sep_width as u16;
            }

            // Drop and update panel item
            drop(panel);

            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {
                mut_panel.update(subtle)?;

                width[offset] += mut_panel.width;
            }
        }

        // Calculate spacer and fix sizes
        for i in 0..spacer.len() {
            if 0 < spacer[i] {
                spacer_width[i] = (screen.base.width - width[i]) / spacer[i];
                rounding_fix[i] = screen.base.width - (width[i] + spacer[i] * spacer_width[i]);
            }
        }

        // Reset values before next pass
        selected_panel_num = 0;
        is_centered = false;

        // Pass 2: Move and resize windows
        for (panel_idx, panel) in screen.panels.iter().enumerate() {

            // Check flags
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            // Switch to bottom panel and reset
            if 0 == selected_panel_num && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                selected_panel_num = 1;
                nspacer[0] = 0;
                nspacer[2] = 0;
                x[0] = 0;
                x[2] = 0;
                is_centered = false;
            }

            if panel.flags.intersects(PanelFlags::CENTER) {
                is_centered = !is_centered;
            }

            // Offset select panels variables for either center or not
            offset = if is_centered { selected_panel_num + 2 } else { selected_panel_num };

            // Set start position of centered panel items
            if is_centered && 0 == x[offset] {
                x[offset] = (screen.base.width - width[offset]) / 2;
            }

            // Add separator before panel item
            if panel.flags.intersects(PanelFlags::SEPARATOR_BEFORE) {
                x[offset] += subtle.separator_style.sep_width as u16;
            }

            // Add spacer before item
            if panel.flags.intersects(PanelFlags::SPACER_BEFORE) {
                x[offset] += spacer_width[offset];

                // Increase last spacer size by rounding fix
                nspacer[offset] += 1;

                if nspacer[offset] == spacer[offset] {
                    x[offset] += rounding_fix[offset];
                }
            }

            // Set panel position
            if panel.flags.intersects(PanelFlags::TRAY) {

                // FIXME: Last one wins if used multiple times
                let selected_panel_win = if 0 == selected_panel_num {
                    screen.top_panel_win
                } else {
                    screen.bottom_panel_win
                };

                conn.reparent_window(subtle.tray_win, selected_panel_win, 0, 0,)?.check()?;

                let aux = ChangeWindowAttributesAux::default()
                    .background_pixel(subtle.tray_style.bg as u32);

                conn.change_window_attributes(subtle.tray_win, &aux)?.check()?;

                let aux = ConfigureWindowAux::default()
                    .x(x[offset] as i32 + subtle.tray_style.calc_spacing(CalcSpacing::Left) as i32)
                    .y(subtle.tray_style.calc_spacing(CalcSpacing::Top) as i32)
                    .width(max!(1, panel.width as u32
                        - subtle.tray_style.calc_spacing(CalcSpacing::Width) as u32))
                    .height(max!(1, subtle.panel_height as u32
                        - subtle.tray_style.calc_spacing(CalcSpacing::Height) as u32))
                    .stack_mode(StackMode::ABOVE);

                conn.configure_window(subtle.tray_win, &aux)?.check()?;
                conn.map_subwindows(selected_panel_win)?.check()?;

                println!("reparent + configure");
            }

            // Store x position before separator and spacer for later re-borrow
            let panel_x = x[offset];

            // Add separator after item
            if panel.flags.intersects(PanelFlags::SEPARATOR_AFTER) {
                x[offset] += subtle.separator_style.sep_width as u16;
            }

            // Add spacer after item
            if panel.flags.intersects(PanelFlags::SPACER_AFTER) {
                x[offset] += spacer_width[offset];

                // Increase last spacer size by rounding fix
                nspacer[offset] += 1;

                if nspacer[offset] == spacer[offset] {
                    x[offset] += rounding_fix[offset];
                }
            }

            x[offset] += panel.width;

            // Drop and update panel item
            drop(panel);

            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {
                mut_panel.x = panel_x as i16;
            }
        }
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn render(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    // Update screens
    for screen in subtle.screens.iter() {
        let panel_win = screen.top_panel_win;

        clear_double_buffer(subtle, &screen, &subtle.top_panel_style)?;

        // Render panel items
        for (panel_idx, panel) in screen.panels.iter().enumerate() {
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            // Switch to bottom panel
            if panel_win != screen.bottom_panel_win && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                conn.copy_area(subtle.panel_double_buffer, panel_win, subtle.draw_gc, 0, 0, 0, 0,
                               screen.base.width, subtle.panel_height
                )?.check()?;

                clear_double_buffer(subtle, &screen, &subtle.bottom_panel_style)?;
            }

            drop(panel);

            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {
                mut_panel.render(subtle)?;
            }
        }

        conn.copy_area(subtle.panel_double_buffer, panel_win, subtle.draw_gc, 0, 0, 0, 0,
                       screen.base.width, subtle.panel_height)?.check()?;
    }

    conn.flush()?;

    debug!("{}", function_name!());

    Ok(())
}
