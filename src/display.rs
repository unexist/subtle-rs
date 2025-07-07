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

use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::{COPY_DEPTH_FROM_PARENT, NONE};
use x11rb::protocol::xproto::{ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, MapState, Time, WindowClass};
use crate::{Config, Subtle};
use crate::client::Client;
use crate::subtle::Flags;

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(Some(&*config.display))?;

    // Create support window
    let screen = &conn.setup().roots[screen_num];

    subtle.support_win = conn.generate_id()?;

    let aux = CreateWindowAux::default()
        .event_mask(EventMask::PROPERTY_CHANGE)
        .override_redirect(1);

    conn.create_window(COPY_DEPTH_FROM_PARENT, subtle.support_win, screen.root,
                       -100, -100, 1, 1, 0,
                       WindowClass::INPUT_OUTPUT, screen.root_visual, &aux)?;

    // Check extensions
    if conn.query_extension("XINERAMA".as_ref())?.reply()?.present {
        subtle.flags.insert(Flags::XINERAMA);
        
        debug!("Found xinerama extension");
    }
    
    if conn.query_extension("RANDR".as_ref())?.reply()?.present {
        subtle.flags.insert(Flags::XRANDR);

        debug!("Found xrandr extension");
    }
    
    conn.flush()?;

    subtle.width = conn.setup().roots[screen_num].width_in_pixels;
    subtle.height = conn.setup().roots[screen_num].height_in_pixels;
    subtle.conn.set(conn).unwrap();
    subtle.screen_num = screen_num;

    info!("Display ({}) is {}x{}", config.display, subtle.width, subtle.height);

    Ok(())
}

pub(crate) fn claim(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    let session = conn.intern_atom(false,
                                   format!("WM_S{}", subtle.screen_num).as_bytes())?.reply()?.atom;
    
    let owner = conn.get_selection_owner(session)?.reply()?.owner;
    
    if NONE != owner {
        if !subtle.flags.contains(Flags::REPLACE) {
            return Err(anyhow!("Found a running window manager"))
        }
        
        let aux = ChangeWindowAttributesAux::default()
            .event_mask(EventMask::STRUCTURE_NOTIFY);
        conn.change_window_attributes(owner, &aux)?.check()?;

        conn.flush()?;
    }

    // Acquire session selection
    conn.set_selection_owner(subtle.support_win, session, Time::CURRENT_TIME)?.check()?;
    
    let reply = conn.get_selection_owner(session)?.reply()?;
    
    if conn.get_selection_owner(session)?.reply()?.owner != subtle.support_win {
        return Err(anyhow!("Failed replacing current window manager"))
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn scan(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let screen = &conn.setup().roots[subtle.screen_num];

    for win in conn.query_tree(screen.root)?.reply()?.children {
        let attr = conn.get_window_attributes(win)?.reply()?;

        if !attr.override_redirect {
            match attr.map_state {
                MapState::VIEWABLE => {
                    let client = Client::new(subtle, win);

                    subtle.clients.push(client?);
                },
                _ => {},
            }
        }
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(_subtle: &Subtle) -> Result<()> {
    Ok(())
}

pub(crate) fn finish(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    
    conn.destroy_window(subtle.support_win)?;

    debug!("{}", function_name!());

    Ok(())
}
