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
use x11rb::protocol::xproto::{AtomEnum, PropMode, Rectangle};
use x11rb::wrapper::ConnectionExt;
use crate::config::Config;
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::{ClientFlags, WMState};
use crate::tagging::Tagging;

bitflags! {
    #[derive(Default, Debug, Copy, Clone)]
    pub(crate) struct ScreenFlags: u32 {
        const PANEL1 = 1 << 0; // Screen panel1 enabled
        const PANEL2 = 1 << 1; // Screen panel2 enabled
        const VIRTUAL = 1 << 3; // Screen is virtual       
    }
}

#[derive(Default, Debug)]
pub(crate) struct Screen {
    pub(crate) flags: ScreenFlags,

    pub(crate) view_id: isize,

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

pub(crate) fn configure(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let mut visible_tags = Tagging::empty();
    let mut visible_views = Tagging::empty();
    let mut client_tags = Tagging::empty();

    // Either check each client or just get visible clients
    if 0 < subtle.clients.len() {
        // Check each client
        for (client_idx, client) in subtle.clients.iter().enumerate() {
            let mut gravity_id: isize = 0;
            let mut screen_id: usize = 0;
            let mut view_id: usize = 0;
            let mut visible = 0;

            // Ignore dead or just iconified clients
            if client.flags.contains(ClientFlags::DEAD) {
                continue;
            }

            // Set available client tags to ease lookups
            client_tags.insert(client.tags);

            for (screen_idx, screen) in subtle.screens.iter().enumerate() {
                if -1 != screen.view_id && let Some(view) = subtle.views.get(screen.view_id as usize) {

                    // Set visible tags and views tgo ease lookups
                    visible_tags.insert(view.tags);
                    visible_views.insert(Tagging::from_bits_retain(1 << screen.view_id));

                    if visible_tags.contains(view.tags) {
                        // Keep screen when sticky
                        if client.flags.contains(ClientFlags::MODE_STICK)
                            && let Some(client_screen) = subtle.screens.get(client.screen_id as usize)
                        {
                            view_id = client_screen.view_id as usize;
                            screen_id = client.screen_id as usize;
                        } else {
                            view_id = screen.view_id as usize;
                            screen_id = screen_idx;
                        }

                        gravity_id = client.gravities[screen.view_id as usize] as isize;
                        visible += 1;
                    }
                }
            }

            // After all screens are checked..
            if 0 < visible {
                client.set_wm_state(subtle, WMState::NormalState)?;
                client.map(subtle)?;

                // Warp after gravity and screen have been set if not disabled
                if client.flags.contains(ClientFlags::MODE_URGENT)
                    && !subtle.flags.contains(SubtleFlags::SKIP_URGENT_WARP)
                    && !subtle.flags.contains(SubtleFlags::SKIP_POINTER_WARP)
                {
                    client.warp(subtle)?;
                }

                // EWMH: Desktop, screen
                conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_WM_DESKTOP,
                                       AtomEnum::CARDINAL, &[view_id as u32])?.check()?;

                conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_SCREEN,
                                       AtomEnum::CARDINAL, &[screen_id as u32])?.check()?;

                // Drop and re-borrow mut this time
                drop(client);

                if let Some(mut mut_client) = subtle.clients.borrow_mut(client_idx) {
                    mut_client.arrange(subtle, gravity_id, screen_id as isize)?;
                }
            } else {
                client.set_wm_state(subtle, WMState::WithdrawnState)?;
                client.unmap(subtle)?;

                // Drop and re-borrow mut this time
                drop(client);

                if let Some(mut mut_client) = subtle.clients.borrow_mut(client_idx) {
                    mut_client.flags.insert(ClientFlags::UNMAP);
                }
            }
        }
    } else {
        // Check views of each screen
        for screen in subtle.screens.iter() {
            if -1 != screen.view_id && let Some(view) = subtle.views.get(screen.view_id as usize) {
                visible_tags |= view.tags;
                visible_views |= Tagging::from_bits_retain(1 << (screen.view_id + 1));
            }
        }
    }

    subtle.visible_tags.replace(visible_tags);
    subtle.visible_views.replace(visible_views);
    subtle.client_tags.replace(client_tags);

    // EWMH: Visible tags, views
    let screen = &conn.setup().roots[subtle.screen_num];

    conn.change_property32(PropMode::REPLACE, screen.root, atoms.SUBTLE_VISIBLE_TAGS,
                           AtomEnum::CARDINAL, &[visible_tags.bits()])?.check()?;
    conn.change_property32(PropMode::REPLACE, screen.root, atoms.SUBTLE_VISIBLE_VIEWS,
                           AtomEnum::CARDINAL, &[visible_views.bits()])?.check()?;

    conn.flush()?;

    debug!("{}: visible_tags={:?}, visible_views={:?}, client_tags={:?}",
        function_name!(), visible_tags, visible_views, client_tags);
    
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
