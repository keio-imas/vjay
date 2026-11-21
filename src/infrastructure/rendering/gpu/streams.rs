use crate::video_io::{VideoIoRegistry, VideoReceiver, VideoSender};
use std::{collections::HashMap, sync::Arc};
use vjay_application::{
    compiler::RenderPlan,
    ports::{Extent, PortResult},
};
use vjay_domain::{execution::StreamRequest, graph::NodeId};
struct Input {
    node: NodeId,
    status: Option<&'static str>,
    request: StreamRequest,
    receiver: Box<dyn VideoReceiver>,
}
struct Output {
    node: NodeId,
    slot: usize,
    request: StreamRequest,
    sender: Box<dyn VideoSender>,
}
#[derive(Default)]
pub(super) struct Streams {
    pub factory: Arc<VideoIoRegistry>,
    inputs: Vec<Input>,
    outputs: Vec<Output>,
    extent: Option<Extent>,
    pub errors: HashMap<NodeId, String>,
}
impl Streams {
    pub fn new(factory: Arc<VideoIoRegistry>) -> Self {
        Self {
            factory,
            ..Default::default()
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        plan: &RenderPlan,
        extent: Extent,
        views: &mut [Option<wgpu::Texture>],
    ) -> PortResult<()> {
        if self.extent != Some(extent) {
            self.inputs.clear();
            self.outputs.clear();
            self.extent = Some(extent);
        }
        self.errors.clear();
        let mut previous = std::mem::take(&mut self.inputs);
        let mut outputs = std::mem::take(&mut self.outputs);
        outputs.retain(|s| {
            plan.outputs
                .iter()
                .any(|o| o.node == s.node && o.stream.as_ref() == Some(&s.request))
        });
        let mut ndi_bytes = 0u64;
        for (step, node) in plan.steps.iter().enumerate() {
            let Some(request) = &node.operation.stream else {
                continue;
            };
            if self.inputs.len() >= 8 {
                let error = "At most eight shared video inputs are supported".to_string();
                if node.program_required {
                    return Err(error);
                }
                self.errors.insert(node.node, error);
                continue;
            }
            let result = if let Some(i) = previous
                .iter()
                .position(|s| s.node == node.node && s.request == *request)
            {
                Ok(previous.swap_remove(i).receiver)
            } else {
                self.factory
                    .factory(request.provider)
                    .and_then(|f| f.receiver(request, device, queue, extent))
            };
            match result {
                Ok(receiver) => {
                    views[step] = Some(receiver.texture().clone());
                    self.inputs.push(Input {
                        node: node.node,
                        status: None,
                        request: request.clone(),
                        receiver,
                    });
                }
                Err(error) => {
                    if node.program_required {
                        return Err(error);
                    }
                    self.errors.insert(node.node, error);
                }
            }
        }
        for output in &plan.outputs {
            let Some(request) = &output.stream else {
                continue;
            };
            if request.options.get("enabled").is_some_and(|v| v != "On") {
                continue;
            }
            if request.provider == "ndi" {
                // Two GPU staging frames and three recycled CPU frames per sender.
                let row = u64::from(extent.width) * 4;
                let bytes = (2 * row.div_ceil(256) * 256 + 3 * row) * u64::from(extent.height);
                if ndi_bytes + bytes > 512 * 1024 * 1024 {
                    self.errors.insert(
                        output.node,
                        "NDI OUT transfer buffers exceed the 512 MiB budget".into(),
                    );
                    continue;
                }
                ndi_bytes += bytes;
            }
            let result = if let Some(i) = outputs
                .iter()
                .position(|s| s.node == output.node && s.request == *request)
            {
                Ok(outputs.swap_remove(i).sender)
            } else {
                self.factory
                    .factory(request.provider)
                    .and_then(|f| f.sender(request, device, queue, extent))
            };
            let sender = match result {
                Ok(sender) => sender,
                Err(error) => {
                    self.errors.insert(output.node, error);
                    continue;
                }
            };
            self.outputs.push(Output {
                node: output.node,
                slot: output.slot,
                request: request.clone(),
                sender,
            });
        }
        Ok(())
    }
    pub fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
    ) -> PortResult<()> {
        for input in &mut self.inputs {
            if let Err(error) = input.receiver.update(device, queue, encoder) {
                self.errors.insert(input.node, error);
                continue;
            }
            let status = input.receiver.status();
            if input.status != status {
                input.status = status;
                if let Some(status) = status {
                    self.errors.insert(input.node, status.into());
                } else {
                    self.errors.remove(&input.node);
                }
            }
        }
        Ok(())
    }
    pub fn encode(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        textures: &[wgpu::Texture],
    ) -> PortResult<()> {
        self.outputs
            .retain_mut(|o| match o.sender.encode(encoder, &textures[o.slot]) {
                Ok(()) => true,
                Err(e) => {
                    self.errors.insert(o.node, e);
                    false
                }
            });
        Ok(())
    }
    pub fn submitted(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) -> PortResult<()> {
        self.outputs
            .retain_mut(|o| match o.sender.submitted(device, queue) {
                Ok(()) => true,
                Err(e) => {
                    self.errors.insert(o.node, e);
                    false
                }
            });
        Ok(())
    }
}
