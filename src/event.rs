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
use x11rb::connection::Connection;
use x11rb::protocol::xproto::MapRequestEvent;
use x11rb::protocol::Event;
use crate::subtle::Subtle;
use crate::client::Client;

fn handle_map_request(subtle: &Subtle, event: MapRequestEvent) {
    let _client = Client::new(subtle, event.window);
}

pub(crate) fn handle_requests(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    
    while subtle.running.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        let event = conn.wait_for_event()?;

        match event {
            Event::MapRequest(event) => handle_map_request(subtle, event),

            _ => println!("Unhandled event: {:?}", event),
        }
    }
    
    Ok(())
}