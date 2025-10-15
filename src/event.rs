///
/// @package subtle-rs
///
/// @file Event functions
/// @copyright 2025-present Christoph Kappel <christoph@unexist.dev>
/// @version $Id$
///
/// This program can be distributed under the terms of the GNU GPLv3.
/// See the file LICENSE for details.
///

use anyhow::{Context, Result};
use std::sync::atomic;
use std::sync::atomic::Ordering;
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::CURRENT_TIME;
use x11rb::protocol::xproto::{ButtonPressEvent, ClientMessageEvent, ConfigureNotifyEvent, ConfigureRequestEvent, ConfigureWindowAux, ConnectionExt, DestroyNotifyEvent, EnterNotifyEvent, ExposeEvent, FocusInEvent, KeyPressEvent, LeaveNotifyEvent, MapRequestEvent, Mapping, MappingNotifyEvent, ModMask, PropertyNotifyEvent, SelectionClearEvent, UnmapNotifyEvent, Window};
use x11rb::protocol::Event;
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::{Client, ClientFlags, RestackOrder};
use crate::{client, display, ewmh, grab, panel, screen, tray};
use crate::ewmh::WMState;
use crate::grab::{GrabAction, GrabFlags};
use crate::panel::PanelAction;
use crate::tray::{Tray, TrayFlags, XEmbed, XEmbedFocus};

fn handle_button_press(subtle: &Subtle, event: ButtonPressEvent) -> Result<()> {
    if let Some((_, screen)) = subtle.find_screen_by_panel_win(event.event) {
        screen.handle_action(
            subtle,
            &PanelAction::MouseDown(event.event_x, event.event_y, event.detail as i8),
            screen.bottom_panel_win == event.event)?;

        // Finally configure and render
        screen::configure(subtle)?;
        panel::render(subtle)?;
        screen::publish(subtle, false)?;
    }

    debug!("{}: win={}, x={}, y={}", function_name!(), event.event, event.event_x, event.event_y);

    Ok(())
}

fn handle_configure(subtle: &Subtle, event: ConfigureNotifyEvent) -> Result<()> {
    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_configure_request(subtle: &Subtle, event: ConfigureRequestEvent) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Complicated request! (see ICCCM 4.1.5)
    // No change    -> Synthetic ConfigureNotify
    // Move/restack -> Synthetic + real ConfigureNotify
    // Resize       -> Real ConfigureNotify

    // Check if we know the window
    if let Some(client) = subtle.find_client_mut(event.window) {
        // Check flags if the request is important
        if !client.flags.contains(ClientFlags::MODE_FULL)
            && subtle.flags.contains(SubtleFlags::RESIZE)
            || client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE)
        {
            let maybe_screen = subtle.screens.get(client.screen_idx as usize);
        }
    // Unmanaged window
    } else {
        conn.configure_window(event.window,
                              &ConfigureWindowAux::from_configure_request(&event))?.check()?;
    }

    Ok(())
}

fn handle_client_message(subtle: &Subtle, event: ClientMessageEvent) -> Result<()> {
    let atoms = subtle.atoms.get().unwrap();

    println!("win={}, data={:?}", event.window, event.data);

    // Check if we know the window
    if event.window == subtle.tray_win {
        if atoms._NET_SYSTEM_TRAY_OPCODE == event.type_ {
            let data = event.data.as_data32();

            match XEmbed::from_repr(data[1] as u8).context("Unknown tray opcode")? {
                XEmbed::EmbeddedNotify => {
                    if subtle.find_tray(data[2] as Window).is_none() {
                        if let Ok(tray) = Tray::new(subtle, data[2] as Window) {
                            subtle.add_tray(tray);

                            screen::configure(subtle)?;
                            panel::update(subtle)?;
                            panel::render(subtle)?;
                        }
                    }
                },
                XEmbed::WindowActivate => {
                    ewmh::send_message(subtle, data[2] as Window,
                                       atoms._XEMBED, &[CURRENT_TIME, XEmbed::FocusIn as u32,
                                           XEmbedFocus::Current as u32, 0, 0])?;
                },
                _ => {},
            }
        }
    } else if let Some(client) = subtle.find_client(event.window) {
        if atoms._NET_CLOSE_WINDOW == event.type_ {
            client.close(subtle)?;

            screen::configure(subtle)?;
            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    } else if let Some(tray) = subtle.find_tray(event.window) {
        if atoms._NET_CLOSE_WINDOW == event.type_ {
            tray.close(subtle)?;

            screen::configure(subtle)?;
            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    }

    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_destroy(subtle: &Subtle, event: DestroyNotifyEvent) -> Result<()> {
    // Check if we know the window
    if let Some(client) = subtle.find_client(event.window) {
        client.kill(subtle)?;

        drop(client);

        subtle.remove_client_by_win(event.window);

        client::publish(subtle, false)?;

        screen::configure(subtle)?;
        panel::update(subtle)?;
        panel::render(subtle)?;
    } else {
        // Check if window is client leader
        for client in subtle.clients.borrow_mut().iter_mut() {
            if client.leader == event.window {
                client.flags.insert(ClientFlags::DEAD);
            }
        }
    }

    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_enter(subtle: &Subtle, event: EnterNotifyEvent) -> Result<()> {
    if let Some(client) = subtle.find_client(event.event) {
        if !subtle.flags.intersects(SubtleFlags::CLICK_TO_FOCUS) {
            client.focus(subtle, false)?;
        }
    }

    debug!("{}: event={}, x={}, y={}", function_name!(),
        event.event, event.event_x, event.event_y);

    Ok(())
}

fn handle_leave(subtle: &Subtle, event: LeaveNotifyEvent) -> Result<()> {
    if let Some((_, screen)) = subtle.find_screen_by_panel_win(event.event) {
            screen.handle_action(subtle, &PanelAction::MouseOut,
                                 screen.bottom_panel_win == event.event)?;
    }

    debug!("{}: event={}, child={}, root={}", function_name!(),
        event.event, event.child, event.root);

    Ok(())
}

fn handle_expose(subtle: &Subtle, event: ExposeEvent) -> Result<()> {
    // Render only once
    if 0 == event.count {
        panel::render(subtle)?;
    }
    
    debug!("{}: win={}, count={}", function_name!(), event.window, event.count);

    Ok(())
}

fn handle_focus_in(subtle: &Subtle, event: FocusInEvent) -> Result<()> {
    if let Some(mut client) = subtle.find_client_mut(event.event) {

        // Remove urgent after getting focus
        if client.flags.intersects(ClientFlags::MODE_URGENT) {
            client.flags.remove(ClientFlags::MODE_URGENT);
            subtle.urgent_tags.replace(subtle.urgent_tags.get() - client.tags);
        }

        drop(client);

        // Update focus history
        if let Some(mut focus_win) = subtle.focus_history.borrow_mut(0) {
            *focus_win = event.event;
        }

        // Update screen
        panel::update(subtle)?;
        panel::render(subtle)?;
    }

    debug!("{}: win={}", function_name!(), event.event);

    Ok(())
}

fn handle_key_press(subtle: &Subtle, event: KeyPressEvent) -> Result<()> {
    // Limit mod mask to relevant ones
    let relevant_modifiers = ModMask::from(event.state.bits()
        & (ModMask::SHIFT | ModMask::CONTROL | ModMask::M1 | ModMask::M4));

    if let Some(grab) = subtle.find_grab(event.detail, relevant_modifiers) {
        let flag = grab.flags.difference(GrabFlags::IS_KEY | GrabFlags::IS_MOUSE);

        match flag {
            GrabFlags::VIEW_SWITCH | GrabFlags::VIEW_SELECT => {
                if let GrabAction::Index(idx) = grab.action {
                    if let Some(view) = subtle.views.get(idx as usize - 1) {
                        let mut screen_idx: isize = -1;

                        // Find screen: Prefer screen of current window
                        if subtle.flags.intersects(SubtleFlags::SKIP_POINTER_WARP)
                            && let Some(focus_client) = subtle.find_focus_client()
                            && focus_client.is_visible(subtle)
                        {
                            screen_idx = focus_client.screen_idx;
                        } else if let Some((maybe_screen_id, _)) = subtle.find_screen_by_xy(
                            event.event_x, event.event_y)
                        {
                            screen_idx = maybe_screen_id as isize;
                        }

                        view.focus(subtle, screen_idx as usize,
                                   GrabFlags::VIEW_SWITCH == flag, true)?;

                        // Finally configure and render
                        screen::configure(subtle)?;
                        panel::render(subtle)?;
                    }
                }
            },

            GrabFlags::WINDOW_MODE => {
                if let Some(mut focus_client) = subtle.find_focus_client_mut() {
                    if let GrabAction::Index(bits) = grab.action {
                        let mut mode_flags = ClientFlags::from_bits(bits)
                            .context("Unknown client flags")?;

                        focus_client.toggle(subtle, &mut mode_flags, true)?;

                        // Update screen and focus
                        if focus_client.is_visible(subtle) || ClientFlags::MODE_STICK == mode_flags {
                            // Store values and drop reference
                            let is_visible = focus_client.is_visible(subtle);
                            let screen_idx = focus_client.screen_idx;

                            drop(focus_client);

                            // Find next and focus
                            if !is_visible {
                                if let Some(next_client) = client::find_next(subtle, screen_idx, false) {
                                    next_client.focus(subtle, true)?;
                                }
                            }

                            // Finally configure, update and render
                            screen::configure(subtle)?;
                            panel::update(subtle)?;
                            panel::render(subtle)?;
                        }
                    }
                }
            }

            GrabFlags::WINDOW_GRAVITY => {
                if let Some(mut focus_client) = subtle.find_focus_client_mut() {
                    if let GrabAction::List(gravity_ids) = &grab.action {
                        // Remove float and fullscreen mode
                        if focus_client.flags.intersects(ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL) {
                            let mut mode_flags = focus_client.flags & (ClientFlags::MODE_FLOAT | ClientFlags::MODE_FULL);
                            focus_client.toggle(subtle, &mut mode_flags, true)?;

                            screen::configure(subtle)?;
                            panel::update(subtle)?;

                            focus_client.gravity_idx = -1; // Reset
                        }

                        // Find next gravity or fallback to first
                        let mut new_gravity_id = *gravity_ids.first().context("No gravity ID")?;

                        for (idx, gravity_id) in gravity_ids.iter().enumerate() {
                            if focus_client.gravity_idx == *gravity_id as isize {
                                if idx < gravity_ids.len() {
                                    new_gravity_id = idx + 1;
                                }

                                break;
                            }
                        }

                        // Finally update client
                        let screen_id = focus_client.screen_idx;
                        focus_client.arrange(subtle, new_gravity_id as isize, screen_id)?;

                        client::restack_clients(RestackOrder::Up)?;

                        if !subtle.flags.intersects(SubtleFlags::SKIP_POINTER_WARP) {
                            focus_client.warp_pointer(subtle)?;
                        }
                    }
                }
            },

            GrabFlags::WINDOW_KILL => {
                if let Some(focus_client) = subtle.find_focus_client_mut() {
                    let screen_idx = focus_client.screen_idx;

                    focus_client.close(subtle)?;

                    screen::configure(subtle)?;
                    panel::update(subtle)?;
                    panel::render(subtle)?;

                    // Update focus if necessary
                    if let Some(next_client) = client::find_next(subtle, screen_idx, false) {
                        next_client.focus(subtle, true)?;
                    }
                }
            },

            GrabFlags::SUBTLE_QUIT => {
                subtle.shutdown.store(true, Ordering::Relaxed);
            },

            _ => {},
        }

        println!("grab={:?}", grab);
    }

    panel::update(subtle)?;
    panel::render(subtle)?;

    // Restore binds
    let conn = subtle.conn.get().context("Failed to get connection")?;
    let default_screen = &conn.setup().roots[subtle.screen_num];

    grab::unset(subtle, default_screen.root)?;
    grab::set(subtle, default_screen.root, GrabFlags::IS_KEY)?;

    debug!("{}: win={}, keycode={}", function_name!(), event.event, event.detail);

    Ok(())
}

fn handle_mapping(subtle: &Subtle, event: MappingNotifyEvent) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    //conn.set_modifier_mapping(&[event.first_keycode])?;

    // Update grabs
    if Mapping::KEYBOARD == event.request {
        let default_screen = &conn.setup().roots[subtle.screen_num];

        grab::unset(subtle, default_screen.root)?;
        grab::set(subtle, default_screen.root, GrabFlags::IS_KEY)?;
    }

    debug!("{}", function_name!());

    Ok(())
}

fn handle_property(subtle: &Subtle, event: PropertyNotifyEvent) -> Result<()> {
    let atoms = subtle.atoms.get().unwrap();

    if atoms.WM_NAME == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            client.set_wm_name(subtle)?;

            if let Some(win) = subtle.focus_history.borrow(0)
                && event.window == *win
            {
                drop(client);

                panel::update(subtle)?;
                panel::render(subtle)?;
            }
        }
    } else if atoms.WM_NORMAL_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            client.set_size_hints(subtle, &mut mode_flags)?;

            let mut enable_only = client.flags.complement().intersection(mode_flags);

            client.toggle(subtle, &mut enable_only, true)?;

            if client.is_visible(subtle) {
                drop(client);

                panel::update(subtle)?;
                panel::render(subtle)?;
            }

        }
    } else if atoms.WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            client.set_wm_hints(subtle, &mut mode_flags)?;

            let mut enable_only = client.flags.complement().intersection(mode_flags);

            client.toggle(subtle, &mut enable_only, true)?;

            if client.is_visible(subtle) || client.flags.contains(ClientFlags::MODE_URGENT) {
                drop(client);

                panel::update(subtle)?;
                panel::render(subtle)?;
            }
        }
    } else if atoms._NET_WM_STRUT == event.atom {
        if let Some(client) = subtle.find_client_mut(event.window) {
            //client.set_strut(subtle)?;

            drop(client);

            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    } else if atoms._MOTIF_WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();
            let mut enable_only = client.flags.complement().intersection(mode_flags);

            client.toggle(subtle, &mut enable_only, true)?;
            client.set_motif_wm_hints(subtle, &mut mode_flags)?;
        }
    } else if atoms._XEMBED_INFO == event.atom {
        if let Some(mut tray) = subtle.find_tray_mut(event.window) {
            tray.set_state(subtle)?;

            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    }

    // TODO tray

    debug!("{}: win={}, atom={}", function_name!(), event.window, event.atom);

    Ok(())
}

fn handle_map_request(subtle: &Subtle, event: MapRequestEvent) -> Result<()> {
    // Check if we know the window
    if let Some(mut client) = subtle.find_client_mut(event.window) {
        client.flags.remove(ClientFlags::DEAD);
        client.flags.insert(ClientFlags::ARRANGE);

        screen::configure(subtle)?;
        panel::update(subtle)?;
        panel::render(subtle)?;
    } else if let Ok(client) = Client::new(subtle, event.window) {
        subtle.add_client(client);

        screen::configure(subtle)?;
        panel::update(subtle)?;
        panel::render(subtle)?;
    }

    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_unmap(subtle: &Subtle, event: UnmapNotifyEvent) -> Result<()> {
    // Check if we know the window
    if let Some(mut client) = subtle.find_client_mut(event.window) {
        // Set withdrawn state (see ICCCM 4.1.4)
        client.set_wm_state(subtle, WMState::Withdrawn)?;

        // Ignore our generated unmap events
        if client.flags.contains(ClientFlags::UNMAP) {
            client.flags.remove(ClientFlags::UNMAP);
        } else {
            client.kill(subtle)?;

            drop(client);

            subtle.remove_client_by_win(event.window);

            client::publish(subtle, false)?;

            screen::configure(subtle)?;
            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    } else if let Some(mut tray) = subtle.find_tray_mut(event.window) {
        // Set withdrawn state (see ICCCM 4.1.4)
        tray.set_wm_state(subtle, WMState::Withdrawn)?;

        // Ignore our generated unmap events
        if tray.flags.contains(TrayFlags::UNMAP) {
            tray.flags.remove(TrayFlags::UNMAP);
        } else {
            tray.kill(subtle)?;

            drop(tray);

            subtle.remove_client_by_win(event.window);

            tray::publish(subtle)?;

            screen::configure(subtle)?;
            panel::update(subtle)?;
            panel::render(subtle)?;
        }
    }

    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_selection(subtle: &Subtle, event: SelectionClearEvent) -> Result<()> {
    if event.owner == subtle.tray_win {
        unimplemented!()
    } else if event.owner == subtle.support_win {
        warn!("Leaving the field");

        subtle.shutdown.store(false, atomic::Ordering::Relaxed);
    }
    
    debug!("{}: win={}, tray={}, support={}",
        function_name!(), event.owner, subtle.tray_win, subtle.support_win);

    Ok(())
}

pub(crate) fn event_loop(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Update screen and panels
    screen::configure(subtle)?;
    panel::update(subtle)?;
    panel::render(subtle)?;

    // Set tray selection
    if subtle.flags.intersects(SubtleFlags::TRAY) {
        display::select_tray(subtle)?;
    }

    conn.flush()?;

    // Set grabs and focus first client if any
    let default_screen = &conn.setup().roots[subtle.screen_num];

    grab::set(subtle, default_screen.root, GrabFlags::IS_KEY)?;

    if let Some(client) = client::find_next(subtle, 0, false) {
        client.focus(subtle, true)?;
    }

    while !subtle.shutdown.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        if let Some(event) = conn.poll_for_event()? {
            match event {
                Event::ButtonPress(evt) => handle_button_press(subtle, evt)?,
                Event::ConfigureNotify(evt) => handle_configure(subtle, evt)?,
                Event::ConfigureRequest(evt) => handle_configure_request(subtle, evt)?,
                Event::ClientMessage(evt) => handle_client_message(subtle, evt)?,
                Event::DestroyNotify(evt) => handle_destroy(subtle, evt)?,
                Event::EnterNotify(evt) => handle_enter(subtle, evt)?,
                Event::LeaveNotify(evt) => handle_leave(subtle, evt)?,
                Event::Expose(evt) => handle_expose(subtle, evt)?,
                Event::FocusIn(evt) => handle_focus_in(subtle, evt)?,
                Event::KeyPress(evt) => handle_key_press(subtle, evt)?,
                Event::MappingNotify(evt) => handle_mapping(subtle, evt)?,
                Event::MapRequest(evt) => handle_map_request(subtle, evt)?,
                Event::PropertyNotify(evt) => handle_property(subtle, evt)?,
                Event::SelectionClear(evt) => handle_selection(subtle, evt)?,
                Event::UnmapNotify(evt) => handle_unmap(subtle, evt)?,

                _ => {
                    if subtle.flags.intersects(SubtleFlags::DEBUG) {
                        warn!("Unhandled event: {:?}", event)
                    }
                },
            }
        }
    }

    // Drop tray selection
    if subtle.flags.intersects(SubtleFlags::TRAY) {
        display::deselect_tray(subtle)?;
    }
    
    Ok(())
}