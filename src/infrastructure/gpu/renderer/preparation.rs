use super::*;
impl WgpuRenderer {
    pub(super) fn prepare_plan(&mut self, plan: &RenderPlan, extent: Extent) -> PortResult<()> {
        Extent::new(extent.width, extent.height)?;
        let video_count = plan
            .steps
            .iter()
            .filter(|step| step.operation.media.is_some())
            .count();
        let source_count = video_count.min(8)
            + 2 * plan
                .steps
                .iter()
                .filter(|s| s.operation.stream.is_some())
                .count()
            + plan
                .steps
                .iter()
                .filter(|s| s.still_image || s.operation.prepared.is_some())
                .count();
        let required_videos = plan
            .steps
            .iter()
            .filter(|s| s.program_required && s.operation.media.is_some())
            .count();
        let bytes = extent.width as u64
            * extent.height as u64
            * 4
            * (plan.texture_count + source_count) as u64;
        if required_videos > 8 {
            return Err("At most eight simultaneous video sources are supported".into());
        }
        if bytes > 512 * 1024 * 1024 {
            return Err(
                "Patch exceeds the 512 MiB texture budget; lower the output resolution".into(),
            );
        }
        let memory_scope = self.device.push_error_scope(wgpu::ErrorFilter::OutOfMemory);
        let pipelines = self
            .custom
            .prepare(&self.device, &self.layout, plan, &self.programs)?;
        let validation_scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut video = VideoInputs::prepare(
            &self.device,
            &self.queue,
            plan,
            extent,
            &self.assets,
            self.media_factory.as_ref(),
            self.video.take(),
        )?;
        self.stills.prepare(
            (&self.device, &self.queue),
            plan,
            extent,
            &self.assets,
            self.image_loaders.files.as_deref(),
            &mut video.views,
        )?;
        self.prepared_images.prepare(
            (&self.device, &self.queue),
            plan,
            extent,
            self.image_loaders.generated.as_deref(),
            &mut video.views,
        )?;
        self.streams
            .prepare(&self.device, &self.queue, plan, extent, &mut video.views)?;
        let candidate = Resources::new(&self.device, &self.layout, plan, extent, &video.views);
        let validation = pollster::block_on(validation_scope.pop());
        let memory = pollster::block_on(memory_scope.pop());
        if let Some(error) = validation.or(memory) {
            return Err(error.to_string());
        }
        if let Some(p) = &self.presentation {
            p.borrow_mut()
                .prepare(&self.device, &candidate.views[plan.output_slot])?;
        }
        if let Some(p) = &self.presentation {
            let nodes = plan
                .previews
                .iter()
                .map(|preview| {
                    (
                        preview.node,
                        candidate.views[plan.steps[preview.step].target].clone(),
                    )
                })
                .collect::<Vec<_>>();
            p.borrow_mut().prepare_nodes(&self.device, &nodes)?;
            let outputs = plan
                .outputs
                .iter()
                .filter(|o| o.stream.is_none())
                .map(|o| (o.node, candidate.views[o.slot].clone()))
                .collect::<Vec<_>>();
            p.borrow_mut().prepare_outputs(&self.device, &outputs)?;
        }
        self.output_slots = plan.outputs.iter().map(|o| (o.node, o.slot)).collect();
        self.resources = Some(candidate);
        self.video = Some(video);
        self.pipelines = pipelines;
        Ok(())
    }
}
