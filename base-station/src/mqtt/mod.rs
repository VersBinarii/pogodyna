use std::time::Duration;

use mqttrs::{
    Connect, Packet, Pid, Protocol, Subscribe, SubscribeTopic, decode_slice, encode_slice,
};
use tokio::net::TcpStream;
use tokio::task::JoinHandle;

use crate::SensorReading;
use crate::db::Repository;
use crate::error::BsError;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, ReadHalf, WriteHalf};
use tokio::sync::{Mutex, Notify};

pub struct MqttClient<R> {
    writer: Arc<Mutex<Option<WriteHalf<TcpStream>>>>,
    read_task: Arc<Mutex<Option<JoinHandle<()>>>>,
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
            read_task: Arc::new(Mutex::new(None)),
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
                    println!("Connected to {}", self.broker_addr);

                    let connect_packet = build_connect_packet(&self.id).unwrap();
                    if let Err(e) = stream.write_all(&connect_packet).await {
                        eprintln!("[mqtt] Failed to send CONNECT: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                    println!("Connect sent - awaiting ack");

                    let mut connack_buf = [0u8; 4];
                    if let Err(e) = stream.read_exact(&mut connack_buf).await {
                        eprintln!("[mqtt] Failed to read CONNACK: {}", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }

                    println!("Received packet - looking for ack");

                    if !parse_connack(&connack_buf) {
                        eprintln!("[mqtt] Invalid CONNACK received: {:x?}", connack_buf);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }

                    println!("Connection ACKed");
                    let (read_half, write_half) = tokio::io::split(stream);

                    {
                        let mut writer_lock = self.writer.lock().await;
                        *writer_lock = Some(write_half);
                    }

                    // Spawn the read task
                    println!("Starting subscription handler");
                    let shutdown_clone = self.shutdown_notify.clone();
                    let this_self = self.clone();
                    let read_handle = tokio::spawn(async move {
                        tokio::select! {
                            _ = this_self.read_loop(read_half) => {},
                            _ = shutdown_clone.notified() => {
                                println!("[mqtt] Shutdown signal received in read loop.");
                                break;
                            }
                        }
                    });

                    {
                        let mut read_task_lock = self.read_task.lock().await;
                        *read_task_lock = Some(read_handle);
                    }

                    if let Some(handle) = self.read_task.lock().await.take() {
                        if let Err(e) = handle.await {
                            eprintln!("[mqtt] Read task join error: {:?}", e);
                        }
                    }
                }

                Err(e) => {
                    eprintln!("[mqtt] Connection failed: {}", e);
                }
            }
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(5)) => {},
                _ = self.shutdown_notify.notified() => {
                    println!("[mqtt] Shutdown during reconnect delay.");
                    break;
                }
            }
        }
    }
    async fn read_loop(&self, mut reader: ReadHalf<TcpStream>) -> Result<(), BsError> {
        let mut buf = [0u8; 1024];

        println!("sending subscription");
        if let Err(e) = self.subscribe("sensor/update").await {
            eprintln!("Error sending subscribe: {}", e);
            return Err(e);
        }
        self.connected_notify.notify_one();
        println!("read_loop started");
        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                println!("[mqtt] EOF from broker");
                break;
            }

            self.handle_packet(&buf[..n]).await;
        }

        Ok(())
    }

    async fn handle_packet(&self, packet: &[u8]) {
        if is_mqtt_packet(packet[0]) {
            match decode_slice(packet) {
                Ok(Some(Packet::Publish(publish))) => {
                    let sensor_reading: SensorReading =
                        serde_json::from_slice(publish.payload).unwrap();
                    println!("Got update: {sensor_reading}");
                    let _ = self.repository.insert_sensor_reading(sensor_reading).await;
                }
                _ => println!("We have some packet we dont care about for now"),
            }
        } else {
            println!("Ignoring unknown packet");
        }
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

fn is_mqtt_packet(first_byte: u8) -> bool {
    let packet_type = first_byte >> 4;
    (1..=14).contains(&packet_type)
}

fn build_connect_packet(client_id: &str) -> Result<Vec<u8>, BsError> {
    let packet: Packet = Connect {
        protocol: Protocol::MQTT311,
        keep_alive: 120,
        client_id,
        clean_session: true,
        last_will: None,
        username: None,
        password: None,
    }
    .into();

    let mut buf = [0u8; 64];
    let packet_length = encode_slice(&packet, &mut buf)?;
    Ok(buf[..packet_length].to_vec())
}

fn parse_connack(buf: &[u8]) -> bool {
    // 0x20 (CONNACK), 0x02 (remaining length), 0x00 (flags), 0x00 (return code)
    buf.len() == 4 && buf[0] == 0x20 && buf[1] == 0x02 && buf[3] == 0x00
}

fn build_subscribe_packet(topic: &str) -> Result<Vec<u8>, BsError> {
    let subscribe_topic = SubscribeTopic {
        topic_path: String::from(topic),
        qos: mqttrs::QoS::AtMostOnce,
    };
    let topics = vec![subscribe_topic];
    let packet: Packet = Subscribe {
        pid: Pid::default(),
        topics,
    }
    .into();
    let mut buf = [0u8; 64];
    let packet_length = encode_slice(&packet, &mut buf)?;
    Ok(buf[..packet_length].to_vec())
}
