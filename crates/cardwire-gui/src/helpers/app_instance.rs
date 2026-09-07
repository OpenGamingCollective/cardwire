use std::{
    hash::{Hash, Hasher},
    sync::Arc,
    time::Duration,
};

use iced::{Subscription, futures::stream};
use tokio::sync::Notify;
use zbus::{Connection, connection::Builder, fdo::RequestNameFlags};

const BUS_NAME: &str = "org.opengamingcollective.cardwire.Gui";
const OBJECT_PATH: &str = "/org/opengamingcollective/cardwire/Gui";

struct ActivationInterface(Arc<Notify>);

#[zbus::interface(name = "org.opengamingcollective.cardwire.Gui")]
impl ActivationInterface {
    fn activate(&self) {
        // Keep a permit if the GUI has not started listening yet. Repeated
        // requests can be coalesced because they all open the same window.
        self.0.notify_one();
    }
}

/// Owns the session bus name for as long as the GUI is running.
#[derive(Debug, Clone)]
pub struct AppInstance {
    connection: Connection,
    activation: Arc<Notify>,
}

impl Hash for AppInstance {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.connection.unique_name().hash(state);
    }
}

impl AppInstance {
    /// Returns `None` when another instance owns the name. Explicit background
    /// launches leave that instance hidden; normal launches ask it to open.
    pub async fn acquire(activate_existing: bool) -> zbus::Result<Option<Self>> {
        let activation = Arc::new(Notify::new());
        let connection = Builder::session()?
            .method_timeout(Duration::from_secs(5))
            // Export before claiming the name so simultaneous launches can
            // immediately call Activate on the winner.
            .serve_at(OBJECT_PATH, ActivationInterface(activation.clone()))?
            .build()
            .await?;

        match connection
            .request_name_with_flags(BUS_NAME, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(_) => Ok(Some(Self {
                connection,
                activation,
            })),
            Err(zbus::Error::NameTaken) => {
                if activate_existing {
                    connection
                        .call_method(Some(BUS_NAME), OBJECT_PATH, Some(BUS_NAME), "Activate", &())
                        .await?;
                }
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    pub fn subscription(&self) -> Subscription<()> {
        Subscription::run_with(self.clone(), |instance| {
            stream::unfold(instance.activation.clone(), |activation| async move {
                activation.notified().await;
                Some(((), activation))
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced::futures::FutureExt;

    #[tokio::test]
    #[ignore = "run under dbus-run-session to isolate the GUI bus name"]
    async fn exclusive_ownership_activation_and_release() {
        let primary = AppInstance::acquire(true).await.unwrap().unwrap();

        assert!(AppInstance::acquire(false).await.unwrap().is_none());
        assert!(primary.activation.notified().now_or_never().is_none());

        // Activation must survive arrival before the GUI subscribes.
        assert!(AppInstance::acquire(true).await.unwrap().is_none());
        tokio::time::timeout(Duration::from_secs(1), primary.activation.notified())
            .await
            .unwrap();

        primary.connection.close().await.unwrap();

        // Concurrent startups elect exactly one owner, without stale locks.
        let (first, second) = tokio::join!(AppInstance::acquire(true), AppInstance::acquire(true));
        let first = first.unwrap();
        let second = second.unwrap();
        assert_ne!(first.is_some(), second.is_some());
        let winner = first.or(second).unwrap();
        tokio::time::timeout(Duration::from_secs(1), winner.activation.notified())
            .await
            .unwrap();
        winner.connection.close().await.unwrap();
    }
}
