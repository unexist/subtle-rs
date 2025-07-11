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
use anyhow::{Context, Result};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::CURRENT_TIME;
use x11rb::protocol::randr::ConnectionExt as randr_ext;
use x11rb::protocol::xinerama::ConnectionExt as xinerama_ext;
use x11rb::protocol::xproto::{AtomEnum, MapState, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt;
use crate::config::Config;
use crate::subtle::SubtleFlags;
use crate::subtle::Subtle;
use crate::client::{Client, ClientFlags, WMState};
use crate::tagging::Tagging;

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct ScreenFlags: u32 {
        const PANEL1 = 1 << 0; // Screen panel1 enabled
        const PANEL2 = 1 << 1; // Screen panel2 enabled
        const VIRTUAL = 1 << 3; // Screen is virtual       
    }
}

#[derive(Default, Debug)]
pub(crate) struct Screen {
    pub(crate) flags: ScreenFlags,

    pub(crate) view_id: usize,

    pub(crate) geom: Rectangle,
    pub(crate) base: Rectangle,
}

impl Screen {
    pub(crate) fn new(subtle: &Subtle, x: i16, y: i16, width: u16, height: u16) -> Self {
        let screen = Self {
            flags: ScreenFlags::empty(),
            geom: Rectangle {
                x,
                y,
                width,
                height,
            },
            ..Self::default()
        };

        debug!("{}: {}", function_name!(), screen);

        screen
    }
}

impl fmt::Display for Screen {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "geom=(x={}, y={}, width={}, height={})",
               self.geom.x, self.geom.y, self.geom.width, self.geom.height)
    }
}

pub(crate) fn init(_config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Check both but prefer xrandr
    if subtle.flags.contains(SubtleFlags::XRANDR) {
        let screen = &conn.setup().roots[subtle.screen_num];
        let crtcs= conn.randr_get_screen_resources_current(screen.root)?.reply()?.crtcs;

        for crtc in crtcs.iter() {
            let screen_size = conn.randr_get_crtc_info(*crtc, CURRENT_TIME)?.reply()?;

            let screen = Screen::new(subtle, screen_size.x, screen_size.y, 
                                     screen_size.width, screen_size.height);

            subtle.screens.push(screen);
        }
    }

    if subtle.flags.contains(SubtleFlags::XINERAMA) && subtle.screens.is_empty() {
        if 0 != conn.xinerama_is_active()?.reply()?.state {
            let screens = conn.xinerama_query_screens()?.reply()?.screen_info;

            for screen_info in screens.iter() {
                let screen = Screen::new(subtle, screen_info.x_org, screen_info.y_org,
                                         screen_info.width, screen_info.height);

                subtle.screens.push(screen);
            }

        }
    }
    
    // Create default screen
    if subtle.screens.is_empty() {
        let screen = Screen::new(subtle, 0, 0, subtle.width, subtle.height);
        
        subtle.screens.push(screen);
    }

    publish(subtle, true)?;

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(subtle: &mut Subtle) -> Result<()> {
    let mut visible_tags = Tagging::empty();
    let mut visible_views = Tagging::empty();
    let mut client_tags = Tagging::empty();

    // Check each client
    for client in subtle.clients.iter() {
        let mut gravity_id: usize = 0;
        let mut screen_id: usize = 0;
        let mut view_id: usize = 0;
        let mut visible = 0;

        if client.flags.contains(ClientFlags::DEAD) {
            continue;
        }

        // Set available client tags to ease lookups
        client_tags.insert(client.tags);

        for (j, screen) in subtle.screens.iter().enumerate() {
            let view = &subtle.views[screen.view_id];

            // Set visible tags and views tgo ease lookups
            visible_tags.insert(view.tags);
            visible_views.insert(Tagging::from_bits_retain(1 << screen.view_id));

            if visible_tags.contains(view.tags) {
                // Keep screen when sticky
                if client.flags.contains(ClientFlags::MODE_STICK) {
                    let screen = &subtle.screens[client.screen_id];

                    screen_id = client.screen_id;
                } else {
                    screen_id = j;
                }
                
                view_id = screen.view_id;
                gravity_id = client.gravities[screen.view_id];
                visible += 1;
            }
        }
        
        // After all screens are checked..
        if 0 < visible {
            client.set_wm_state(subtle, WMState::NormalState)?;
            client.map(subtle)?;
        } else {
            client.set_wm_state(subtle, WMState::WithdrawnState)?;
            client.unmap(subtle)?;
        }
    }

    subtle.visible_tags = visible_tags;
    subtle.visible_views = visible_views;
    subtle.client_tags = client_tags;

    debug!("{}", function_name!());
    
    Ok(())
}

pub(crate) fn update(subtle: &Subtle) {
    debug!("{}", function_name!());
}


pub(crate) fn render(subtle: &Subtle) {
    debug!("{}", function_name!());
}

pub(crate) fn publish(subtle: &Subtle, publish_all: bool) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];

    if publish_all {
        let mut workareas: Vec<u32> = Vec::with_capacity(4 * subtle.screens.len());
        let mut panels: Vec<u32> = Vec::with_capacity(2 * subtle.screens.len());
        let mut viewports: Vec<u32> = Vec::with_capacity(2 * subtle.screens.len());

        for screen in subtle.screens.iter() {
            workareas.push(screen.geom.x as u32);
            workareas.push(screen.geom.y as u32);
            workareas.push(screen.geom.width as u32);
            workareas.push(screen.geom.height as u32);

            panels.push(if screen.flags.contains(ScreenFlags::PANEL1) {
                subtle.panel_height as u32 } else { 0 });
            panels.push(if screen.flags.contains(ScreenFlags::PANEL2) {
                subtle.panel_height as u32 } else { 0 });

            viewports.push(0);
            viewports.push(0);
        }

        // EWMH: Workarea
        conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_WORKAREA,
                               AtomEnum::CARDINAL, &workareas)?.check()?;

        // EWMH: Screen panels
        conn.change_property32(PropMode::REPLACE, screen.root, atoms.SUBTLE_SCREEN_PANELS,
                               AtomEnum::CARDINAL, &panels)?.check()?;

        // EWMH: Desktop viewport
        conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_DESKTOP_VIEWPORT,
                               AtomEnum::CARDINAL, &viewports)?.check()?;
    }

    let mut views: Vec<u32> = Vec::with_capacity(subtle.screens.len());

    for screen in subtle.screens.iter() {
        views.push(screen.view_id as u32);
    }

    // EWMH: Views per screen
    conn.change_property32(PropMode::REPLACE, screen.root, atoms.SUBTLE_SCREEN_VIEWS,
                           AtomEnum::CARDINAL, &views)?.check()?;

    conn.flush()?;

    debug!("{}: screens={}", function_name!(), subtle.views.len());

    Ok(())
}
