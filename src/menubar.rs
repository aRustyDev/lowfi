//! Native macOS menu-bar frontend. All AppKit-backed objects stay on this thread.

use std::thread;

use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::macos::{ActivationPolicy, EventLoopExtMacOS},
};
use tokio::sync::{mpsc, watch};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};

use crate::{config, player::PlaybackSnapshot, Message};

enum UserEvent {
    Menu(MenuEvent),
    Snapshot(PlaybackSnapshot),
}

/// Starts the main-thread menu event loop and the shared player worker.
pub fn run(
    settings: config::Settings,
    bindings: config::BindingMap,
    terminal: bool,
) -> crate::Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();
    let (commands, command_rx) = mpsc::unbounded_channel();
    let (state_tx, state_rx) = watch::channel(PlaybackSnapshot::default());

    let observer_proxy = proxy.clone();
    let observer = thread::Builder::new()
        .name("lowfi-menubar-state".into())
        .spawn(move || {
            let mut state_rx = state_rx;
            while state_rx.blocking_changed().is_ok() {
                let _ = observer_proxy.send_event(UserEvent::Snapshot(state_rx.borrow().clone()));
            }
        })?;
    let worker = thread::Builder::new()
        .name("lowfi-player".into())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(crate::Error::from)?;
            runtime.block_on(crate::run(
                settings,
                bindings,
                terminal,
                Some(state_tx),
                Some(command_rx),
            ))
        })?;

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let mut state = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let menu = Menu::new();
                let status = MenuItem::with_id("status", "Loading…", false, None);
                let play_pause = MenuItem::with_id("play-pause", "Pause", true, None);
                let next = MenuItem::with_id("next", "Next", true, None);
                let quit = MenuItem::with_id("quit", "Quit", true, None);
                if menu
                    .append_items(&[
                        &status,
                        &PredefinedMenuItem::separator(),
                        &play_pause,
                        &next,
                        &PredefinedMenuItem::separator(),
                        &quit,
                    ])
                    .is_err()
                {
                    eprintln!("lowfi: unable to create the menu bar menu");
                    let _ = commands.send(Message::Quit);
                    *control_flow = ControlFlow::Exit;
                    return;
                }
                match TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_icon(icon())
                    .with_icon_as_template(true)
                    .with_tooltip("Lowfi")
                    .build()
                {
                    Ok(tray) => state = Some((tray, status, play_pause, next, quit)),
                    Err(error) => {
                        eprintln!("lowfi: unable to initialize menu bar icon: {error}");
                        let _ = commands.send(Message::Quit);
                        *control_flow = ControlFlow::Exit;
                    }
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) => {
                let Some((_, _, _, _, quit)) = &state else {
                    return;
                };
                if event.id == quit.id() {
                    let _ = commands.send(Message::Quit);
                    *control_flow = ControlFlow::Exit;
                } else if event.id.as_ref() == "play-pause" {
                    let _ = commands.send(Message::PlayPause);
                } else if event.id.as_ref() == "next" {
                    let _ = commands.send(Message::Next);
                }
            }
            Event::UserEvent(UserEvent::Snapshot(snapshot)) => {
                if let Some((_, status, play_pause, next, _)) = &state {
                    status.set_text(snapshot.title.unwrap_or_else(|| "Loading…".into()));
                    play_pause.set_text(if snapshot.paused { "Play" } else { "Pause" });
                    next.set_enabled(!snapshot.loading);
                }
            }
            Event::LoopDestroyed => {
                let _ = commands.send(Message::Quit);
            }
            _ => (),
        }
    });

    MenuEvent::set_event_handler(None);
    let player_result = worker
        .join()
        .map_err(|_| crate::Error::MenubarUnavailable)?;
    let _ = observer.join();
    player_result
}

fn icon() -> Icon {
    let mut rgba = vec![0; 16 * 16 * 4];
    for y in 2..14 {
        for x in 2..14 {
            if (x - 8) * (x - 8) + (y - 8) * (y - 8) <= 36 {
                let pixel = (y * 16 + x) * 4;
                rgba[pixel..pixel + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, 16, 16).expect("embedded icon dimensions are valid")
}
