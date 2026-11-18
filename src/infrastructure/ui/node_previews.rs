use super::*;
use crate::gpu::presentation::{PreviewBounds, PreviewRegion};
mod regions;
use vjay_domain::graph::NodeRecord;
impl Editor {
    pub(super) fn node_preview(
        &self,
        node: &NodeRecord,
        stack: std::rc::Rc<Vec<PreviewRegion>>,
        index: usize,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        let id = node.id;
        let pan = self.pan;
        let z = self.zoom;
        let preview = self.preview.clone();
        let clip = self.patch_bounds.clone();
        let extent = self.session().extent();
        let error = self
            .session()
            .node_error(id)
            .or_else(|| {
                self.session()
                    .asset_source_status(id)
                    .filter(|status| *status != "Ready")
            })
            .map(|e| e.chars().take(300).collect::<String>());
        let failed = error.is_some();
        div()
            .id(SharedString::from(format!("node-preview-{}", id.0)))
            .debug_selector(move || format!("node-preview-{}", id.0))
            .absolute()
            .left(px(8.0 * z))
            .right(px(8.0 * z))
            .bottom(px(7.0 * z))
            .h(px(103.0 * z))
            .bg(rgb(BACKGROUND))
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        if let Some(preview) = &preview {
                            let width = f32::from(bounds.size.width).min(
                                f32::from(bounds.size.height) * extent.width as f32
                                    / extent.height as f32,
                            );
                            let height = width * extent.height as f32 / extent.width as f32;
                            let bounds = Bounds::new(
                                bounds.origin
                                    + point(
                                        px((f32::from(bounds.size.width) - width) * 0.5),
                                        px((f32::from(bounds.size.height) - height) * 0.5),
                                    ),
                                size(px(width), px(height)),
                            );
                            let bounds_data = PreviewBounds {
                                x: bounds.origin.x.into(),
                                y: bounds.origin.y.into(),
                                width: bounds.size.width.into(),
                                height: bounds.size.height.into(),
                                window_height: window.viewport_size().height.into(),
                                scale: window.scale_factor(),
                            };
                            let visible = if failed {
                                Vec::new()
                            } else if let Some(clip) = clip.get() {
                                let viewport = PreviewRegion {
                                    x: clip.left().into(),
                                    y: clip.top().into(),
                                    width: clip.size.width.into(),
                                    height: clip.size.height.into(),
                                };
                                let rect = PreviewRegion {
                                    x: bounds_data.x,
                                    y: bounds_data.y,
                                    width: bounds_data.width,
                                    height: bounds_data.height,
                                };
                                regions::visible(
                                    rect,
                                    viewport,
                                    stack[index + 1..].iter().map(|r| PreviewRegion {
                                        x: r.x * z + f32::from(clip.left()) + pan.x - 1.0,
                                        y: r.y * z + f32::from(clip.top()) + pan.y - 1.0,
                                        width: r.width * z + 2.0,
                                        height: r.height * z + 2.0,
                                    }),
                                )
                            } else {
                                Vec::new()
                            };
                            preview
                                .borrow_mut()
                                .set_node_regions(id, bounds_data, &visible);
                        }
                    },
                )
                .size_full(),
            )
            .when_some(error, |card, error| {
                card.child(
                    div()
                        .absolute()
                        .inset_0()
                        .p_2()
                        .overflow_hidden()
                        .text_xs()
                        .text_color(rgb(0xe79791))
                        .child(error),
                )
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if event.modifiers.alt {
                        this.begin_node_move(id, event, window, cx);
                    } else {
                        this.select_node(id, event, window, cx);
                    }
                }),
            )
            .into_any_element()
    }
    pub(super) fn toggle_node_preview(&mut self, id: NodeId, cx: &mut Context<Self>) {
        let Some(node) = self.session().project().graph.node(id).ok() else {
            return;
        };
        let mut settings = node.settings.clone();
        settings.preview = !settings.preview;
        let result = self.session_mut().set_settings(id, settings);
        self.apply(result);
        cx.notify();
    }
    pub(super) fn node_height(&self, node: &NodeRecord) -> f32 {
        let schema = self.session().schema(node.id);
        let base = patch::node_height(schema.input_count(), schema.outputs.len());
        if schema.capabilities.annotation {
            base.max(110.0)
        } else if node.settings.preview
            && schema
                .outputs
                .iter()
                .any(|p| p.signal == vjay_domain::port::SignalType::Texture)
        {
            base + 117.0
        } else {
            base
        }
    }
}
