///
/// @package subtle-rs
///
/// @file Screen functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use std::cell::Cell;
use bitflags::bitflags;
use log::{debug, warn};
use anyhow::{Context, Result};
use stdext::function_name;
use veccell::VecCell;
use x11rb::connection::Connection;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};
use x11rb::protocol::randr::ConnectionExt as randr_ext;
use x11rb::protocol::xinerama::ConnectionExt as xinerama_ext;
use x11rb::protocol::xproto::{AtomEnum, BackPixmap, ChangeGCAux, ConfigureWindowAux, ConnectionExt, CreateWindowAux, EventMask, Pixmap, PropMode, Rectangle, StackMode, Window, WindowClass};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::{ClientFlags, WMState};
use crate::panel::{Panel, PanelAction, PanelFlags};
use crate::style::Style;
use crate::tagging::Tagging;

bitflags! {
    #[derive(Default, Debug, Copy, Clone)]
    pub(crate) struct ScreenFlags: u32 {
        const TOP_PANEL = 1 << 0; // Screen panel1 enabled
        const BOTTOM_PANEL = 1 << 1; // Screen panel2 enabled
        const VIRTUAL = 1 << 2; // Screen is virtual
    }
}

#[derive(Debug)]
pub(crate) struct Screen {
    pub(crate) flags: ScreenFlags,

    pub(crate) view_idx: Cell<isize>,

    pub(crate) top_panel_win: Window,
    pub(crate) bottom_panel_win: Window,
    pub(crate) drawable: Pixmap,

    pub(crate) geom: Rectangle,
    pub(crate) base: Rectangle,

    pub(crate) panels: VecCell<Panel>,
}

impl Screen {
    pub(crate) fn new(subtle: &Subtle, x: i16, y: i16, width: u16, height: u16) -> Result<Self> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        let screen_size = Rectangle {
            x,
            y,
            width,
            height
        };

        let mut screen = Self {
            geom: screen_size,
            base: screen_size,
            ..Self::default()
        };

        // Create panel windows
        let default_screen = &conn.setup().roots[subtle.screen_num];

        let aux = CreateWindowAux::default()
            .event_mask(EventMask::BUTTON_PRESS
                | EventMask::ENTER_WINDOW
                | EventMask::LEAVE_WINDOW
                | EventMask::EXPOSURE)
            .override_redirect(1)
            .background_pixmap(BackPixmap::PARENT_RELATIVE);

        screen.top_panel_win = conn.generate_id()?;

        conn.create_window(COPY_DEPTH_FROM_PARENT, screen.top_panel_win, default_screen.root,
                           0, 0, 1, 1, 0,
                           WindowClass::INPUT_OUTPUT, default_screen.root_visual, &aux)?.check()?;

        screen.bottom_panel_win = conn.generate_id()?;

        conn.create_window(COPY_DEPTH_FROM_PARENT, screen.bottom_panel_win, default_screen.root,
                           0, 0, 1, 1, 0,
                           WindowClass::INPUT_OUTPUT, default_screen.root_visual, &aux)?.check()?;

        debug!("{}: screen={}", function_name!(), screen);

        Ok(screen)
    }

    pub(crate) fn clear(&self, subtle: &Subtle, style: &Style) -> Result<()> {
        let conn = subtle.conn.get().context("Failed to get connection")?;

        conn.change_gc(subtle.draw_gc, &ChangeGCAux::default().foreground(style.bg as u32))?.check()?;

        // Clear drawable
        conn.poly_fill_rectangle(self.drawable, subtle.draw_gc, &[Rectangle {
            x: 0,
            y: 0,
            width: self.base.width,
            height: subtle.panel_height
        }])?.check()?;

        Ok(())
    }

    pub(crate) fn handle_action(&self, subtle: &Subtle, action: &PanelAction, is_bottom: bool) -> Result<()> {
        for panel in self.panels.iter() {
            panel.handle_action(subtle, action, is_bottom)?;
        }

        debug!("{}: screen={}", function_name!(), self);

        Ok(())
    }
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            flags: ScreenFlags::empty(),

            view_idx: Cell::new(-1),

            top_panel_win: Window::default(),
            bottom_panel_win: Window::default(),
            drawable: 0,

            geom: Rectangle::default(),
            base: Rectangle::default(),
            panels: VecCell::new(),
        }
    }
}

impl fmt::Display for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(geom=(x={}, y={}, width={}, height={}, view_idx={}, panel_len={}, flags={:?}))",
               self.geom.x, self.geom.y, self.geom.width, self.geom.height,
               self.view_idx.get(), self.panels.len(), self.flags)
    }
}

fn configure_panel(screen: &mut Screen, panels: &Vec<String>, is_bottom: bool) {
    if !panels.is_empty() {
        // Add bottom marker to first panel on bottom panel in linear vec
        let mut flags = if is_bottom { PanelFlags::BOTTOM_MARKER } else { PanelFlags::empty() };
        let mut last_panel_idx = -1;

        for (panel_idx, panel_name) in panels.iter().enumerate() {
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
                "title" | "views" => {
                    if let Some(panel) = Panel::new(
                        PanelFlags::try_from(panel_name).unwrap_or_default() | flags)
                    {
                        screen.flags.insert(if is_bottom { ScreenFlags::BOTTOM_PANEL } else { ScreenFlags::TOP_PANEL });
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

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Check both Xinerama and xrandr, but prefer the latter
    if subtle.flags.intersects(SubtleFlags::XRANDR) {
        let default_screen = &conn.setup().roots[subtle.screen_num];
        let crtcs= conn.randr_get_screen_resources_current(default_screen.root)?.reply()?.crtcs;

        for crtc in crtcs.iter() {
            let screen_size = conn.randr_get_crtc_info(*crtc, CURRENT_TIME)?.reply()?;

            if let Ok(screen) = Screen::new(subtle, screen_size.x, screen_size.y,
                                            screen_size.width, screen_size.height)
            {
                subtle.screens.push(screen);
            }
        }
    }

    if subtle.flags.intersects(SubtleFlags::XINERAMA) && subtle.screens.is_empty() {
        if 0 != conn.xinerama_is_active()?.reply()?.state {
            let screens = conn.xinerama_query_screens()?.reply()?.screen_info;

            for screen_info in screens.iter() {
                if let Ok(screen) = Screen::new(subtle, screen_info.x_org, screen_info.y_org,
                                                screen_info.width, screen_info.height)
                {
                    subtle.screens.push(screen);
                }
            }

        }
    }
    
    // Create default screen
    if subtle.screens.is_empty() {
        if let Ok(screen) = Screen::new(subtle, 0, 0, subtle.width, subtle.height) {
            subtle.screens.push(screen);
        }
    }

    // Load screen config
    for (screen_idx, values) in config.screens.iter().enumerate() {
        if subtle.screens.len() > screen_idx
            && let Some(screen) = subtle.screens.get_mut(screen_idx)
        {
            // Handle panels
            if let Some(MixedConfigVal::VS(top_panels)) = values.get("top_panel") {
                configure_panel(screen, top_panels, false);
            }

            if let Some(MixedConfigVal::VS(bottom_panels)) = values.get("bottom_panel") {
                configure_panel(screen, bottom_panels, false);
            }

            // Handle virtual
            // TODO virtual
        }
    }

    publish(subtle, true)?;

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let mut visible_tags = Tagging::empty();
    let mut visible_views = Tagging::empty();
    let mut client_tags = Tagging::empty();

    // Either check each client or just get visible clients
    let mut clients = subtle.clients.borrow_mut();

    if 0 < clients.len() {
        // Check each client
        for client_idx in 0..clients.len() {
            let mut new_gravity_idx: isize = 0;
            let mut new_screen_idx: usize = 0;
            let mut new_view_idx: usize = 0;
            let mut visible = 0;

            let client = clients.get_mut(client_idx).unwrap();

            // Ignore dead or just iconified clients
            if client.flags.intersects(ClientFlags::DEAD) {
                continue;
            }

            // Store available client tags to ease lookups
            client_tags.insert(client.tags);

            for (screen_idx, screen) in subtle.screens.iter().enumerate() {
                if -1 != screen.view_idx.get() && let Some(view) = subtle.views.get(screen.view_idx.get() as usize) {

                    // Set visible tags and views to ease lookups
                    visible_tags.insert(view.tags);
                    visible_views.insert(Tagging::from_bits_retain(1 << screen.view_idx.get() + 1));

                    if visible_tags.intersects(client.tags) {
                        // Keep screen when sticky
                        if client.flags.intersects(ClientFlags::MODE_STICK)
                            && let Some(client_screen) = subtle.screens.get(client.screen_idx as usize)
                        {
                            new_view_idx = client_screen.view_idx.get() as usize;
                            new_screen_idx = client.screen_idx as usize;
                        } else {
                            new_view_idx = screen.view_idx.get() as usize;
                            new_screen_idx = screen_idx;
                        }

                        new_gravity_idx = client.gravities[screen.view_idx.get() as usize] as isize;
                        visible += 1;
                    }
                }
            }

            // After all screens are checked..
            if 0 < visible {
                client.arrange(subtle, new_gravity_idx, new_screen_idx as isize)?;
                client.set_wm_state(subtle, WMState::Normal)?;
                client.map(subtle)?;

                // Warp after gravity and screen have been set if not disabled
                if client.flags.intersects(ClientFlags::MODE_URGENT)
                    && !subtle.flags.intersects(SubtleFlags::SKIP_URGENT_WARP)
                    && !subtle.flags.intersects(SubtleFlags::SKIP_POINTER_WARP)
                {
                    client.warp_pointer(subtle)?;
                }

                // EWMH: Desktop, screen
                conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_WM_DESKTOP,
                                       AtomEnum::CARDINAL, &[new_view_idx as u32])?.check()?;

                conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_SCREEN,
                                       AtomEnum::CARDINAL, &[new_screen_idx as u32])?.check()?;

                // Drop and re-borrow mut this time
                client.arrange(subtle, new_gravity_idx, new_screen_idx as isize)?;
            } else {
                // Ignore next unmap
                client.flags.insert(ClientFlags::UNMAP);

                client.set_wm_state(subtle, WMState::Withdrawn)?;
                client.unmap(subtle)?;
            }
        }
    } else {
        // Check views of each screen
        for screen in subtle.screens.iter() {
            if -1 != screen.view_idx.get() && let Some(view) = subtle.views
                .get(screen.view_idx.get() as usize)
            {
                visible_tags |= view.tags;
                visible_views |= Tagging::from_bits_retain(1 << (screen.view_idx.get() + 1));
            }
        }
    }

    subtle.visible_tags.replace(visible_tags);
    subtle.visible_views.replace(visible_views);
    subtle.client_tags.replace(client_tags);

    // EWMH: Visible tags, views
    let default_screen = &conn.setup().roots[subtle.screen_num];

    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_VISIBLE_TAGS,
                           AtomEnum::CARDINAL, &[visible_tags.bits()])?.check()?;
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_VISIBLE_VIEWS,
                           AtomEnum::CARDINAL, &[visible_views.bits()])?.check()?;

    conn.flush()?;

    debug!("{}: visible_tags={:?}, visible_views={:?}, client_tags={:?}",
        function_name!(), visible_tags, visible_views, client_tags);
    
    Ok(())
}

pub(crate) fn update(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    // Update screens
    for screen in subtle.screens.iter() {
        let mut is_centered = false;
        let mut panel_number = 0;
        let mut offset = 0;

        let mut x = [0; 4];
        let mut nspacer = [0; 4];
        let mut spacer_width = [0; 4];
        let mut fix = [0; 4];
        let mut width = [0; 4];
        let mut spacer = [0; 4];

        // Pass 1: Collect width for spacer sizes
        for (panel_idx, panel) in screen.panels.iter().enumerate() {

            // Check flags
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            if 0 == panel_number && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                panel_number = 1;
                is_centered = false;
            }

            if panel.flags.intersects(PanelFlags::CENTER) {
                is_centered = !is_centered;
            }

            // Offset selects panel variables for either center or not
            offset = if is_centered { panel_number + 2 } else { panel_number };

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
        for i in 0..4 {
            if 0 < spacer[i] {
                spacer_width[i] = (screen.base.width - width[i]) / spacer[i];
                fix[i] = screen.base.width - (width[i] + spacer[i] * spacer_width[i]);
            }
        }

        // Reset values before next pass
        panel_number = 0;
        is_centered = false;

        // Pass 2: Move and resize windows
        for (panel_idx, panel) in screen.panels.iter().enumerate() {

            // Check flags
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            // Switch to bottom panel and reset
            if 0 == panel_number && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                panel_number = 1;
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
            offset = if is_centered { panel_number + 2 } else { panel_number };

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
                    x[offset] += fix[offset];
                }
            }

            // Set panel position
            if panel.flags.intersects(PanelFlags::TRAY) {
                // TODO tray
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
                    x[offset] += fix[offset];
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

        screen.clear(subtle, &subtle.top_panel_style)?;

        // Render panel items
        for (panel_idx, panel) in screen.panels.iter().enumerate() {
            if panel.flags.intersects(PanelFlags::HIDDEN) {
                continue;
            }

            // Switch to bottom panel
            if panel_win != screen.bottom_panel_win && panel.flags.intersects(PanelFlags::BOTTOM_MARKER) {
                conn.copy_area(screen.drawable, panel_win, subtle.draw_gc, 0, 0, 0, 0,
                               screen.base.width, subtle.panel_height)?.check()?;

                screen.clear(subtle, &subtle.bottom_panel_style)?;
            }

            drop(panel);

            if let Some(mut mut_panel) = screen.panels.borrow_mut(panel_idx) {
                mut_panel.render(subtle, screen.drawable)?;
            }
        }

        conn.copy_area(screen.drawable, panel_win, subtle.draw_gc, 0, 0, 0, 0,
                       screen.base.width, subtle.panel_height)?.check()?;
    }

    conn.flush()?;

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn resize(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    for screen in subtle.screens.iter_mut() {

        // Add strut
        screen.geom.x = screen.base.x + subtle.clients_style.padding.left;
        screen.geom.y = screen.base.y + subtle.clients_style.padding.top;
        screen.geom.width = (screen.base.width as i16 - subtle.clients_style.padding.left
            - subtle.clients_style.padding.right) as u16;
        screen.geom.height = (screen.base.height as i16 - subtle.clients_style.padding.top
            - subtle.clients_style.padding.bottom) as u16;

        // Update panels
        if screen.flags.intersects(ScreenFlags::TOP_PANEL) {
            let aux = ConfigureWindowAux::default()
                .x(screen.base.x as i32)
                .y(screen.base.y as i32)
                .width(screen.base.width as u32)
                .height(subtle.panel_height as u32)
                .stack_mode(StackMode::ABOVE);

            conn.configure_window(screen.top_panel_win, &aux)?.check()?;
            conn.map_window(screen.top_panel_win)?.check()?;

            // Update height
            screen.geom.y += subtle.panel_height as i16;
            screen.geom.height -= subtle.panel_height;
        } else {
            conn.unmap_window(screen.top_panel_win)?.check()?;
        }

        if screen.flags.intersects(ScreenFlags::BOTTOM_PANEL) {
            let aux = ConfigureWindowAux::default()
                .x(screen.base.x as i32)
                .y(screen.base.y as i32 + screen.base.height as i32
                    - subtle.panel_height as i32)
                .width(screen.base.width as u32)
                .height(subtle.panel_height as u32)
                .stack_mode(StackMode::ABOVE);

            conn.configure_window(screen.bottom_panel_win, &aux)?.check()?;
            conn.map_window(screen.bottom_panel_win)?.check()?;

            // Update height
            screen.geom.height -= subtle.panel_height;
        } else {
            conn.unmap_window(screen.bottom_panel_win)?.check()?;
        }

        // Re-create double buffer
        if 0 != screen.drawable {
            conn.free_pixmap(screen.drawable)?.check()?;
        }

        screen.drawable = conn.generate_id()?;

        conn.create_pixmap(default_screen.root_depth, screen.drawable, default_screen.root,
                           screen.base.width, subtle.panel_height)?.check()?;
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn publish(subtle: &Subtle, publish_all: bool) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    if publish_all {
        let mut workareas: Vec<u32> = Vec::with_capacity(4 * subtle.screens.len());
        let mut panels: Vec<u32> = Vec::with_capacity(2 * subtle.screens.len());
        let mut viewports: Vec<u32> = Vec::with_capacity(2 * subtle.screens.len());

        for screen in subtle.screens.iter() {
            workareas.push(screen.geom.x as u32);
            workareas.push(screen.geom.y as u32);
            workareas.push(screen.geom.width as u32);
            workareas.push(screen.geom.height as u32);

            panels.push(if screen.flags.intersects(ScreenFlags::TOP_PANEL) {
                subtle.panel_height as u32 } else { 0 });
            panels.push(if screen.flags.intersects(ScreenFlags::BOTTOM_PANEL) {
                subtle.panel_height as u32 } else { 0 });

            viewports.push(0);
            viewports.push(0);
        }

        // EWMH: Workarea
        conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_WORKAREA,
                               AtomEnum::CARDINAL, &workareas)?.check()?;

        // EWMH: Screen panels
        conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_SCREEN_PANELS,
                               AtomEnum::CARDINAL, &panels)?.check()?;

        // EWMH: Desktop viewport
        conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_DESKTOP_VIEWPORT,
                               AtomEnum::CARDINAL, &viewports)?.check()?;
    }

    let mut views: Vec<u32> = Vec::with_capacity(subtle.screens.len());

    for screen in subtle.screens.iter() {
        views.push(screen.view_idx.get() as u32);
    }

    // EWMH: Views per screen
    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms.SUBTLE_SCREEN_VIEWS,
                           AtomEnum::CARDINAL, &views)?.check()?;

    conn.flush()?;

    debug!("{}: screens={}", function_name!(), subtle.screens.len());

    Ok(())
}
