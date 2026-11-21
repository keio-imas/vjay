use std::{
    net::UdpSocket,
    sync::mpsc::{self, Receiver, SyncSender},
};
use vjay_application::{control_io::*, ports::PortResult};
mod midi;

pub struct NativeControlIo {
    send: SyncSender<ControlEvent>,
    receive: Receiver<ControlEvent>,
    midi: Option<midi::MidiInputs>,
    socket: Option<UdpSocket>,
    port: Option<u16>,
    host: String,
}
impl Default for NativeControlIo {
    fn default() -> Self {
        let (send, receive) = mpsc::sync_channel(1024);
        Self {
            send,
            receive,
            midi: None,
            socket: None,
            port: None,
            host: String::new(),
        }
    }
}
impl ControlIo for NativeControlIo {
    fn configure(&mut self, settings: &InputSettings) -> PortResult<String> {
        let socket = match settings.osc_port {
            Some(port) if Some(port) == self.port && settings.osc_host == self.host => self
                .socket
                .as_ref()
                .map(UdpSocket::try_clone)
                .transpose()
                .map_err(|e| e.to_string())?,
            Some(port) => {
                let socket = UdpSocket::bind((
                    settings
                        .osc_host
                        .parse::<std::net::IpAddr>()
                        .map_err(|_| "Invalid OSC bind IP")?,
                    port,
                ))
                .map_err(|e| format!("OSC port {port}: {e}"))?;
                socket.set_nonblocking(true).map_err(|e| e.to_string())?;
                Some(socket)
            }
            None => None,
        };
        if settings.midi && self.midi.is_none() {
            self.midi = Some(midi::MidiInputs::open(self.send.clone())?);
        }
        if !settings.midi {
            self.midi = None;
            for _ in self.receive.try_iter() {}
        }
        self.socket = socket;
        self.port = settings.osc_port;
        self.host = settings.osc_host.clone();
        Ok(format!(
            "MIDI: {} inputs · OSC: {}",
            self.midi_ports()
                .iter()
                .filter(|p| p.error.is_none())
                .count(),
            self.port
                .map_or_else(|| "off".into(), |port| format!("{}:{port}", self.host))
        ))
    }
    fn midi_ports(&self) -> &[MidiPort] {
        self.midi.as_ref().map_or(&[], |midi| midi.ports.as_slice())
    }
    fn refresh_midi(&mut self) -> PortResult<()> {
        if let Some(midi) = self.midi.as_mut() {
            midi.refresh()?;
        }
        Ok(())
    }
    fn poll(&mut self, events: &mut Vec<ControlEvent>) -> PortResult<()> {
        events.extend(self.receive.try_iter().take(1024));
        if let Some(socket) = &self.socket {
            let mut buffer = [0u8; 65536];
            for _ in 0..128 {
                match socket.recv_from(&mut buffer) {
                    Ok((size, _)) => {
                        if let Ok((_, packet)) = rosc::decoder::decode_udp(&buffer[..size]) {
                            decode_osc(packet, events, 0);
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(error) => return Err(error.to_string()),
                }
            }
        }
        Ok(())
    }
}
pub fn decode_midi(bytes: &[u8]) -> Option<ControlEvent> {
    if bytes.len() < 3 {
        return None;
    }
    let channel = bytes[0] & 0x0f;
    let message = match bytes[0] & 0xf0 {
        0xb0 => MidiMessage::Cc {
            controller: bytes[1].min(127),
            value: bytes[2].min(127),
        },
        0x90 => MidiMessage::Note {
            note: bytes[1].min(127),
            velocity: bytes[2].min(127),
        },
        0x80 => MidiMessage::Note {
            note: bytes[1].min(127),
            velocity: 0,
        },
        _ => return None,
    };
    Some(ControlEvent::Midi { channel, message })
}
fn decode_osc(packet: rosc::OscPacket, events: &mut Vec<ControlEvent>, depth: usize) {
    if depth > 8 || events.len() >= 4096 {
        return;
    }
    match packet {
        rosc::OscPacket::Message(message) => {
            let value = message.args.first().and_then(|arg| match arg {
                rosc::OscType::Float(v) => Some(*v),
                rosc::OscType::Double(v) => Some(*v as f32),
                rosc::OscType::Int(v) => Some(*v as f32),
                rosc::OscType::Long(v) => Some(*v as f32),
                rosc::OscType::Bool(v) => Some(f32::from(*v)),
                _ => None,
            });
            if let Some(value) = value.filter(|v| v.is_finite()) {
                events.push(ControlEvent::Osc {
                    address: message.addr,
                    value,
                });
            }
        }
        rosc::OscPacket::Bundle(bundle) => {
            for packet in bundle.content {
                decode_osc(packet, events, depth + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests;
