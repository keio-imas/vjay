use std::sync::Arc;
use vjay_application::{
    control_io::{ControlEvent, ControlIo, InputSettings, MidiPort},
    ports::PortResult,
    services::{ServiceControls, ServiceRequest},
};
use vjay_domain::graph::NodeId;
pub mod link;
pub mod nodes;
pub mod rekordbox;

pub trait NodeService {
    fn update(&mut self, values: &[f32]);
    fn poll(&mut self, node: NodeId, events: &mut Vec<ControlEvent>);
}
pub trait ServiceFactory: Send + Sync {
    fn id(&self) -> &'static str;
    fn create(&self, request: &ServiceRequest) -> PortResult<Box<dyn NodeService>>;
}
struct Active {
    request: ServiceRequest,
    service: Box<dyn NodeService>,
}
pub struct ServiceControlIo {
    input: Box<dyn ControlIo>,
    factories: Arc<Vec<Box<dyn ServiceFactory>>>,
    active: Vec<Active>,
}
impl ServiceControlIo {
    pub fn new(input: Box<dyn ControlIo>, factories: Arc<Vec<Box<dyn ServiceFactory>>>) -> Self {
        Self {
            input,
            factories,
            active: Vec::new(),
        }
    }
}
impl ControlIo for ServiceControlIo {
    fn configure(&mut self, s: &InputSettings) -> PortResult<String> {
        self.input.configure(s)
    }
    fn poll(&mut self, events: &mut Vec<ControlEvent>) -> PortResult<()> {
        for active in &mut self.active {
            active.service.poll(active.request.node, events);
        }
        self.input.poll(events)
    }
    fn midi_ports(&self) -> &[MidiPort] {
        self.input.midi_ports()
    }
    fn refresh_midi(&mut self) -> PortResult<()> {
        self.input.refresh_midi()
    }
    fn prepare_services(&mut self, requests: &[ServiceRequest]) -> PortResult<()> {
        let mut old = std::mem::take(&mut self.active);
        for request in requests {
            let service = if let Some(index) = old.iter().position(|a| a.request == *request) {
                old.swap_remove(index).service
            } else {
                self.factories
                    .iter()
                    .find(|f| f.id() == request.service)
                    .ok_or_else(|| format!("Service adapter unavailable: {}", request.service))?
                    .create(request)?
            };
            self.active.push(Active {
                request: request.clone(),
                service,
            });
        }
        Ok(())
    }
    fn update_services(&mut self, controls: &[ServiceControls]) {
        for (active, control) in self.active.iter_mut().zip(controls) {
            debug_assert_eq!(active.request.node, control.node);
            active.service.update(&control.values);
        }
    }
}
