//! Main-thread macOS menu-bar frontend.

use std::{sync::mpsc, thread};

use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder},
    platform::{
        macos::{ActivationPolicy, EventLoopExtMacOS},
        run_return::EventLoopExtRunReturn,
    },
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    Icon, TrayIconBuilder,
};

use crate::{app::Application, config, player::PlaybackSnapshot, Message};

enum UserEvent {
    Menu(MenuEvent),
    Snapshot(PlaybackSnapshot),
}

/// Runs Tao on the macOS main thread while playback runs on one worker thread.
pub(crate) fn run(
    settings: config::Settings,
    bindings: config::BindingMap,
    terminal: bool,
) -> crate::Result<()> {
    let mut event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    let worker = thread::Builder::new().name("lowfi-player".into()).spawn(
        move || -> crate::Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(async move {
                let stream = crate::audio::stream()?;
                let mut app =
                    Application::init(settings, bindings, stream.mixer(), terminal).await?;
                let commands = app.sender();
                let mut snapshots = app.subscribe();
                let observer = proxy.clone();
                let _ = observer.send_event(UserEvent::Snapshot(snapshots.borrow().clone()));
                tokio::spawn(async move {
                    while snapshots.changed().await.is_ok() {
                        let _ =
                            observer.send_event(UserEvent::Snapshot(snapshots.borrow().clone()));
                    }
                });
                ready_tx
                    .send(commands)
                    .map_err(|_| crate::Error::MenubarUnavailable)?;
                app.run().await?;
                app.close().await
            })
        },
    )?;
    let commands = ready_rx
        .recv()
        .map_err(|_| crate::Error::MenubarUnavailable)?;
    let menu_proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = menu_proxy.send_event(UserEvent::Menu(event));
    }));

    let mut controls = None;
    event_loop.run_return(|event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::NewEvents(StartCause::Init) => {
                let menu = Menu::new();
                let status = MenuItem::with_id("status", "Loading…", false, None);
                let play = MenuItem::with_id("play", "Pause", true, None);
                let next = MenuItem::with_id("next", "Next", false, None);
                let quit = MenuItem::with_id("quit", "Quit", true, None);
                let _ = menu.append_items(&[
                    &status,
                    &PredefinedMenuItem::separator(),
                    &play,
                    &next,
                    &PredefinedMenuItem::separator(),
                    &quit,
                ]);
                match TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_icon(icon())
                    .with_icon_as_template(true)
                    .with_tooltip("Lowfi")
                    .build()
                {
                    Ok(tray) => controls = Some((tray, status, play, next, quit)),
                    Err(error) => {
                        eprintln!("lowfi: unable to initialize menu bar icon: {error}");
                        let _ = commands.send(Message::Quit);
                        *flow = ControlFlow::Exit;
                    }
                }
            }
            Event::UserEvent(UserEvent::Menu(event)) => {
                if event.id.as_ref() == "quit" {
                    let _ = commands.send(Message::Quit);
                    *flow = ControlFlow::Exit;
                } else if event.id.as_ref() == "play" {
                    let _ = commands.send(Message::PlayPause);
                } else if event.id.as_ref() == "next" {
                    let _ = commands.send(Message::Next);
                }
            }
            Event::UserEvent(UserEvent::Snapshot(snapshot)) => {
                if let Some((_, status, play, next, _)) = &controls {
                    status.set_text(
                        snapshot
                            .title
                            .unwrap_or_else(|| format!("{} — Loading…", snapshot.track_list)),
                    );
                    play.set_text(if snapshot.paused { "Play" } else { "Pause" });
                    next.set_enabled(!snapshot.loading);
                }
            }
            _ => (),
        }
    });
    let _ = commands.send(Message::Quit);
    worker
        .join()
        .map_err(|_| crate::Error::MenubarUnavailable)?
}

fn icon() -> Icon {
    let mut pixels = vec![0; 16 * 16 * 4];
    for y in 2usize..14 {
        for x in 2usize..14 {
            if (x as i32 - 8).pow(2) + (y as i32 - 8).pow(2) <= 36 {
                let start = (y * 16 + x) * 4;
                pixels[start..start + 4].copy_from_slice(&[0, 0, 0, 255]);
            }
        }
    }
    Icon::from_rgba(pixels, 16, 16).expect("valid embedded icon")
}
