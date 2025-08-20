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
use log::{debug, warn};
use stdext::function_name;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ButtonPressEvent, ConfigureNotifyEvent, ConfigureRequestEvent, ConfigureWindowAux, ConnectionExt, DestroyNotifyEvent, EnterNotifyEvent, ExposeEvent, FocusInEvent, KeyPressEvent, LeaveNotifyEvent, MapRequestEvent, Mapping, MappingNotifyEvent, ModMask, PropertyNotifyEvent, SelectionClearEvent, UnmapNotifyEvent};
use x11rb::protocol::Event;
use crate::subtle::{SubtleFlags, Subtle};
use crate::client::{Client, ClientFlags, WMState};
use crate::{client, grab, screen};
use crate::grab::GrabFlags;
use crate::panel::PanelAction;

fn handle_button_press(subtle: &Subtle, event: ButtonPressEvent) -> Result<()> {
    if let Some((_, screen)) = subtle.find_screen_by_panel_win(event.event) {
        screen.handle_action(
            subtle,
            &PanelAction::MouseDown(event.event_x, event.event_y, event.detail as i8),
            screen.bottom_panel_win == event.event)?;

        // Finally configure and render
        screen::configure(subtle)?;
        screen::render(subtle)?;
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

    // Check window
    if let Some(client) = subtle.find_client_mut(event.window) {
        // Check flags if the request is important
        if !client.flags.contains(ClientFlags::MODE_FULL)
            && subtle.flags.contains(SubtleFlags::RESIZE)
            || client.flags.contains(ClientFlags::MODE_FLOAT | ClientFlags::MODE_RESIZE)
        {
            let maybe_screen = subtle.screens.get(client.screen_id as usize);
        }
    // Unmanaged window
    } else {
        let aux = ConfigureWindowAux::default()
            .x(event.x as i32)
            .y(event.y as i32)
            .width(event.width as u32)
            .height(event.height as u32)
            .border_width(0)
            .sibling(event.sibling)
            .stack_mode(event.stack_mode);

        conn.configure_window(event.window, &aux)?.check()?;
    }

    Ok(())
}

fn handle_destroy(subtle: &Subtle, event: DestroyNotifyEvent) -> Result<()> {
    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_enter(subtle: &Subtle, event: EnterNotifyEvent) -> Result<()> {
    if let Some(client) = subtle.find_client_mut(event.event) {
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
        screen::render(subtle)?;
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
        if let Some(mut focus) = subtle.focus_history.borrow_mut(0) {
            *focus = event.event;
        }

        // Update screen
        screen::update(subtle)?;
        screen::render(subtle)?;
    }

    debug!("{}: win={}", function_name!(), event.event);

    Ok(())
}

fn handle_key_press(subtle: &Subtle, event: KeyPressEvent) -> Result<()> {
    // Hacky conversion
    let bits: u16 = event.state.into();
    let mod_mask = ModMask::from(bits & 0xFF);

    println!("mod_mask={:?}", mod_mask);

    if let Some(grab) = subtle.find_grab(event.detail, mod_mask) {
        println!("grab={:?}", grab);
    }

    screen::update(subtle)?;
    screen::render(subtle)?;

    // Restore binds
    let conn = subtle.conn.get().context("Failed to get connection")?;
    let default_screen = &conn.setup().roots[subtle.screen_num];

    grab::unset(subtle, default_screen.root)?;
    grab::set(subtle, default_screen.root, GrabFlags::IS_KEY)?;

    println!("{}: win={}", function_name!(), event.event);

    Ok(())
}

fn handle_mapping(subtle: &Subtle, event: MappingNotifyEvent) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    conn.set_modifier_mapping(&[event.first_keycode])?;

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

                screen::update(subtle)?;
                screen::render(subtle)?;
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

                screen::update(subtle)?;
                screen::render(subtle)?;
            }

        }
        // TODO tray
    } else if atoms.WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();

            client.set_wm_hints(subtle, &mut mode_flags)?;

            let mut enable_only = client.flags.complement().intersection(mode_flags);

            client.toggle(subtle, &mut enable_only, true)?;

            if client.is_visible(subtle) || client.flags.contains(ClientFlags::MODE_URGENT) {
                drop(client);

                screen::update(subtle)?;
                screen::render(subtle)?;
            }
        }
    } else if atoms._NET_WM_STRUT == event.atom {
        if let Some(client) = subtle.find_client_mut(event.window) {
            //client.set_strut(subtle)?;

            drop(client);

            screen::update(subtle)?;
            screen::render(subtle)?;
        }
    } else if atoms._MOTIF_WM_HINTS == event.atom {
        if let Some(mut client) = subtle.find_client_mut(event.window) {
            let mut mode_flags = ClientFlags::empty();
            let mut enable_only = client.flags.complement().intersection(mode_flags);

            client.toggle(subtle, &mut enable_only, true)?;
            client.set_motif_wm_hints(subtle, &mut mode_flags)?;
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

        drop(client);

        screen::configure(subtle)?;
        screen::update(subtle)?;
        screen::render(subtle)?;
    } else if let Ok(client) = Client::new(subtle, event.window) {
        //subtle.clients.push(client);

        drop(client);

        screen::configure(subtle)?;
        screen::update(subtle)?;
        screen::render(subtle)?;
    }

    debug!("{}: win={}", function_name!(), event.window);

    Ok(())
}

fn handle_unmap(subtle: &Subtle, event: UnmapNotifyEvent) -> Result<()> {
    // Check if we know the window
    if let Some(mut client) = subtle.find_client_mut(event.window) {
        // Set withdrawn state (see ICCCM 4.1.4)
        let _ = client.set_wm_state(subtle, WMState::WithdrawnState);

        // Ignore our generated unmap events
        if client.flags.contains(ClientFlags::UNMAP) {
            client.flags.remove(ClientFlags::UNMAP);
        } else {
            // Kill client
            //subtle.clients.pop(client);
            //client.kill(subtle);

            drop(client);

            client::publish(subtle, false)?;

            screen::configure(subtle)?;
            screen::update(subtle)?;
            screen::render(subtle)?;
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

        subtle.exterminate.store(false, atomic::Ordering::Relaxed);
    }
    
    debug!("{}: win={}, tray={}, support={}",
        function_name!(), event.owner, subtle.tray_win, subtle.support_win);

    Ok(())
}

pub(crate) fn event_loop(subtle: &Subtle) -> Result<()> {
    let conn = subtle.conn.get().context("Failed to get connection")?;

    // Update screen and panels
    screen::configure(subtle)?;
    screen::update(subtle)?;
    screen::render(subtle)?;

    conn.flush()?;

    // Set grabs and focus first client if any
    let default_screen = &conn.setup().roots[subtle.screen_num];

    grab::set(subtle, default_screen.root, GrabFlags::IS_KEY)?;

    if let Some(client) = client::find_next(subtle, 0, false) {
        client.focus(subtle, true)?;
    }

    while !subtle.exterminate.load(atomic::Ordering::SeqCst) {
        conn.flush()?;

        if let Some(event) = conn.poll_for_event()? {
            match event {
                Event::ButtonPress(evt) => handle_button_press(subtle, evt)?,
                Event::ConfigureNotify(evt) => handle_configure(subtle, evt)?,
                Event::ConfigureRequest(evt) => handle_configure_request(subtle, evt)?,
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
    
    Ok(())
}