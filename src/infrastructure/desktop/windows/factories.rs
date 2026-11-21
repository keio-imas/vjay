use super::{WindowsGpuPreview, window::host_handle};
use crate::{
    gpu::{
        GpuContext,
        presentation::{GpuPresentation, NodePreviewFactory},
        presentation_group::PresentationGroup,
    },
    platform::{OutputFactory, SharedPreview},
};
use ::windows::Win32::Foundation::HWND;
use std::{cell::RefCell, rc::Rc};
use vjay_application::ports::PortResult;
use vjay_domain::graph::NodeId;

pub struct WindowsOutputFactory {
    context: GpuContext,
    group: Rc<RefCell<PresentationGroup>>,
}
impl WindowsOutputFactory {
    pub fn new(context: GpuContext, group: Rc<RefCell<PresentationGroup>>) -> Self {
        Self { context, group }
    }
}
impl OutputFactory for WindowsOutputFactory {
    fn create(&self, window: &gpui::Window, source: NodeId) -> PortResult<SharedPreview> {
        let mut output =
            WindowsGpuPreview::attach_with_instance(window, self.context.instance.clone())?;
        output.initialize(&self.context.device, &self.context.adapter)?;
        let output: SharedPreview = Rc::new(RefCell::new(output));
        self.group.borrow_mut().register(source, &output)?;
        Ok(output)
    }
}

pub struct WindowsNodePreviewFactory {
    host: HWND,
    context: GpuContext,
}
impl WindowsNodePreviewFactory {
    pub fn new(window: &gpui::Window, context: GpuContext) -> PortResult<Self> {
        Ok(Self {
            host: host_handle(window)?,
            context,
        })
    }
}
impl NodePreviewFactory for WindowsNodePreviewFactory {
    fn create(&self) -> PortResult<Box<dyn GpuPresentation>> {
        let mut preview = WindowsGpuPreview::attach_host(self.host, self.context.instance.clone())?;
        preview.initialize(&self.context.device, &self.context.adapter)?;
        Ok(Box::new(preview))
    }
}
