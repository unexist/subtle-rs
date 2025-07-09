use std::ptr::fn_addr_eq;
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
use x11rb::protocol::xproto::{DestroyNotifyEvent, DestroyWindowRequest, ExposeEvent, MapRequestEvent, PropertyNotifyEvent, SelectionClearEvent, UnmapNotifyEvent};
use x11rb::protocol::Event;
use x11rb::protocol::xinput::PropertyEvent;
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::Client;
use crate::screen;

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
    debug!("{}: win={}", function_name!(), event.window);
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
    
    while !subtle.exterminate.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        if let Some(event) = conn.poll_for_event()? {
            match event {
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