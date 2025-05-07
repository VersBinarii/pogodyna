use std::time::Duration;

use packets::{build_connect_packet, build_subscribe_packet, parse_connack};
use read_loop::handle_packet;
use tokio::net::TcpStream;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::db::Repository;
use crate::error::BsError;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, Notify};

mod packets;
mod read_loop;

#[derive(Debug, PartialEq)]
pub enum ReadLoopResult {
    Ok,
    Skipped,
    Shutdown,
    Unknown,
}

pub struct MqttClient<R> {
    writer: Arc<Mutex<Option<WriteHalf<TcpStream>>>>,
    broker_addr: String,
    id: String,
    repository: R,
    shutdown_notify: Arc<Notify>,
    connected_notify: Arc<Notify>,
}

impl<R> MqttClient<R>
where
    R: Repository + Send + Sync + 'static,
{
    pub async fn run_forever(
        broker_addr: String,
        id: String,
        repository: R,
    ) -> (Arc<Self>, JoinHandle<()>) {
        let client = Arc::new(MqttClient {
            writer: Arc::new(Mutex::new(None)),
            broker_addr,
            id,
            repository,
            shutdown_notify: Arc::new(Notify::new()),
            connected_notify: Arc::new(Notify::new()),
        });

        let client_clone = client.clone();
        let connection_loop_handle = tokio::spawn(async move {
            client_clone.connection_loop().await;
        });

        (client, connection_loop_handle)
    }
    async fn connection_loop(self: Arc<Self>) {
        loop {
            match TcpStream::connect(&self.broker_addr).await {
                Ok(mut stream) => {
                    info!("Connected to {}", self.broker_addr);

                    let connect_packet = build_connect_packet(&self.id).unwrap();
                    if let Err(e) = stream.write_all(&connect_packet).await {
                        error!("[mqtt] Failed to send CONNECT: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    info!("Connect sent - awaiting ack");

                    let mut connack_buf = [0u8; 4];
                    if let Err(e) = stream.read_exact(&mut connack_buf).await {
                        error!("[mqtt] Failed to read CONNACK: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }

                    info!("Received packet - looking for ack");

                    if !parse_connack(&connack_buf) {
                        error!("[mqtt] Invalid CONNACK received: {:x?}", connack_buf);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }

                    info!("Connection ACKed");
                    let (read_half, write_half) = tokio::io::split(stream);

                    {
                        let mut writer_lock = self.writer.lock().await;
                        *writer_lock = Some(write_half);
                    }

                    // Spawn the read task
                    info!("Starting subscription handler");
                    let shutdown_clone = self.shutdown_notify.clone();
                    let this_self = self.clone();
                    let read_handle = tokio::spawn(async move {
                        tokio::select! {
                            _ = this_self.read_loop(read_half) => {
                            warn!("Read loop exited but will attempt to re-connect to broker");
                            },
                            _ = shutdown_clone.notified() => {
                                info!("[mqtt] Shutdown signal received in read loop.");
                            }
                            // TODO: Add keepalive sender
                        }
                    });

                    if let Err(e) = read_handle.await {
                        error!("[mqtt] Read task join error: {:?}", e);
                    }
                }
                Err(e) => {
                    error!("[mqtt] Connection failed: {}", e);
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = self.shutdown_notify.notified() => {
                    info!("[mqtt] Shutdown during reconnect delay.");
                    break;
                }
            }
        }
    }
    async fn read_loop(&self, mut reader: ReadHalf<TcpStream>) -> Result<(), BsError> {
        // NOTE: This should be fine for now but it should be handled in a nicer way
        // since MQTT packets can be way way larger
        let mut buf = [0u8; 2048];

        debug!("sending subscription");
        if let Err(e) = self.subscribe("sensor/update").await {
            error!("Error sending subscribe: {}", e);
            return Err(e);
        }
        self.connected_notify.notify_one();
        debug!("read_loop started");
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                warn!("[mqtt] EOF from broker");
                break;
            }

            // TODO: We make single attempt at the packet parsing
            // but if the packet is larger than a buffer
            // we should pull out more bytes and attempt parsing again
            handle_packet(&self.repository, &buf[..n]).await?;
        }

        Ok(())
    }

    pub async fn wait_for_server_setup(&self) {
        self.connected_notify.notified().await;
    }

    pub async fn subscribe(&self, topic: &str) -> Result<(), BsError> {
        let packet = build_subscribe_packet(topic)?;
        if let Some(ref mut writer) = *self.writer.lock().await {
            writer.write_all(&packet).await?;
        }
        Ok(())
    }

    pub fn shutdown(&self) {
        self.shutdown_notify.notify_waiters();
    }
}
