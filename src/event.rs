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
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ExposeEvent, MapRequestEvent, SelectionClearEvent};
use x11rb::protocol::Event;
use crate::subtle::{Flags, Subtle};
use crate::client::Client;
use crate::screen;

fn handle_expose(subtle: &Subtle, event: ExposeEvent) {
    if 0 == event.count {
        screen::render(subtle);    
    }
    
    debug!("Expose: win={}", event.window);
}

fn handle_map_request(subtle: &Subtle, event: MapRequestEvent) {
    let _client = Client::new(subtle, event.window);
}

fn handle_selection(subtle: &Subtle, event: SelectionClearEvent) {
    if event.owner == subtle.tray_win {
       debug!("Tray not supported yet"); 
    } else if event.owner == subtle.support_win {
        warn!("Leaving the field");
        subtle.running.store(false, atomic::Ordering::Relaxed);
    }
    
    debug!("SelectionClear: win={}, tray={}, support={}",
        event.owner, subtle.tray_win, subtle.support_win);
}

pub(crate) fn event_loop(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    
    while subtle.running.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        let event = conn.wait_for_event()?;

        match event {
            Event::Expose(evt) => handle_expose(subtle, evt),
            Event::MapRequest(evt) => handle_map_request(subtle, evt),
            Event::SelectionClear(evt) => handle_selection(subtle, evt),

            _ => println!("Unhandled event: {:?}", event),
        }
    }
    
    Ok(())
}