use super::*;
#[cfg(unix)]
static MIDI_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
#[test]
fn midi_parsing_handles_channels_note_off_and_truncated_packets() {
    assert!(matches!(
        decode_midi(&[0xb2, 74, 127]),
        Some(ControlEvent::Midi {
            channel: 2,
            message: MidiMessage::Cc {
                controller: 74,
                value: 127
            }
        })
    ));
    assert!(matches!(
        decode_midi(&[0x83, 60, 64]),
        Some(ControlEvent::Midi {
            channel: 3,
            message: MidiMessage::Note {
                note: 60,
                velocity: 0
            }
        })
    ));
    assert!(decode_midi(&[0x90, 60]).is_none());
    assert!(decode_midi(&[0xf8]).is_none());
}
#[test]
fn osc_udp_receives_values_and_rejects_nonfinite_arguments() {
    let mut io = NativeControlIo::default();
    io.configure(&InputSettings {
        osc_port: Some(0),
        osc_host: "127.0.0.1".into(),
        ..Default::default()
    })
    .unwrap();
    let address = io.socket.as_ref().unwrap().local_addr().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    for (osc_address, value) in [("/fader", 0.75), ("/fader", f32::NAN), ("/done", 1.0)] {
        let bytes = rosc::encoder::encode(&rosc::OscPacket::Message(rosc::OscMessage {
            addr: osc_address.into(),
            args: vec![rosc::OscType::Float(value)],
        }))
        .unwrap();
        sender.send_to(&bytes, address).unwrap();
    }
    let mut events = Vec::new();
    let start = std::time::Instant::now();
    loop {
        io.poll(&mut events).unwrap();
        if events
            .iter()
            .any(|e| matches!(e, ControlEvent::Osc {address,..} if address=="/done"))
            || start.elapsed() >= std::time::Duration::from_secs(1)
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(events.len(), 2);
    assert!(
        matches!(&events[0],ControlEvent::Osc{address,value} if address=="/fader"&&*value==0.75)
    );
    io.configure(&InputSettings::default()).unwrap();
    assert!(io.socket.is_none());
}

#[cfg(unix)]
#[test]
#[ignore = "requires a native MIDI service; uses only a virtual MIDI source"]
fn native_midi_receives_a_virtual_source() {
    let _guard = MIDI_TEST_LOCK.lock().unwrap();
    use midir::os::unix::VirtualOutput;
    let mut source = midir::MidiOutput::new("VJAY test source")
        .unwrap()
        .create_virtual("VJAY isolated test")
        .unwrap();
    let mut io = NativeControlIo::default();
    io.configure(&InputSettings {
        midi: true,
        ..Default::default()
    })
    .unwrap();
    source.send(&[0xb2, 74, 99]).unwrap();
    let start = std::time::Instant::now();
    let mut events = Vec::new();
    while events.is_empty() && start.elapsed() < std::time::Duration::from_secs(2) {
        std::thread::sleep(std::time::Duration::from_millis(5));
        io.poll(&mut events).unwrap();
    }
    assert!(events.iter().any(|e| matches!(
        e.midi(),
        Some((
            _,
            2,
            MidiMessage::Cc {
                controller: 74,
                value: 99
            }
        ))
    )));
}

#[cfg(unix)]
mod session;
