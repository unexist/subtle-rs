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

use anyhow::{anyhow, Result};
use log::{error, info};
use x11rb::connection::Connection;
use x11rb::COPY_DEPTH_FROM_PARENT;
use x11rb::protocol::randr::select_input;
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, Time, WindowClass};
use crate::{Config, Subtle};
use crate::subtle::Flags;

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(Some(&*config.display))?;

    // Create support window
    let screen = &conn.setup().roots[screen_num];

    subtle.support = conn.generate_id()?;

    let aux = CreateWindowAux::default()
        .event_mask(EventMask::PROPERTY_CHANGE)
        .override_redirect(1);

    conn.create_window(COPY_DEPTH_FROM_PARENT, subtle.support, screen.root,
                       -100, -100, 1, 1, 0,
                       WindowClass::INPUT_OUTPUT, screen.root_visual, &aux)?;

    subtle.width = conn.setup().roots[screen_num].width_in_pixels;
    subtle.height = conn.setup().roots[screen_num].height_in_pixels;
    subtle.conn = Option::from(conn);
    subtle.screen_num = screen_num;

    info!("Display ({}) is {}x{}", config.display, subtle.width, subtle.height);

    Ok(())
}

pub(crate) fn claim(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.as_mut().ok_or(anyhow!("No connection"))?;
    let session = conn.intern_atom(false,
                                   format!("WM_S{}", subtle.screen_num).as_bytes())?.reply()?.atom;

    let owner = conn.get_selection_owner(session)?.reply()?.owner;
    
    if 0 != owner {
        if !subtle.flags.contains(Flags::REPLACE) {
            return Err(anyhow!("Found a running window manager"))
        }
        
        let aux = ChangeWindowAttributesAux::default()
            .event_mask(EventMask::STRUCTURE_NOTIFY);
        conn.change_window_attributes(owner, &aux)?.check()?;

        conn.flush()?;
    }

    conn.set_selection_owner(session, subtle.support, Time::CURRENT_TIME)?;

    if conn.get_selection_owner(session)?.reply()?.owner != subtle.support {
        return Err(anyhow!("Failed replacing current window manager"))
    }

    Ok(())
}

pub(crate) fn configure(_subtle: &Subtle) -> Result<()> {
    Ok(())
}

pub(crate) fn finish(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.as_mut().ok_or(anyhow!("No connection"))?;
    
    conn.destroy_window(subtle.support)?;
    
    Ok(())
}
