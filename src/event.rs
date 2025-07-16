///
/// @package subtle-rs
///
/// @file Display functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use anyhow::{Context, Result};
use std::sync::atomic;
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{DestroyNotifyEvent, DestroyWindowRequest, EnterNotifyEvent, ExposeEvent, MapRequestEvent, PropertyNotifyEvent, SelectionClearEvent, UnmapNotifyEvent};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::{EnterEvent, PropertyEvent};
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::{Client, ClientFlags};
use crate::{client, screen};

fn handle_enter(subtle: &Subtle, event: EnterNotifyEvent) {
    if let Some(mut client) = subtle.find_client_mut(event.event) {
        if !subtle.flags.contains(SubtleFlags::FOCUS_CLICK) {
            let _ = client.focus(subtle, false);
        }
    }

    if let Some(client) = subtle.focus_history.borrow_mut(0) {

    }

    debug!("{}: win={}, root={}", function_name!(), event.event, event.root);
}

fn handle_expose(subtle: &Subtle, event: ExposeEvent) {
    if 0 == event.count {
        screen::render(subtle);    
    }
    
    debug!("{}: win={}, count={}", function_name!(), event.window, event.count);
}

fn handle_destroy(subtle: &Subtle, event: DestroyNotifyEvent) {
    debug!("{}: win={}", function_name!(), event.window);
}

fn handle_property(subtle: &Subtle, event: PropertyNotifyEvent) {
    let atoms = subtle.atoms.get().unwrap();

    if atoms.WM_NAME == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let _ = client.set_wm_name(subtle);

            if let Some(win) = subtle.focus_history.borrow(0)
                && event.window == *win
            {
                screen::update(subtle);
                screen::render(subtle);
            }
        }
    } else if atoms.WM_NORMAL_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            let _ = client.set_size_hints(subtle, &mut mode_flags);

            let mut enable_only = client.flags.complement().intersection(mode_flags);
            let _ = client.toggle(subtle, &mut enable_only, true);

            if client.is_visible(subtle) {
                screen::update(subtle);
                screen::render(subtle);
            }

        }
        // TODO tray
    } else if atoms.WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            let _ = client.set_wm_hints(subtle, &mut mode_flags);

            let mut enable_only = client.flags.complement().intersection(mode_flags);
            let _ = client.toggle(subtle, &mut enable_only, true);

            if client.is_visible(subtle) || client.flags.contains(ClientFlags::MODE_URGENT) {
                screen::update(subtle);
                screen::render(subtle);
            }
        }
    } else if atoms._NET_WM_STRUT == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let _ = client.set_strut(subtle);

            screen::update(subtle);
        }
    } else if atoms._MOTIF_WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            let mut enable_only = client.flags.complement().intersection(mode_flags);
            let _ = client.toggle(subtle, &mut enable_only, true);

            let _= client.set_motif_wm_hints(subtle, &mut mode_flags);
        }
    }
    // TODO tray

    debug!("{}: win={}, atom={}", function_name!(), event.window, event.atom);
}

fn handle_map_request(subtle: &Subtle, event: MapRequestEvent) {
    // Check if we know the window
    let client = subtle.find_client(event.window);

    if client.is_some() {
        screen::render(subtle);
    } else {
        let _map_client = Client::new(subtle, event.window);
    }

    debug!("{}: win={}", function_name!(), event.window);
}

fn handle_unmap(subtle: &Subtle, event: UnmapNotifyEvent) {
    debug!("{}: win={}", function_name!(), event.window);
}

fn handle_selection(subtle: &Subtle, event: SelectionClearEvent) {
    if event.owner == subtle.tray_win {
        debug!("Tray not supported yet");
    } else if event.owner == subtle.support_win {
        warn!("Leaving the field");
        subtle.exterminate.store(false, atomic::Ordering::Relaxed);
    }
    
    debug!("{}: win={}, tray={}, support={}",
        function_name!(), event.owner, subtle.tray_win, subtle.support_win);
}

pub(crate) fn event_loop(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Update screen and panels
    screen::configure(subtle)?;
    screen::update(subtle);
    screen::render(subtle);

    conn.flush()?;

    // Set grabs and focus first client if any
    //sub_GrabSet(ROOT, SUB_GRAB_KEY) // TODO grabs

    if let Some(mut client) = client::find_next(subtle, 0, false) {
        client.focus(subtle, true)?;
    }

    while !subtle.exterminate.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        if let Some(event) = conn.poll_for_event()? {
            match event {
                Event::EnterNotify(evt) => handle_enter(subtle, evt),
                Event::Expose(evt) => handle_expose(subtle, evt),
                Event::DestroyNotify(evt) => handle_destroy(subtle, evt),
                Event::MapRequest(evt) => handle_map_request(subtle, evt),
                Event::PropertyNotify(evt) => handle_property(subtle, evt),
                Event::SelectionClear(evt) => handle_selection(subtle, evt),
                Event::UnmapNotify(evt) => handle_unmap(subtle, evt),

                _ => println!("Unhandled event: {:?}", event),
            }
        }
    }
    
    Ok(())
}