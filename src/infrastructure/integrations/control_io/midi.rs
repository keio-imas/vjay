use super::*;

pub(super) const VIRTUAL_SOURCE: &str = "vjay:virtual-input";
pub(super) struct MidiInputs {
    connections: Vec<midir::MidiInputConnection<()>>,
    #[cfg(unix)]
    virtual_input: Option<midir::MidiInputConnection<()>>,
    pub ports: Vec<MidiPort>,
    send: SyncSender<ControlEvent>,
}
impl MidiInputs {
    pub fn open(send: SyncSender<ControlEvent>) -> PortResult<Self> {
        let mut inputs = Self {
            connections: Vec::new(),
            #[cfg(unix)]
            virtual_input: None,
            ports: Vec::new(),
            send,
        };
        #[cfg(unix)]
        {
            use midir::os::unix::VirtualInput;
            let virtual_port = Self::input()?.create_virtual(
                "VJAY MIDI In",
                callback(inputs.send.clone(), VIRTUAL_SOURCE.into()),
                (),
            );
            let error = match virtual_port {
                Ok(port) => {
                    inputs.virtual_input = Some(port);
                    None
                }
                Err(error) => Some(error.to_string()),
            };
            inputs.ports.push(MidiPort {
                id: VIRTUAL_SOURCE.into(),
                name: "VJAY MIDI In · virtual".into(),
                virtual_input: true,
                error,
            });
        }
        inputs.refresh()?;
        Ok(inputs)
    }
    fn input() -> PortResult<midir::MidiInput> {
        let mut input = midir::MidiInput::new("VJAY MIDI input").map_err(|e| e.to_string())?;
        input.ignore(midir::Ignore::All);
        Ok(input)
    }
    pub fn refresh(&mut self) -> PortResult<()> {
        let input = Self::input()?;
        self.connections.clear();
        self.ports.retain(|port| port.id == VIRTUAL_SOURCE);
        for port in input.ports().into_iter().take(63) {
            let id = format!("midi:{}", port.id());
            let name = input
                .port_name(&port)
                .unwrap_or_else(|_| "Unnamed MIDI input".into());
            let error = match Self::input()?.connect(
                &port,
                "VJAY",
                callback(self.send.clone(), id.clone()),
                (),
            ) {
                Ok(connection) => {
                    self.connections.push(connection);
                    None
                }
                Err(error) => Some(error.to_string()),
            };
            // One inaccessible device must not prevent the other inputs from opening.
            self.ports.push(MidiPort {
                id,
                name,
                error,
                virtual_input: false,
            });
        }
        Ok(())
    }
}
fn callback(
    send: SyncSender<ControlEvent>,
    source: String,
) -> impl FnMut(u64, &[u8], &mut ()) + Send + 'static {
    move |_, bytes, _| {
        if let Some(event) = decode_midi(bytes)
            && let Some((_, channel, message)) = event.midi()
        {
            let _ = send.try_send(ControlEvent::MidiFrom {
                source: source.clone(),
                channel,
                message,
            });
        }
    }
}
