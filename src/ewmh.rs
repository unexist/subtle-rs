///
/// @package subtle-rs
///
/// @file Ewmh functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use anyhow::Result;
use log::debug;
use stdext::function_name;
use crate::config::Config;
use crate::subtle::Subtle;

x11rb::atom_manager! {
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
        _NET_SYSTEM_TRAY_OPCODE, _NET_SYSTEM_TRAY_MESSAGE_DATA, _NET_SYSTEM_TRAY_S,

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

pub(crate) fn init(config: &Config, subtle: &mut Subtle) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    
    let atoms = Atoms::new(conn)?.reply()?;
    
    subtle.atoms.set(atoms).unwrap();
    
    debug!("{}", function_name!());

    Ok(())
}
