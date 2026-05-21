use tokio::sync::mpsc;
use uuid::Uuid;

pub type DroppedConnectionReceiver = mpsc::UnboundedReceiver<Uuid>;
type DroppedConnectionSender = mpsc::UnboundedSender<Uuid>;

#[derive(Clone)]
pub struct ConnectionDropNotifier {
    connection_id: Uuid,
    sender: DroppedConnectionSender,
}

impl ConnectionDropNotifier {
    pub fn notify(&self) {
        if let Err(e) = self.sender.send(self.connection_id) {
            log::error!("Failed to publish dropped connection event: {e}");
        }
    }
}

#[derive(Clone)]
pub struct ConnectionMonitor {
    sender: DroppedConnectionSender,
}

impl ConnectionMonitor {
    pub fn new() -> (Self, DroppedConnectionReceiver) {
        let (sender, receiver) = mpsc::unbounded_channel();
        (Self { sender }, receiver)
    }

    pub fn notifier(&self, connection_id: Uuid) -> ConnectionDropNotifier {
        ConnectionDropNotifier {
            connection_id,
            sender: self.sender.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::*;

    #[tokio::test]
    async fn notifier_sends_dropped_connection_id() {
        let (monitor, mut dropped_connections) = ConnectionMonitor::new();
        let connection_id = Uuid::new_v4();

        monitor.notifier(connection_id).notify();

        let dropped_connection = timeout(Duration::from_secs(1), dropped_connections.recv())
            .await
            .expect("timed out waiting for dropped connection")
            .expect("dropped connection channel closed");

        assert_eq!(dropped_connection, connection_id);

        assert!(
            timeout(Duration::from_millis(50), dropped_connections.recv())
                .await
                .is_err(),
            "connection should only be reported once"
        );
    }
}
