use log::warn;
use zbus::{Error, blocking::Connection, fdo::RequestNameFlags};

pub(crate) const BUS_NAME: &str = "org.opengamingcollective.cardwire.Gui";
pub(crate) const OBJECT_PATH: &str = "/org/opengamingcollective/cardwire/Gui";
/// Signal a later launch broadcasts to ask the running instance to raise its window.
pub(crate) const SHOW_SIGNAL: &str = "Show";

pub enum Acquisition {
    Acquired(Connection),
    AlreadyRunning,
    /// Could not be verified (e.g. no session bus available).
    Unchecked,
}

pub fn acquire() -> Acquisition {
    let connection = match Connection::session() {
        Ok(connection) => connection,
        Err(error) => {
            warn!(
                "could not connect to session bus to check for another running instance: {error}"
            );
            return Acquisition::Unchecked;
        }
    };

    match connection.request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into()) {
        Ok(_) => Acquisition::Acquired(connection),
        Err(Error::NameTaken) => {
            if let Err(error) =
                connection.emit_signal(None::<&str>, OBJECT_PATH, BUS_NAME, SHOW_SIGNAL, &())
            {
                warn!("could not ask the running instance to show its window: {error}");
            }
            Acquisition::AlreadyRunning
        }
        Err(error) => {
            warn!("could not request D-Bus name to check for another running instance: {error}");
            Acquisition::Unchecked
        }
    }
}
