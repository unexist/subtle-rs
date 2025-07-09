///
/// @package subtle-rs
///
/// @file Client functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use std::fmt;
use std::ops::Div;
use x11rb::protocol::xproto::{Atom, AtomEnum, ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, CreateWindowAux, EventMask, PropMode, Rectangle, SetMode, Window};
use bitflags::bitflags;
use anyhow::{anyhow, Context, Result};
use easy_min_max::max;
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::NONE;
use x11rb::properties::{WmSizeHints, WmSizeHintsSpecification};
use x11rb::protocol::randr::ModeFlag;
use x11rb::wrapper::ConnectionExt as ConnectionExtWrapper;
use crate::ewmh::{Atoms, AtomsCookie};
use crate::subtle::{Subtle, SubtleFlags};
use crate::gravity::GravityFlags;
use crate::tagging::Tagging;

const MIN_WIDTH: u16 = 1;
const MIN_HEIGHT: u16 = 1;

#[repr(u8)]
#[derive(Copy, Clone)]
pub(crate) enum WMState {
    WithdrawnState = 0,
    NormalState = 1,
}

bitflags! {
    #[derive(Default, Debug)]
    pub(crate) struct ClientFlags: u32 {
        const DEAD = 1 << 0;  // Dead window
        const FOCUS = 1 << 1; // Send focus message
        const INPUT = 1 << 2; // Active/passive focus-model
        const CLOSE = 1 << 3; // Send close message
        const UNMAP = 1 << 4; // Ignore unmaps
        const ARRANGE = 1 << 5; // Re-arrange client

        const MODE_FULL = 1 << 6; // Fullscreen mode (also used in tags)
        const MODE_FLOAT = 1 << 7; // Float mode
        const MODE_STICK = 1 << 8; // Stick mode
        const MODE_STICK_SCREEN = 1 << 9; // Stick tagged screen mode
        const MODE_URGENT = 1 << 10; // Urgent mode
        const MODE_RESIZE = 1 << 11; // Resize mode
        const MODE_ZAPHOD = 1 << 12; // Zaphod mode
        const MODE_FIXED = 1 << 13; // Fixed size mode
        const MODE_CENTER = 1 << 14; // Center position mode
        const MODE_BORDERLESS = 1 << 15; // Borderless

        const TYPE_NORMAL = 1 << 16; // Normal type (also used in match)
        const TYPE_DESKTOP = 1 << 17; // Desktop type
        const TYPE_DOCK = 1 << 18; // Dock type
        const TYPE_TOOLBAR = 1 << 19; // Toolbar type
        const TYPE_SPLASH = 1 << 20; // Splash type
        const TYPE_DIALOG = 1 << 21; // Dialog type
    }
}

#[derive(Default, Debug)]
pub(crate) struct Client {
    pub(crate) flags: ClientFlags,
    pub(crate) tags: Tagging,

    pub(crate) win: Window,
    pub(crate) leader: Window,

    pub(crate) name: String,
    pub(crate) instance: String,
    pub(crate) klass: String,
    pub(crate) role: String,

    pub(crate) min_ratio: f32,
    pub(crate) max_ratio: f32,

    pub(crate) min_width: u16,
    pub(crate) min_height: u16,
    pub(crate) max_width: i16,
    pub(crate) max_height: i16,
    pub(crate) width_inc: u16,
    pub(crate) height_inc: u16,
    pub(crate) base_width: u16,
    pub(crate) base_height: u16,

    pub(crate) screen_id: usize,
    pub(crate) gravity_id: isize,
    
    pub(crate) geom: Rectangle,

    pub(crate) gravities: Vec<usize>,
}

impl Client {
    pub(crate) fn new(subtle: &Subtle, win: Window) -> Result<Self> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        conn.grab_server()?;
        conn.change_save_set(SetMode::INSERT, win)?;

        // Fetch name, instance, class and role
        let wm_name = conn.get_property(false, win,
                                        atoms.WM_NAME, AtomEnum::STRING,
                                        0, u32::MAX)?.reply()?.value;

        let wm_klass = conn.get_property(false, win, atoms.WM_CLASS,
                                         AtomEnum::STRING, 0, u32::MAX)?.reply()?.value;

        let wm_role= conn.get_property(false, win, AtomEnum::STRING,
                                       atoms.WM_WINDOW_ROLE, 0, u32::MAX)?.reply()?.value;

        let inst_klass = String::from_utf8(wm_klass)
            .expect("UTF-8 string should be valid UTF-8")
            .trim_matches('\0')
            .split('\0')
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        // X Properties
        let geom_reply = conn.get_geometry(win)?.reply()?;

        let aux = ChangeWindowAttributesAux::default()
            .border_pixel(Some(0)) // TODO Styles
            .event_mask(EventMask::PROPERTY_CHANGE
                | EventMask::ENTER_WINDOW
                | EventMask::FOCUS_CHANGE);

        conn.change_window_attributes(win, &aux)?.check()?;

        let aux = ConfigureWindowAux::default()
            .border_width(Some(0)); // TODO Styles

        conn.configure_window(win, &aux)?.check()?;

        conn.ungrab_server()?;

        let mut client = Self {
            win,
            name: String::from_utf8(wm_name)?,
            instance: inst_klass[0].to_string(),
            klass: inst_klass[1].to_string(),
            role: String::from_utf8(wm_role)?,

            screen_id: 0,
            gravity_id: -1,

            geom: Rectangle {
                x: geom_reply.x,
                y: geom_reply.y,
                width: max!(MIN_WIDTH, geom_reply.width),
                height: max!(MIN_HEIGHT, geom_reply.height),
            },
            gravities: Vec::with_capacity(subtle.views.len()),
            ..Self::default()
        };

        // Update client
        let mut mode_flags = ClientFlags::empty();

        //client.set_strut
        client.set_size_hints(subtle, &mut mode_flags)?;
        client.set_wm_state(subtle, WMState::WithdrawnState)?;
        client.set_wm_protocols(subtle)?;
        client.set_wm_type(subtle, &mut mode_flags)?;
        //client.set_wm_hints
        client.set_motif_wm_hints(subtle, &mut mode_flags)?;
        client.set_net_wm_state(subtle, &mut mode_flags)?;
        //client.set_transient
        client.retag(subtle, &mut mode_flags)?;
        //client.toggle(mode_flags

        // Set leader window
        let leader = conn.get_property(false, client.win, AtomEnum::WINDOW,
                                       atoms.WM_CLIENT_LEADER, 0, 1)?.reply()?.value;

        if !leader.is_empty() && NONE != leader[0] as u32 {
            client.leader = leader[0] as Window;
        }

        // EWMH: Gravity, screen, desktop, extents
        let data: [u32; 1] = [client.gravity_id as u32];

        conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_GRAVITY,
            AtomEnum::CARDINAL, &data)?.check()?;

        let data: [u32; 1] = [client.screen_id as u32];

        conn.change_property32(PropMode::REPLACE, client.win, atoms.SUBTLE_CLIENT_SCREEN,
                               AtomEnum::CARDINAL, &data)?.check()?;

        let data: [u32; 1] = [0];

        conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_WM_DESKTOP,
            AtomEnum::CARDINAL, &data)?.check()?;

        // TODO Struts
        //conn.change_property32(PropMode::REPLACE, client.win, atoms._NET_FRAME_EXTENTS
        //                       AtomEnum::CARDINAL, &data)?.check()?;

        debug!("{}: {}", function_name!(), client);

        Ok(client)
    }
    
    pub(crate) fn set_size_hints(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Assume first screen
        let screen = subtle.screens.first().context("No screens")?;

        // Set default values
        self.min_width = MIN_WIDTH;
        self.min_height = MIN_HEIGHT;
        self.max_width = -1;
        self.max_height = -1;
        self.min_ratio = 0.0;
        self.max_ratio = 0.0;
        self.width_inc = 1;
        self.height_inc = 1;
        self.base_width = 0;
        self.base_height = 0;

        let maybe_hints = WmSizeHints::get_normal_hints(conn, self.win)?.reply()?;

        if let Some(hints) = maybe_hints {
            // Program min size - limit min size to screen size if larger
           if let Some((min_width, min_height)) = hints.min_size {
               self.min_width = if self.min_width > screen.geom.width {
                   screen.geom.width } else { max!(MIN_WIDTH, min_width as u16) };

               self.min_height = if self.min_height > screen.geom.height {
                   screen.geom.height } else { max!(MIN_HEIGHT, min_height as u16) };
           }

            // Program max size - limit max size to screen if larger
            if let Some((max_width, max_height)) = hints.max_size {
                self.max_width = if max_width > screen.geom.width as i32 {
                    screen.geom.width as i16 } else { max_width as i16 };

                self.max_height = if max_height > screen.geom.height as i32 {
                    (screen.geom.height - subtle.panel_height) as i16
                } else { max_height as i16 };
            }

            // Set float when min == max size (EWMH: Fixed size windows)
            if let Some((min_width, min_height)) = hints.min_size
                && let Some((max_width, max_height)) = hints.max_size
            {
                if min_width == max_width && min_height == max_height && !self.flags.contains(ClientFlags::TYPE_DESKTOP) {
                    mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FIXED);
                }
            }

            // Aspect ratios
            if let Some((min_aspect, max_aspect)) = hints.aspect {
                self.min_ratio = min_aspect.numerator as f32 / min_aspect.denominator as f32;
                self.max_ratio = max_aspect.numerator as f32 / max_aspect.denominator as f32;
            }

            // Resize increment steps
            if let Some((width_inc, height_inc)) = hints.size_increment {
                self.width_inc = width_inc as u16;
                self.height_inc = height_inc as u16;

            }

            // Base sizes
            if let Some((base_width, base_height)) = hints.base_size {
                self.base_width = base_width as u16;
                self.base_height = base_height as u16;
            }

            // Check for specific position and size
            if subtle.flags.contains(SubtleFlags::RESIZE)
                || self.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE | ClientFlags::TYPE_DOCK)
            {
                // User/program position
                if let Some((hint_spec, x, y)) = hints.position {
                    match hint_spec {
                        WmSizeHintsSpecification::UserSpecified | WmSizeHintsSpecification::ProgramSpecified => {
                            self.geom.x = x as i16;
                            self.geom.y = y as i16;
                        }
                    }
                }

                // User/program size
                if let Some((hint_spec, x, y)) = hints.size {
                    match hint_spec {
                        WmSizeHintsSpecification::UserSpecified | WmSizeHintsSpecification::ProgramSpecified => {
                            self.geom.width = x as u16;
                            self.geom.height = y as u16;
                        }
                    }
                }

                // Sanitize positions for stupid clients like GIMP
                self.resize(subtle, &screen.geom, true)?;
            }
        }

        debug!("{}: client={}, minw={}, minh={}, maxw={}, maxh={}, \
            minr={}, maxr={}, incw={}, inch={}, basew={}, baseh={}",
            function_name!(), self, self.min_width, self.min_height,
            self.max_width, self.max_height,
            self.min_ratio, self.max_ratio, self.width_inc, self.height_inc,
            self.base_width, self.base_height);

        Ok(())
    }

    pub(crate) fn set_wm_state(&self, subtle: &Subtle, state: WMState) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let data: [u8; 2] = [state as u8, NONE as u8];

        conn.change_property(PropMode::REPLACE,
                             self.win, atoms.WM_STATE, atoms.WM_STATE, 8, 2, &data)?;

        Ok(())
    }

    pub(crate) fn set_wm_protocols(&mut self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let protocols = conn.get_property(false, self.win, atoms.WM_PROTOCOLS,
                                          AtomEnum::ATOM, 0, u32::MAX)?.reply()?.value;

        for protocol in protocols {
            if atoms.WM_TAKE_FOCUS == protocol as u32 {
                self.flags.insert(ClientFlags::FOCUS);
            } else if atoms.WM_DELETE_WINDOW == protocol as u32 {
                self.flags.insert(ClientFlags::CLOSE);
            }
        }

        debug!("{}: client={}", function_name!(), self);

        Ok(())
    }

    pub(crate) fn set_wm_type(&mut self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let wm_types = conn.get_property(false, self.win, AtomEnum::ATOM,
                                         atoms._NET_WM_WINDOW_TYPE, 0, 5)?.reply()?.value;

        for wm_type in wm_types {
            if atoms._NET_WM_WINDOW_TYPE_DESKTOP == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DESKTOP);
                mode_flags.insert(ClientFlags::MODE_FIXED | ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_WINDOW_TYPE_DOCK == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DOCK);
                mode_flags.insert(ClientFlags::MODE_FIXED | ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_WINDOW_TYPE_TOOLBAR == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_TOOLBAR);
            } else if atoms._NET_WM_WINDOW_TYPE_SPLASH == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_SPLASH);
                mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_CENTER);
            } else if atoms._NET_WM_WINDOW_TYPE_DIALOG == wm_type as u32 {
                self.flags.insert(ClientFlags::TYPE_DIALOG);
                mode_flags.insert(ClientFlags::MODE_FLOAT | ClientFlags::MODE_CENTER);
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    pub(crate) fn set_motif_wm_hints(&self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let hints = conn.get_property(false, self.win, atoms._MOTIF_WM_HINTS,
                                      atoms._MOTIF_WM_HINTS, 0, 1)?.reply()?.value;

        // TODO

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    pub(crate) fn set_net_wm_state(&self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        let states = conn.get_property(false, self.win, AtomEnum::ATOM,
                                       atoms._NET_WM_STATE, 0, 4)?.reply()?.value;

        for state in states {
            if atoms._NET_WM_STATE_FULLSCREEN == state as Atom {
                mode_flags.insert(ClientFlags::MODE_FULL);
            } else if atoms._NET_WM_STATE_ABOVE == state as Atom {
                mode_flags.insert(ClientFlags::MODE_FLOAT);
            } else if atoms._NET_WM_STATE_STICKY == state as Atom {
                mode_flags.insert(ClientFlags::MODE_STICK);
            } else if atoms._NET_WM_STATE_DEMANDS_ATTENTION == state as Atom {
                mode_flags.insert(ClientFlags::MODE_URGENT);
            }
        }

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }


    pub(crate) fn tag(&self, tag_idx: usize, mode_flags: &mut ClientFlags) {
        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);
    }

    pub(crate) fn retag(&self, subtle: &Subtle, mode_flags: &mut ClientFlags) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        for (idx, tag) in subtle.tags.iter().enumerate() {
            if tag.matches(self) {
                self.tag(idx, mode_flags);
            }
        }

        if self.flags.contains(ClientFlags::MODE_STICK) && !mode_flags.contains(ClientFlags::MODE_STICK) {
            let mut visible: u8 = 0;

            for view in subtle.views.iter() {
                if view.tags.contains(self.tags) {
                    visible += 1;
                }
            }

            if 0 == visible {
                self.tag(0, mode_flags);
            }
        }

        // EWMH: Tags
        let data: [u32; 1] = [self.tags.bits()];

        conn.change_property32(PropMode::REPLACE, self.win, 
                               atoms.SUBTLE_CLIENT_TAGS, AtomEnum::CARDINAL, &data)?.check()?;

        debug!("{}: client={}, mode_flags={:?}", function_name!(), self, mode_flags);

        Ok(())
    }

    pub(crate) fn resize(&mut self, subtle: &Subtle, bounds: &Rectangle, use_size_hints: bool) -> Result<()> {
        if use_size_hints {
            self.check_bounds(bounds, false, false);
        }

        if !self.flags.contains(ClientFlags::MODE_FULL | ClientFlags::TYPE_DOCK) {
            let mut max_x = 0;
            let mut max_y = 0;

            if !self.flags.contains(ClientFlags::MODE_FIXED) {
                if self.geom.width > bounds.width {
                    self.geom.width = bounds.width;
                }

                if self.geom.height > bounds.height {
                    self.geom.height = bounds.height;
                }
            }

            // Check whether window fits into bounds
            max_x = bounds.x + bounds.width as i16;
            max_y = bounds.y + bounds.height as i16;

            // Check x and center
            if self.geom.x < bounds.x || self.geom.x > max_x || self.geom.x + self.geom.width as i16  > max_x {
                if self.flags.contains(ClientFlags::MODE_FLOAT) {
                    self.geom.x = bounds.x + ((bounds.width as i16 - self.geom.width as i16) / 2);
                } else {
                    self.geom.x = bounds.x;
                }
            }

            // Check y and center
            if self.geom.y < bounds.y || self.geom.y > max_y || self.geom.y + self.geom.height as i16 > max_y {
                if self.flags.contains(ClientFlags::MODE_FLOAT) {
                    self.geom.y = bounds.y + ((bounds.height as i16 - self.geom.height as i16) / 2);
                } else {
                    self.geom.y = bounds.y;
                }
            }
        }
        Ok(())
    }

    fn move_resize(&mut self, subtle: &Subtle, bounds: &Rectangle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        // Update margins, border and gap
        //self.geom.x += subtle.client.margin.left as i16; // TODO Styles
        //self.geom.y += subtle.client.margin.left as i16;
        //self.geom.width -= (2 *

        self.resize(subtle, bounds, true)?;

        let aux = ConfigureWindowAux::default()
            .x(Some(self.geom.x as i32))
            .y(Some(self.geom.y as i32))
            .width(Some(self.geom.width as u32))
            .height(Some(self.geom.height as u32));

        conn.configure_window(self.win, &aux)?.check()?;

        Ok(())
    }

    pub(crate) fn map(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        conn.map_window(self.win)?.check()?;

        Ok(())
    }

    pub(crate) fn unmap(&self, subtle: &Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();

        conn.unmap_window(self.win)?.check()?;

        Ok(())
    }

    pub(crate) fn is_visible(&self, subtle: &Subtle) -> bool {
        subtle.visible_tags.contains(self.tags)
            || self.flags.contains(ClientFlags::TYPE_DESKTOP | ClientFlags::MODE_STICK)
    }

    pub(crate) fn kill(&self, subtle: &mut Subtle) -> Result<()> {
        let conn = subtle.conn.get().unwrap();
        let atoms = subtle.atoms.get().unwrap();

        // Remove _NET_WM_STATE (see EWMH 1.3)
        conn.delete_property(self.win, atoms._NET_WM_STATE)?.check()?;

        // Ignore further events
        let aux = ChangeWindowAttributesAux::default()
            .event_mask(EventMask::NO_EVENT);

        conn.change_window_attributes(self.win, &aux)?.check()?;

        // Remove client tags from urgent tags
        if self.flags.contains(ClientFlags::MODE_URGENT) {
            subtle.urgent_tags = subtle.urgent_tags.difference(self.tags);
        }

        // Tile remaining clients if necessary
        if self.is_visible(subtle) {
            if let Some(gravity) = subtle.gravities.get(self.gravity_id as usize) {
               if subtle.flags.contains(SubtleFlags::TILING)
                   || gravity.flags.contains(GravityFlags::HORZ | GravityFlags::VERT)
               {
                   self.gravity_tile(subtle, self.gravity_id, self.screen_id)?;
               }
            }
        }

        debug!("{}", function_name!());

        Ok(())
    }

    fn gravity_tile(&self, subtle: &mut Subtle, gravity_id: isize, screen_id: usize) -> Result<()> {
        let gravity = subtle.gravities.get(gravity_id as usize)
            .ok_or(anyhow!("Gravity not found"))?;
        let screen = subtle.screens.get(screen_id)
            .ok_or(anyhow!("Screen not found"))?;

        // Pass 1: Count clients with this gravity
        let mut used = 0u16;

        for client in subtle.clients.iter() {
            if client.gravity_id == gravity_id && client.screen_id == screen_id
                && subtle.visible_tags.contains(client.tags)
                && !client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL)
            {
                used += 1;
            }
        }

        if 0 == used {
            return Ok(());
        }

        // Calculate tiled gravity value and rounding fix
        let mut geom: Rectangle = Rectangle::default();

        gravity.calculate_geometry(&screen.geom, &mut geom);

        let mut calc = 0;
        let mut round_fix = 0;

        if gravity.flags.contains(GravityFlags::HORZ) {
            calc = geom.width / used;
            round_fix = geom.width - calc * used;
        } else if gravity.flags.contains(GravityFlags::VERT) {
            calc = geom.height / used;
            round_fix = geom.height - calc * used;
        }

        // Pass 2: Update geometry of every client with this gravity
        let mut pos = 0;

        for client in subtle.clients.iter_mut() {
            if client.gravity_id == gravity_id && client.screen_id == screen_id
                && subtle.visible_tags.contains(client.tags)
                && !client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL)
            {
                if gravity.flags.contains(GravityFlags::HORZ) {
                    client.geom.x = geom.x + (pos * calc) as i16;
                    client.geom.y = geom.y;
                    client.geom.width = if pos == used { calc + round_fix } else { calc };
                    client.geom.height = geom.height;

                    pos += 1;
                } else if gravity.flags.contains(GravityFlags::VERT) {
                    client.geom.x = geom.x;
                    client.geom.y = geom.y + (pos * calc) as i16;
                    client.geom.width = geom.width;
                    client.geom.height = if pos == used { calc + round_fix } else { calc };

                    pos +=1;
                }

                //client.move_resize(subtle, &screen.geom)?
            }
        }

        Ok(())
    }

    fn get_gravity(&self, subtle: &Subtle) -> isize {
        let mut grav_id= 0;

        if -1 == grav_id {
            // Copy gravity from current client
            if let Some(client) = subtle.find_client(subtle.focus_win) {
                grav_id = client.gravity_id;
            }
        } else {
            grav_id = subtle.default_gravity;
        }

        grav_id
    }

    fn check_bounds(&mut self, bounds: &Rectangle, adjust_x: bool, adjust_y: bool) {
        if !self.flags.contains(ClientFlags::MODE_FIXED)
            && (self.flags.contains(ClientFlags::MODE_RESIZE)
            || self.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE))
        {
            let border_width = if self.flags.contains(ClientFlags::MODE_BORDERLESS) { 0 } else { 1 }; // TODO

            // Calculate max width and max height for bounds
            let max_width = if -1 == self.max_width { bounds.width } else { self.max_width as u16 };
            let max_height = if -1 == self.max_height { bounds.height } else { self.max_height as u16 };

            // Limit width and height
            if self.geom.width < self.min_width {
                self.geom.width = self.min_width;
            }

            if self.geom.width > max_width {
                self.geom.width = max_width;
            }

            if self.geom.height < self.min_height {
               self.geom.height = self.min_height;
            }

            if self.geom.height > max_height {
                self.geom.height = max_height;
            }

            // Adjust based on increment values (see ICCCM 4.1.2.3)
            let diff_width = (self.geom.width - self.base_width) % self.width_inc;
            let diff_height = (self.geom.height - self.base_height) % self.height_inc;

            // Adjust x and/or y
            if adjust_x {
                self.geom.x += diff_width as i16;
            }

            if adjust_y {
                self.geom.y += diff_height as i16;
            }

            self.geom.width -= diff_width;
            self.geom.height -= diff_height;

            // Check aspect ratios
            if 0f32 < self.min_ratio && self.geom.height as f32 * self.min_ratio > self.geom.width as f32 {
                self.geom.width = (self.geom.height as f32 * self.min_ratio) as u16;
            }

            if 0f32 < self.max_ratio && self.geom.height as f32 * self.max_ratio < self.geom.width as f32 {
                self.geom.width = (self.geom.height as f32 * self.max_ratio) as u16;
            }
        }
    }
}

impl fmt::Display for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "name={}, instance={}, class={}, win={}, leader={}, \
            geom=(x={}, y={}, width={}, height={}), input={}, focus={}",
               self.name, self.instance, self.klass, self.win, self.leader,
               self.geom.x, self.geom.y, self.geom.width, self.geom.height,
               self.flags.contains(ClientFlags::INPUT), self.flags.contains(ClientFlags::FOCUS))
    }
}

pub(crate) fn publish(subtle: &Subtle, restack_windows: bool) -> Result<()> {
    let conn = subtle.conn.get().unwrap();
    let atoms = subtle.atoms.get().unwrap();

    let screen = &conn.setup().roots[subtle.screen_num];

    let mut wins: Vec<u32> = Vec::with_capacity(subtle.clients.len());

    // Sort clients from top to bottom
    for (idx, client) in subtle.clients.iter().enumerate().rev() {
        wins.push(client.win);
    }

    // EWMH: Client list and stacking list (same for us)
    conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_CLIENT_LIST,
                           AtomEnum::WINDOW, &wins)?;
    conn.change_property32(PropMode::REPLACE, screen.root, atoms._NET_CLIENT_LIST_STACKING,
                           AtomEnum::WINDOW, &wins)?;

    // Restack windows? We assembled the array anyway
    if restack_windows {
        // TODO
        //XRestackWindows
    }

    conn.flush()?;

    debug!("{}: clients={}, restack={}", function_name!(), subtle.clients.len(), restack_windows);

    Ok(())
}
