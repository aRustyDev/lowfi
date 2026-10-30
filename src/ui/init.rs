//! Terminal frontend initialization.

use crate::{
    player,
    ui::{self, input, interface},
    Tasks,
};
use tokio::sync::broadcast;

/// Starts terminal input and rendering and returns its downloader logger.
pub(crate) fn terminal(
    tasks: &mut Tasks,
    state: player::State,
    settings: &crate::config::Settings,
    bindings: crate::config::BindingMap,
    receiver: broadcast::Receiver<player::Update>,
    terminal: bool,
) -> crate::Result<interface::Logger> {
    if !terminal {
        return Ok(interface::Logger::stderr());
    }

    let params = interface::Params::try_from(settings)?;
    let interface = interface::Interface::new(params)?;
    let logger = interface.logger.clone();
    if params.enabled {
        tasks.spawn(ui::run(receiver, interface, ui::State::new(state)));
        tasks.spawn(input::listen(tasks.tx(), bindings));
    }
    Ok(logger)
}
