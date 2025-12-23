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

use std::process;
use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use stdext::function_name;
use struct_iterable::Iterable;
use x11rb::connection::Connection;
use x11rb::{COPY_DEPTH_FROM_PARENT, CURRENT_TIME, NONE};
use x11rb::protocol::xproto::{AtomEnum, CapStyle, ChangeWindowAttributesAux, ConnectionExt, CreateGCAux, CreateWindowAux, EventMask, FillStyle, FontWrapper, InputFocus, JoinStyle, LineStyle, MapState, PropMode, SubwindowMode, Time, WindowClass, GX};
use x11rb::wrapper::ConnectionExt as ConnectionWrapperExt;
use crate::{client, ewmh, Config, Subtle};
use crate::client::Client;
use crate::subtle::SubtleFlags;

// Taken from /usr/include/X11/cursorfont.h
const XC_LEFT_PTR: u16 = 68;
const XC_DOTBOX: u16 = 40;
const XC_SIZING: u16 = 120;

/// Check config and init all display related options
///
/// # Arguments
///
/// * `config` - Config values read either from args or config file
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let (conn, screen_num) = x11rb::connect(Some(&*config.display))?;

    let default_screen = &conn.setup().roots[screen_num];

    // Create support window
    subtle.support_win = conn.generate_id()?;

    let aux = CreateWindowAux::default()
        .event_mask(EventMask::PROPERTY_CHANGE)
        .override_redirect(1);

    conn.create_window(COPY_DEPTH_FROM_PARENT, subtle.support_win, default_screen.root,
                       -100, -100, 1, 1, 0,
                       WindowClass::INPUT_OUTPUT, default_screen.root_visual, &aux)?.check()?;

    // Create tray window
    subtle.tray_win = conn.generate_id()?;

    let aux = CreateWindowAux::default()
        .event_mask(EventMask::KEY_PRESS | EventMask::BUTTON_PRESS)
        .override_redirect(1);

    conn.create_window(COPY_DEPTH_FROM_PARENT, subtle.tray_win, default_screen.root,
                       0, 0, 1, 1, 0,
                       WindowClass::INPUT_OUTPUT, default_screen.root_visual, &aux)?.check()?;

    // Create double buffer id and create/resize later
    subtle.panel_double_buffer = conn.generate_id()?;

    // Check extensions
    if conn.query_extension("XINERAMA".as_ref())?.reply()?.present {
        subtle.flags.insert(SubtleFlags::XINERAMA);
        
        debug!("Found xinerama extension");
    }
    
    if conn.query_extension("RANDR".as_ref())?.reply()?.present {
        subtle.flags.insert(SubtleFlags::XRANDR);

        debug!("Found xrandr extension");
    }

    // Create GCs
    let aux = CreateGCAux::default()
        .function(GX::INVERT)
        .subwindow_mode(SubwindowMode::INCLUDE_INFERIORS)
        .line_width(3);

    subtle.invert_gc = conn.generate_id()?;

    conn.create_gc(subtle.invert_gc, default_screen.root, &aux)?.check()?;

    subtle.draw_gc = conn.generate_id()?;

    let aux = CreateGCAux::default()
        .line_width(1)
        .line_style(LineStyle::SOLID)
        .join_style(JoinStyle::MITER)
        .cap_style(CapStyle::BUTT)
        .fill_style(FillStyle::SOLID);

    conn.create_gc(subtle.draw_gc, default_screen.root, &aux)?.check()?;

    // Create cursors
    let font_wrapper = FontWrapper::open_font(&conn, "cursor".as_bytes())?;

    subtle.arrow_cursor = conn.generate_id()?;
    conn.create_glyph_cursor(subtle.arrow_cursor, font_wrapper.font(), font_wrapper.font(),
                             XC_LEFT_PTR, XC_LEFT_PTR + 1, 0, 0, 0,
                             u16::MAX, u16::MAX, u16::MAX)?.check()?;

    subtle.move_cursor = conn.generate_id()?;
    conn.create_glyph_cursor(subtle.move_cursor, font_wrapper.font(), font_wrapper.font(),
                             XC_DOTBOX, XC_DOTBOX + 1, 0, 0, 0,
                             u16::MAX, u16::MAX, u16::MAX)?.check()?;

    subtle.resize_cursor = conn.generate_id()?;
    conn.create_glyph_cursor(subtle.resize_cursor, font_wrapper.font(), font_wrapper.font(),
                             XC_SIZING, XC_SIZING + 1, 0, 0, 0,
                             u16::MAX, u16::MAX, u16::MAX)?.check()?;

    drop(font_wrapper);

    // Update root window
    let aux = ChangeWindowAttributesAux::default()
        .cursor(subtle.arrow_cursor)
        .event_mask(EventMask::STRUCTURE_NOTIFY
            | EventMask::SUBSTRUCTURE_NOTIFY
            | EventMask::SUBSTRUCTURE_REDIRECT
            | EventMask::FOCUS_CHANGE
            | EventMask::PROPERTY_CHANGE);

    conn.change_window_attributes(default_screen.root, &aux)?.check()?;

    conn.flush()?;

    subtle.width = conn.setup().roots[screen_num].width_in_pixels;
    subtle.height = conn.setup().roots[screen_num].height_in_pixels;
    subtle.screen_num = screen_num;
    subtle.conn.set(conn).map_err(|_e| anyhow!("Connection already set?"))?;

    info!("Display ({}) is {}x{}", config.display, subtle.width, subtle.height);

    Ok(())
}

/// Claim display selection
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
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
    
    if conn.get_selection_owner(session)?.reply()?.owner != subtle.support_win {
        return Err(anyhow!("Failed replacing current window manager"))
    }

    debug!("{}", function_name!());

    Ok(())
}

/// Scan display for clients and adopt them
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
pub(crate) fn scan(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    for win in conn.query_tree(default_screen.root)?.reply()?.children {
        let attr = conn.get_window_attributes(win)?.reply()?;

        if !attr.override_redirect {
            match attr.map_state {
                MapState::VIEWABLE => {
                    let client = Client::new(subtle, win)?;

                    subtle.add_client(client);
                },
                _ => {},
            }
        }
    }
    
    client::publish(subtle, false)?;

    debug!("{}", function_name!());

    Ok(())
}

/// Get tray selection for display
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
pub(crate) fn select_tray(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    // Acquire tray selection
    conn.set_selection_owner(subtle.tray_win, atoms._NET_SYSTEM_TRAY_S0, CURRENT_TIME)?.check()?;

    if conn.get_selection_owner(atoms._NET_SYSTEM_TRAY_S0)?.reply()?.owner != subtle.tray_win {
        return Err(anyhow!("Failed getting system tray selection"))
    }

    // Send manager info
    let default_screen = &conn.setup().roots[subtle.screen_num];

    ewmh::send_message(subtle, default_screen.root, atoms.MANAGER, &[CURRENT_TIME,
        atoms._NET_SYSTEM_TRAY_S0, subtle.tray_win, 0, 0])?;

    debug!("{}", function_name!());

    Ok(())
}

/// Remove tray selection
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A `Result` with either `Unit` on success or otherwise `Error
pub(crate) fn deselect_tray(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    if conn.get_selection_owner(atoms._NET_SYSTEM_TRAY_S0)?.reply()?.owner == subtle.tray_win {
        conn.set_selection_owner(NONE, atoms._NET_SYSTEM_TRAY_S0, CURRENT_TIME)?.check()?;

        let default_screen = &conn.setup().roots[subtle.screen_num];

        conn.delete_property(default_screen.root, atoms._NET_SYSTEM_TRAY_S0)?.check()?;
    }

    debug!("{}", function_name!());

    Ok(())
}

pub(crate) fn configure(_subtle: &Subtle) -> Result<()> {
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
pub(crate) fn publish(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let default_screen = &conn.setup().roots[subtle.screen_num];

    // EWMH: Supported hints
    let mut supported_atoms: Vec<u32> = Vec::with_capacity(atoms.iter().len());

    for (_field_name, field_value) in atoms.iter() {
        let maybe_atom = (&*field_value).downcast_ref::<u32>();

        if let Some(atom) = maybe_atom {
            supported_atoms.push(atom.clone());
        }
    }

    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_SUPPORTED,
                           AtomEnum::ATOM, &supported_atoms)?.check()?;

    // EWMH: Window manager information
    let data: [u32; 1] = [subtle.support_win];

    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_SUPPORTING_WM_CHECK,
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

    conn.change_property32(PropMode::REPLACE, default_screen.root, atoms._NET_DESKTOP_GEOMETRY,
                           AtomEnum::CARDINAL, &data)?.check()?;

    conn.flush()?;

    debug!("{}", function_name!());

    Ok(())
}

/// Tidy up
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn finish(subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    let default_screen = &conn.setup().roots[subtle.screen_num];

    conn.flush()?;

    // Free GCs
    conn.free_gc(subtle.invert_gc)?;
    conn.free_gc(subtle.draw_gc)?;

    // Free cursors
    conn.free_cursor(subtle.arrow_cursor)?;
    conn.free_cursor(subtle.move_cursor)?;
    conn.free_cursor(subtle.resize_cursor)?;

    // Destroy windows
    conn.destroy_window(subtle.support_win)?;
    conn.destroy_window(subtle.tray_win)?;

    // Destroy pixmaps
    if 0 != subtle.panel_double_buffer {
        conn.free_pixmap(subtle.panel_double_buffer)?;
    }

    conn.set_input_focus(InputFocus::POINTER_ROOT, default_screen.root, CURRENT_TIME)?.check()?;

    // Destroy fonts
    for font in subtle.fonts.iter() {
        font.kill(conn)?;
    }

    debug!("{}", function_name!());

    Ok(())
}
