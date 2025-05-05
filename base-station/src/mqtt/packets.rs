use mqttrs::{Connect, Packet, Pid, Protocol, Subscribe, SubscribeTopic, encode_slice};

use crate::error::BsError;

pub fn build_connect_packet(client_id: &str) -> Result<Vec<u8>, BsError> {
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

pub fn parse_connack(buf: &[u8]) -> bool {
    // 0x20 (CONNACK), 0x02 (remaining length), 0x00 (flags), 0x00 (return code)
    buf.len() == 4 && buf[0] == 0x20 && buf[1] == 0x02 && buf[3] == 0x00
}

pub fn build_subscribe_packet(topic: &str) -> Result<Vec<u8>, BsError> {
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
