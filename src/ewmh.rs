///
/// @package subtle-rs
///
/// @file Ewmh functions
/// @copyright (c) 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use anyhow::Result;
use bitflags::bitflags;
use log::debug;
use stdext::function_name;
use struct_iterable::Iterable;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{Atom, ClientMessageEvent, ConnectionExt, EventMask, Window};
use crate::config::Config;
use crate::subtle::{Subtle, SubtleFlags};

#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum WMState {
    Withdrawn = 0,
    Normal = 1,
}

bitflags! {
    #[derive(Default, Debug, Copy, Clone, PartialEq)]
    pub(crate) struct EWMHStateFlags: u32 {
        const FULL = 1 << 0;
        const FLOAT = 1 << 1;
        const STICK = 1 << 2;
        const RESIZE = 1 << 3;
        const URGENT = 1 << 4;
        const ZAPHOD = 1 << 5;
        const FIXED = 1 << 6;
        const CENTER = 1 << 7;
        const BORDERLESS = 1 << 8;
        const VISIBLE = 1 << 9;
        const HIDDEN = 1 << 10;
        const HORZ = 1 << 11;
        const VERT = 1 << 12;
    }
}

x11rb::atom_manager! {
    #[derive(Iterable)]
    pub Atoms: AtomsCookie {
        // ICCCM
        WM_NAME, WM_CLASS, WM_STATE, WM_PROTOCOLS, WM_TAKE_FOCUS,
        WM_DELETE_WINDOW, WM_NORMAL_HINTS, WM_SIZE_HINTS, WM_HINTS,
        WM_WINDOW_ROLE, WM_CLIENT_LEADER,

        // EWMH
        _NET_SUPPORTED, _NET_CLIENT_LIST, _NET_CLIENT_LIST_STACKING,
        _NET_NUMBER_OF_DESKTOPS, _NET_DESKTOP_NAMES, _NET_DESKTOP_GEOMETRY,
        _NET_DESKTOP_VIEWPORT, _NET_CURRENT_DESKTOP, _NET_ACTIVE_WINDOW,
        _NET_WORKAREA, _NET_SUPPORTING_WM_CHECK, _NET_WM_FULL_PLACEMENT,
        _NET_FRAME_EXTENTS,

        // Client
        _NET_CLOSE_WINDOW, _NET_RESTACK_WINDOW, _NET_MOVERESIZE_WINDOW,
        _NET_WM_NAME, _NET_WM_PID, _NET_WM_DESKTOP, _NET_WM_STRUT,

        // Types
        _NET_WM_WINDOW_TYPE, _NET_WM_WINDOW_TYPE_DOCK, _NET_WM_WINDOW_TYPE_DESKTOP,
        _NET_WM_WINDOW_TYPE_TOOLBAR, _NET_WM_WINDOW_TYPE_SPLASH,
        _NET_WM_WINDOW_TYPE_DIALOG,

        // States
        _NET_WM_STATE, _NET_WM_STATE_FULLSCREEN, _NET_WM_STATE_ABOVE,
        _NET_WM_STATE_STICKY, _NET_WM_STATE_DEMANDS_ATTENTION,

        // Tray
        _NET_SYSTEM_TRAY_OPCODE, _NET_SYSTEM_TRAY_MESSAGE_DATA, _NET_SYSTEM_TRAY_S0,

        // Misc
        UTF8_STRING, MANAGER, _MOTIF_WM_HINTS,

        // XEmbed
        _XEMBED, _XEMBED_INFO,

        // subtle
        SUBTLE_CLIENT_TAGS, SUBTLE_CLIENT_RETAG, SUBTLE_CLIENT_GRAVITY,
        SUBTLE_CLIENT_SCREEN, SUBTLE_CLIENT_FLAGS, SUBTLE_GRAVITY_NEW,
        SUBTLE_GRAVITY_FLAGS, SUBTLE_GRAVITY_LIST, SUBTLE_GRAVITY_KILL,
        SUBTLE_TAG_NEW, SUBTLE_TAG_LIST, SUBTLE_TAG_KILL, SUBTLE_TRAY_LIST,
        SUBTLE_VIEW_NEW, SUBTLE_VIEW_TAGS, SUBTLE_VIEW_STYLE, SUBTLE_VIEW_ICONS,
        SUBTLE_VIEW_KILL, SUBTLE_SUBLET_UPDATE, SUBTLE_SUBLET_DATA,
        SUBTLE_SUBLET_STYLE, SUBTLE_SUBLET_FLAGS, SUBTLE_SUBLET_LIST,
        SUBTLE_SUBLET_KILL, SUBTLE_SCREEN_PANELS, SUBTLE_SCREEN_VIEWS,
        SUBTLE_SCREEN_JUMP, SUBTLE_VISIBLE_TAGS, SUBTLE_VISIBLE_VIEWS,
        SUBTLE_RENDER, SUBTLE_RELOAD, SUBTLE_RESTART, SUBTLE_QUIT, SUBTLE_COLORS,
        SUBTLE_FONT, SUBTLE_DATA, SUBTLE_VERSION,
    }
}

/// Check config and init all ewmh related options
///
/// # Arguments
///
/// * `config` - Config values read either from args or config file
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn init(_config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    
    let atoms = Atoms::new(conn)?.reply()?;
    
    subtle.atoms.set(atoms).unwrap();

    subtle.flags.insert(SubtleFlags::EWMH);
    
    debug!("{}", function_name!());

    Ok(())
}

/// Helper to send message to window
///
/// # Arguments
///
/// * `subtle` - Global state object
/// * `win` - Receiving window
/// * `message_type` - Message type
/// * `data32` - Slice of data to send
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn send_message(subtle: &Subtle, win: Window, message_type: Atom, data32: &[u32; 5]) -> Result<()> {
    let conn = subtle.conn.get().unwrap();

    conn.send_event(false, win, EventMask::NO_EVENT, &ClientMessageEvent::new(
        32,
        win,
        message_type,
        *data32
    ))?.check()?;

    Ok(())
}

/// Tidy up afterwards
///
/// # Arguments
///
/// * `subtle` - Global state object
///
/// # Returns
///
/// A [`Result`] with either [`unit`] on success or otherwise [`anyhow::Error`]
pub(crate) fn finish(subtle: &Subtle) -> Result<()> {

    // Delete root properties on real shutdown
    if subtle.flags.contains(SubtleFlags::EWMH) {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let default_screen = &conn.setup().roots[subtle.screen_num];

        // EWMH properties
        conn.delete_property(default_screen.root, atoms._NET_SUPPORTED)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_SUPPORTING_WM_CHECK)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_ACTIVE_WINDOW)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_CURRENT_DESKTOP)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_DESKTOP_NAMES)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_NUMBER_OF_DESKTOPS)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_DESKTOP_VIEWPORT)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_DESKTOP_GEOMETRY)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_WORKAREA)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_CLIENT_LIST)?.check()?;
        conn.delete_property(default_screen.root, atoms._NET_CLIENT_LIST_STACKING)?.check()?;

        // subtle extension
        conn.delete_property(default_screen.root, atoms.SUBTLE_GRAVITY_LIST)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_TAG_LIST)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_TRAY_LIST)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_VIEW_TAGS)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_COLORS)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_SUBLET_LIST)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_SCREEN_VIEWS)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_VISIBLE_VIEWS)?.check()?;
        conn.delete_property(default_screen.root, atoms.SUBTLE_VISIBLE_TAGS)?.check()?;
    }

    debug!("{}", function_name!());

    Ok(())
}
