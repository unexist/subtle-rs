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
use log::debug;
use anyhow::{Context, Result};
use stdext::function_name;
use veccell::VecCell;
use x11rb::connection::Connection;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME};
use x11rb::protocol::randr::ConnectionExt as randr_ext;
use x11rb::protocol::xinerama::ConnectionExt as xinerama_ext;
use x11rb::protocol::xproto::{AtomEnum, BackPixmap, ConfigureWindowAux, ConnectionExt, CreateWindowAux, EventMask, PropMode, Rectangle, StackMode, Window, WindowClass};
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::config::{Config, MixedConfigVal};
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::ClientFlags;
use crate::ewmh::WMState;
use crate::panel;
use crate::panel::{Panel, PanelAction};
use crate::tagging::Tagging;

bitflags! {
    /// Config and state-flags for [`Screen`]
    #[derive(Default, Debug, Copy, Clone)]
    pub(crate) struct ScreenFlags: u32 {
        /// Screen panel1 enabled
        const TOP_PANEL = 1 << 0;
        /// Screen panel2 enabled
        const BOTTOM_PANEL = 1 << 1;
        /// Screen is virtual
        const VIRTUAL = 1 << 2;
    }
}

#[derive(Debug)]
pub(crate) struct Screen {
    pub(crate) flags: ScreenFlags,

    pub(crate) view_idx: Cell<isize>,

    pub(crate) top_panel_win: Window,
    pub(crate) bottom_panel_win: Window,

    pub(crate) geom: Rectangle,
    pub(crate) base: Rectangle,

    pub(crate) panels: VecCell<Panel>,
}

impl Screen {
    /// Create a new instance
    ///
    /// # Arguments
    ///
    /// * `subtle` - Global state object
    /// * `x` - X position
    /// * `y` - Y position
    /// * `width` - Width of the screen
    /// * `height` - Height of the Screen
    ///
    /// # Returns
    ///
    /// A [`Result`] with either [`Screen`] on success or otherwise [`anyhow::Error`]
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

/// Check config and init all screen related options
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
                if !top_panels.is_empty() {
                    panel::parse(screen, top_panels, false);

                    screen.flags.insert(ScreenFlags::TOP_PANEL);
                }
            }

            if let Some(MixedConfigVal::VS(bottom_panels)) = values.get("bottom_panel") {
                if !bottom_panels.is_empty() {
                    panel::parse(screen, bottom_panels, true);

                    screen.flags.insert(ScreenFlags::BOTTOM_PANEL);
                }
            }

            // Handle virtual
            // TODO virtual
        }
    }

    publish(subtle, true)?;

    debug!("{}", function_name!());

    Ok(())
}

/// Publish and export all relevant atoms to allow IPC
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn configure(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let mut visible_tags = Tagging::empty();
    let mut visible_views = Tagging::empty();
    let mut client_tags = Tagging::empty();

    // Either check each client or just get visible clients
    let mut clients = subtle.clients.borrow_mut();

    // Check each client
    for client_idx in 0..clients.len() {
        let mut new_gravity_idx: isize = 0;
        let mut new_screen_idx: usize = 0;
        let mut new_view_idx: usize = 0;
        let mut visible = 0;

        if let Some(client) = clients.get_mut(client_idx) {

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

                    if visible_tags.intersects(client.tags) ||
                        client.flags.intersects(ClientFlags::MODE_STICK | ClientFlags::TYPE_DESKTOP)
                    {
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

                client.arrange(subtle, new_gravity_idx, new_screen_idx as isize)?;
            } else {
                // Ignore next unmap
                client.flags.insert(ClientFlags::UNMAP);

                client.set_wm_state(subtle, WMState::Withdrawn)?;
                client.unmap(subtle)?;
            }
        }
    }

    if clients.is_empty() {
        // Check views of each screen
        for screen in subtle.screens.iter() {
            if -1 != screen.view_idx.get()
                && let Some(view) = subtle.views.get(screen.view_idx.get() as usize)
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
    }

    panel::resize_double_buffer(subtle)?;

    debug!("{}", function_name!());

    Ok(())
}

/// Publish and export all relevant atoms to allow IPC
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `publish_all` - Whether to publish all atoms
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
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
