use super::{NodeService, ServiceFactory};
use rusty_link::{AblLink, SessionState};
use vjay_application::{control_io::ControlEvent, ports::PortResult, services::ServiceRequest};
use vjay_domain::graph::NodeId;
pub struct LinkFactory;
impl ServiceFactory for LinkFactory {
    fn id(&self) -> &'static str {
        "ableton-link"
    }
    fn create(&self, _: &ServiceRequest) -> PortResult<Box<dyn NodeService>> {
        Ok(Box::new(LinkService::new()))
    }
}
struct LinkService {
    link: AblLink,
    state: SessionState,
    controls: [f32; 5],
    last_beat: Option<i64>,
}
impl LinkService {
    fn new() -> Self {
        Self {
            link: AblLink::new(120.0),
            state: SessionState::new(),
            controls: [0.0, 4.0, 120.0, 0.0, -1.0],
            last_beat: None,
        }
    }
}
impl NodeService for LinkService {
    fn update(&mut self, values: &[f32]) {
        let Ok(values) = <&[f32; 5]>::try_from(values) else {
            return;
        };
        self.link.enable(values[0] >= 0.5);
        self.link.enable_start_stop_sync(values[3] >= 0.5);
        let now = self.link.clock_micros();
        self.link.capture_app_session_state(&mut self.state);
        let tempo_changed = values[2] != self.controls[2];
        let transport_changed = values[4] != self.controls[4] && values[4] >= 0.0;
        if tempo_changed {
            self.state.set_tempo(values[2] as f64, now);
        }
        if transport_changed {
            self.state.set_is_playing(values[4] >= 0.5, now);
        }
        if tempo_changed || transport_changed {
            self.link.commit_app_session_state(&self.state);
        }
        self.controls = *values;
    }
    fn poll(&mut self, node: NodeId, events: &mut Vec<ControlEvent>) {
        self.link.capture_app_session_state(&mut self.state);
        let now = self.link.clock_micros();
        let quantum = self.controls[1] as f64;
        let beat = self.state.beat_at_time(now, quantum);
        let mut values = [0.0; 16];
        values[0] = self.state.tempo() as f32;
        values[1] = beat as f32;
        values[2] = (self.state.phase_at_time(now, quantum) / quantum) as f32;
        values[3] = f32::from(
            self.controls[0] >= 0.5 && self.last_beat.is_some_and(|b| b != beat.floor() as i64),
        );
        values[4] = self.link.num_peers() as f32;
        values[5] = f32::from(self.state.is_playing());
        self.last_beat = Some(beat.floor() as i64);
        events.push(ControlEvent::NodeValues { node, values });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires local Link multicast networking"]
    fn two_link_peers_exchange_tempo_and_transport() {
        let peer = AblLink::new(120.0);
        peer.enable_start_stop_sync(true);
        peer.enable(true);
        let mut service = LinkService::new();
        service.update(&[1.0, 4.0, 120.0, 1.0, -1.0]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        while peer.num_peers() == 0 && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(peer.num_peers() > 0);
        let mut state = SessionState::new();
        peer.capture_app_session_state(&mut state);
        let now = peer.clock_micros();
        state.set_tempo(137.0, now);
        state.set_is_playing(true, now);
        peer.commit_app_session_state(&state);
        let mut events = Vec::new();
        loop {
            events.clear();
            service.poll(NodeId(7), &mut events);
            let ControlEvent::NodeValues { values, .. } = &events[0] else {
                panic!()
            };
            if (values[0] - 137.0).abs() < 0.01 && values[5] == 1.0 {
                assert!(values[4] >= 1.0);
                assert!((0.0..1.0).contains(&values[2]));
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "Link did not synchronize"
            );
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        // Updating an unrelated parameter must not replace the peer's tempo.
        service.update(&[1.0, 8.0, 120.0, 1.0, -1.0]);
        service.link.capture_app_session_state(&mut service.state);
        assert!((service.state.tempo() - 137.0).abs() < 0.01);
        service.update(&[1.0, 8.0, 142.0, 1.0, -1.0]);
        assert!((service.state.tempo() - 142.0).abs() < 0.01);
    }
}
