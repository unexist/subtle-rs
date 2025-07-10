use std::process;
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
use struct_iterable::Iterable;
use x11rb::connection::Connection;
use x11rb::{COPY_DEPTH_FROM_PARENT, NONE};
use x11rb::protocol::xproto::{AtomEnum, ChangeWindowAttributesAux, ConnectionExt, CreateWindowAux, EventMask, MapState, PropMode, Time, WindowClass};
use x11rb::wrapper::ConnectionExt as ConnectionWrapperExt;
use crate::{client, Config, Subtle};
use crate::client::{Client};
use crate::ewmh::AtomsCookie;
use crate::subtle::SubtleFlags;

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
        subtle.flags.insert(SubtleFlags::XINERAMA);
        
        debug!("Found xinerama extension");
    }
    
    if conn.query_extension("RANDR".as_ref())?.reply()?.present {
        subtle.flags.insert(SubtleFlags::XRANDR);

        debug!("Found xrandr extension");
    }

    conn.flush()?;

    subtle.width = conn.setup().roots[screen_num].width_in_pixels;
    subtle.height = conn.setup().roots[screen_num].height_in_pixels;
    subtle.screen_num = screen_num;
    subtle.conn.set(conn).unwrap();

    info!("Display ({}) is {}x{}", config.display, subtle.width, subtle.height);

    Ok(())
}

pub(crate) fn claim(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    let session = conn.intern_atom(false,
                                   format!("WM_S{}", subtle.screen_num).as_bytes())?.reply()?.atom;
    
    let owner = conn.get_selection_owner(session)?.reply()?.owner;
    
    if NONE != owner {
        if !subtle.flags.contains(SubtleFlags::REPLACE) {
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
    
    client::publish(subtle, false)?;

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(_subtle: &Subtle) -> Result<()> {
    Ok(())
}

pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];

    // TODO Tray

    // EWMH: Supported hints
    let mut supported_atoms: Vec<u32> = Vec::with_capacity(atoms.iter().len());

    for (field_name, field_value) in atoms.iter() {
        let maybe_atom = (&*field_value).downcast_ref::<u32>();

        if let Some(atom) = maybe_atom {
            supported_atoms.push(atom.clone());
        }
    }

    conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_SUPPORTED,
                           AtomEnum::ATOM, &supported_atoms)?.check()?;

    // EWMH: Window manager information
    let data: [u32; 1] = [subtle.support_win];

    conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_SUPPORTING_WM_CHECK,
                           AtomEnum::WINDOW, &data)?.check()?;
    conn.change_property8(PropMode::REPLACE, subtle.support_win, atoms._NET_WM_NAME,
            AtomEnum::STRING, env!("CARGO_PKG_NAME").as_bytes())?.check()?;
    conn.change_property8(PropMode::REPLACE, subtle.support_win, atoms.WM_CLASS,
                          AtomEnum::STRING, env!("CARGO_PKG_NAME").as_bytes())?.check()?;

    let data: [u32; 1] = [process::id()];

    conn.change_property32(PropMode::REPLACE, subtle.support_win, atoms._NET_WM_PID,
                           AtomEnum::CARDINAL, &data)?.check()?;

    conn.change_property8(PropMode::REPLACE, subtle.support_win, atoms.SUBTLE_VERSION,
                          AtomEnum::STRING, env!("CARGO_PKG_VERSION").as_bytes())?.check()?;

    // EWMH: Desktop geometry
    let data: [u32; 2] = [subtle.width as u32, subtle.height as u32];

    conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_DESKTOP_GEOMETRY,
                           AtomEnum::CARDINAL, &data)?.check()?;

    conn.flush()?;

    debug!("{}: views={}", function_name!(), subtle.views.len());

    Ok(())
}

pub(crate) fn finish(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;
    
    conn.destroy_window(subtle.support_win)?;

    debug!("{}", function_name!());

    Ok(())
}
