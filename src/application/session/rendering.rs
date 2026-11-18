use super::*;

impl Session {
    pub fn tick(&mut self) -> PortResult<()> {
        let now = self.clock.seconds();
        if !self.paused {
            self.seconds += (now - self.last_clock).clamp(0.0, 0.25);
        }
        self.last_clock = now;
        self.render_frame(self.seconds, now, crate::media::FrameDelivery::Latest)
    }
    pub fn render_at(&mut self, seconds: f64) -> PortResult<()> {
        self.render_frame(seconds, seconds, crate::media::FrameDelivery::Exact)
    }
    fn render_frame(
        &mut self,
        seconds: f64,
        control_seconds: f64,
        delivery: crate::media::FrameDelivery,
    ) -> PortResult<()> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err("Invalid playhead time".into());
        }
        self.seconds = seconds;
        if let Err(error) = self.control_io.poll(&mut self.signals.events) {
            self.input_status = error;
        }
        self.process_midi();
        if self.dirty {
            self.asset_sources.rebuild(
                &self.project.graph,
                self.nodes.as_ref(),
                &self.project.assets,
            )?;
        }
        while let Some((node, path)) = self.asset_sources.poll(delivery) {
            if self.project.graph.node(node).is_ok() {
                if self.asset_sources.file_output(node) {
                    self.signals
                        .events
                        .push(crate::control_io::ControlEvent::NodeFile {
                            node,
                            values: vec![Some(std::sync::Arc::from(path.as_path()))],
                        });
                    self.signals
                        .events
                        .push(crate::control_io::ControlEvent::NodeText {
                            node,
                            values: vec![std::sync::Arc::from(
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .as_ref(),
                            )],
                        });
                    let items = self.asset_sources.items(node);
                    let mut values = [0.0; 16];
                    values[0] = items.iter().position(|i| i.path == path).unwrap_or(0) as f32;
                    values[1] = items.len() as f32;
                    self.signals
                        .events
                        .push(crate::control_io::ControlEvent::NodeValues { node, values });
                } else if self.asset_sources.still_image(node) {
                    self.import_image(path, Some(node))?;
                } else {
                    self.import_video(path, Some(node))?;
                }
            }
        }
        if self.dirty {
            self.dirty = false;
            let result = self.synchronize();
            self.valid = result.is_ok();
            self.error = result.err();
        }
        // Control envelopes and input pulses keep running while media transport is paused.
        self.signals.tick(control_seconds);
        self.asset_sources.tick(&self.signals);
        for (node, path, still) in self.file_media.changes(&self.signals) {
            if let Some(path) = path {
                if still {
                    self.import_image(path, Some(node))?;
                } else {
                    self.import_video(path, Some(node))?;
                }
            } else if still
                && self
                    .project
                    .graph
                    .node(node)
                    .is_ok_and(|n| n.asset.is_some())
            {
                self.project
                    .graph
                    .clear_asset(node)
                    .map_err(|e| e.to_string())?;
                self.invalidate(true);
            }
        }
        self.services
            .update(&self.signals, self.control_io.as_mut());
        self.timeline.tick_ready(seconds, &self.signals, |node| {
            self.renderer.media_ready(node)
        });
        self.timeline.resolve_seeks(|node| {
            self.renderer
                .media_duration(node)
                .or_else(|| self.audio.media_duration(node))
        });
        self.audio_plan.update(&self.signals, &self.timeline);
        if self.audio_error.is_none() {
            self.audio_error = self.audio.update(&self.audio_plan, self.paused).err();
        }
        if !self.valid {
            return Err(self
                .asset_sources
                .activity()
                .map(str::to_owned)
                .or_else(|| self.error.clone())
                .unwrap_or_else(|| "Patch is incomplete".into()));
        }
        if let Some(plan) = self.plan.as_mut() {
            self.signals.modulate(plan);
            self.timeline.apply(plan);
        }
        let plan = self.plan.as_ref().ok_or("Missing execution plan")?;
        // Wrap at one day to retain useful f32 time precision in GPU uniforms.
        self.renderer.render(
            plan,
            FrameContext {
                seconds: (seconds % 86400.0) as f32,
                timeline_seconds: seconds,
                extent: self.extent,
                delivery,
            },
        )
    }
    pub(super) fn invalidate(&mut self, topology: bool) {
        self.dirty = true;
        self.modified.set(true);
        self.prepare |= topology;
    }
    fn synchronize(&mut self) -> PortResult<()> {
        // Signal offsets belong to one compiled graph. Never retain an old
        // render plan across a signal-bank rebuild, including failed rebuilds.
        let previous = self.plan.take();
        self.signals
            .rebuild(&self.project.graph, self.nodes.as_ref())?;
        self.asset_sources.bind(&self.signals);
        self.file_media
            .rebuild(&self.project.graph, &self.signals)?;
        self.timeline.rebuild(&self.project.graph, &self.signals)?;
        self.services.rebuild(
            &self.project.graph,
            self.nodes.as_ref(),
            &self.signals,
            self.control_io.as_mut(),
        )?;
        self.audio_plan = match crate::audio::AudioPlan::compile(&self.project.graph, &self.signals)
        {
            Ok(plan) => {
                self.audio_error = self.audio.prepare(&plan, &self.project.assets).err();
                plan
            }
            Err(error) => {
                let empty = crate::audio::AudioPlan::default();
                let _ = self.audio.prepare(&empty, &self.project.assets);
                self.audio_error = Some(error);
                empty
            }
        };
        let mut candidate = compile_with_previews(
            &self.project.graph,
            self.project.output,
            self.nodes.as_ref(),
        )?;
        self.signals.bind(&mut candidate);
        let resource_change = previous.as_ref().is_none_or(|old| {
            old.outputs != candidate.outputs
                || old.texture_count != candidate.texture_count
                || old.output_slot != candidate.output_slot
                || old.steps.len() != candidate.steps.len()
                || old.steps.iter().zip(&candidate.steps).any(|(a, b)| {
                    a.preview != b.preview
                        || a.still_image != b.still_image
                        || a.operation.parameters.len() != b.operation.parameters.len()
                        || a.node != b.node
                        || a.output != b.output
                        || a.inputs != b.inputs
                        || a.target != b.target
                        || a.asset != b.asset
                        || a.operation.prepared.is_some() != b.operation.prepared.is_some()
                        || a.operation.shader != b.operation.shader
                })
        });
        if self.prepare || resource_change {
            self.renderer.set_assets(&self.project.assets);
            self.renderer.prepare(&candidate, self.extent)?;
        }
        self.plan = Some(candidate);
        self.prepare = false;
        self.asset_sources.retain_assets(&self.project.assets);
        Ok(())
    }
}
