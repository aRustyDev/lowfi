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
    Stopped,
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
    let worker_proxy = proxy.clone();
    let worker = thread::Builder::new().name("lowfi-player".into()).spawn(
        move || -> crate::Result<()> {
            let result = (|| -> crate::Result<()> {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                runtime.block_on(async move {
                    let stream = crate::audio::stream()?;
                    let environment = terminal
                        .then(|| crate::ui::Environment::ready(&settings))
                        .transpose()?;
                    let mut app =
                        match Application::init(settings, bindings, stream.mixer(), terminal).await
                        {
                            Ok(app) => app,
                            Err(error) => {
                                if let Some(environment) = environment {
                                    environment.cleanup(false)?;
                                }
                                return Err(error);
                            }
                        };
                    let commands = app.sender();
                    let mut snapshots = app.subscribe();
                    let observer = proxy.clone();
                    let _ = observer.send_event(UserEvent::Snapshot(snapshots.borrow().clone()));
                    tokio::spawn(async move {
                        while snapshots.changed().await.is_ok() {
                            let _ = observer
                                .send_event(UserEvent::Snapshot(snapshots.borrow().clone()));
                        }
                    });
                    ready_tx
                        .send(commands)
                        .map_err(|_| crate::Error::MenubarUnavailable)?;
                    let result = app.run().await;
                    if let Some(environment) = environment {
                        environment.cleanup(result.is_ok())?;
                    }
                    app.close().await?;
                    result
                })
            })();
            let _ = worker_proxy.send_event(UserEvent::Stopped);
            result
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
            Event::UserEvent(UserEvent::Stopped) => *flow = ControlFlow::Exit,
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
    MenuEvent::set_event_handler(None::<fn(MenuEvent)>);
    let _ = commands.send(Message::Quit);
    worker
        .join()
        .map_err(|_| crate::Error::MenubarUnavailable)?
}

const ICON_PIXELS: u32 = 36;
const ICON_SVG: &[u8] = include_bytes!("../docs/media/menubar.svg");

fn icon() -> Icon {
    let pixels = rasterize_icon(ICON_PIXELS)
        .expect("the bundled Lowfi menu-bar SVG must parse and rasterize");
    Icon::from_rgba(pixels, ICON_PIXELS, ICON_PIXELS)
        .expect("the bundled Lowfi menu-bar icon raster must be valid")
}

fn rasterize_icon(size: u32) -> Result<Vec<u8>, String> {
    let tree = resvg::usvg::Tree::from_data(ICON_SVG, &resvg::usvg::Options::default())
        .map_err(|error| error.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .ok_or_else(|| format!("unable to allocate a {size} by {size} icon buffer"))?;
    let scale = size as f32 / 24.0;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_icon_rasterizes_at_menu_bar_sizes() {
        for size in [16, 18, 22, 32, 36, 44] {
            let pixels = rasterize_icon(size).expect("bundled SVG is valid");
            assert_eq!(pixels.len(), (size * size * 4) as usize);

            let alphas = pixels.chunks_exact(4).map(|pixel| pixel[3]);
            assert!(alphas.clone().any(|alpha| alpha > 0), "{size}px has ink");
            assert!(
                alphas.clone().any(|alpha| alpha < u8::MAX),
                "{size}px keeps transparent padding"
            );
            assert!(
                pixels
                    .chunks_exact(4)
                    .filter(|pixel| pixel[3] > 0)
                    .all(|pixel| pixel[..3] == [0, 0, 0]),
                "{size}px is monochrome black"
            );
            let size = size as usize;
            let alpha_at = |x: usize, y: usize| pixels[(y * size + x) * 4 + 3];
            assert!(
                (0..size).all(|x| alpha_at(x, 0) == 0 && alpha_at(x, size - 1) == 0),
                "{size}px has vertical padding"
            );
            assert!(
                (0..size).all(|y| alpha_at(0, y) == 0 && alpha_at(size - 1, y) == 0),
                "{size}px has horizontal padding"
            );
            assert!(
                pixels
                    .chunks_exact(4)
                    .any(|pixel| (1..u8::MAX).contains(&pixel[3])),
                "{size}px has antialiased edges"
            );
        }
    }

    #[test]
    fn tray_icon_constructs_from_the_retina_raster() {
        let _icon = icon();
    }
}
