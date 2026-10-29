//! Responsible for specifically receiving terminal input
//! using [`crossterm`].

use crate::{config::BindingMap, Message};
use crossterm::event::{self, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures_util::{FutureExt, StreamExt};
use tokio::sync::mpsc::Sender;

/// Starts the listener to receive input from the terminal for various events.
pub async fn listen(sender: Sender<Message>, bindings: BindingMap) -> super::Result<()> {
    let mut reader = EventStream::new();

    loop {
        let Some(Ok(event::Event::Key(event))) = reader.next().fuse().await else {
            continue;
        };

        if event.kind == KeyEventKind::Release {
            continue;
        }

        let messages = match message_for(event.code, event.modifiers, &bindings) {
            Some(message) => message,
            None => continue,
        };
        sender.send(messages).await?;
    }
}

fn message_for(code: KeyCode, modifiers: KeyModifiers, bindings: &BindingMap) -> Option<Message> {
    match code {
        KeyCode::Char('c') if modifiers == KeyModifiers::CONTROL => Some(Message::Quit),
        KeyCode::Char(character) => {
            bindings.message(&KeyCode::Char(character.to_ascii_lowercase()))
        }
        KeyCode::Up | KeyCode::Right | KeyCode::Down | KeyCode::Left => bindings.message(&code),
        KeyCode::Media(media) => match media {
            event::MediaKeyCode::Pause
            | event::MediaKeyCode::Play
            | event::MediaKeyCode::PlayPause => Some(Message::PlayPause),
            event::MediaKeyCode::Stop => Some(Message::Pause),
            event::MediaKeyCode::TrackNext => Some(Message::Next),
            event::MediaKeyCode::LowerVolume => Some(Message::ChangeVolume(-0.1)),
            event::MediaKeyCode::RaiseVolume => Some(Message::ChangeVolume(0.1)),
            event::MediaKeyCode::MuteVolume => Some(Message::ChangeVolume(-1.0)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn bindings_are_case_insensitive_and_ctrl_c_is_reserved() {
        let (_, bindings) = crate::config::load(
            Path::new("missing.toml"),
            crate::config::CliOverrides::default(),
        )
        .unwrap();
        assert_eq!(
            message_for(KeyCode::Char('P'), KeyModifiers::NONE, &bindings),
            Some(Message::PlayPause)
        );
        assert_eq!(
            message_for(KeyCode::Char('c'), KeyModifiers::CONTROL, &bindings),
            Some(Message::Quit)
        );
    }
}
