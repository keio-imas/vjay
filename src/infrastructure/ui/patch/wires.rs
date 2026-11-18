use super::*;
impl Editor {
    pub(super) fn patch_wires(&self, positions: &HashMap<NodeId, Position>) -> gpui::AnyElement {
        let cable = self.cable;
        let pan = self.pan;
        let z = self.zoom;
        let edges = self
            .session()
            .project()
            .graph
            .connections()
            .filter(|edge| cable.and_then(|c| c.previous) != Some(*edge))
            .filter_map(|edge| {
                Some((
                    *positions.get(&edge.from)?,
                    *positions.get(&edge.to)?,
                    edge.output,
                    edge.input,
                    self.session().port_signal(edge.from, edge.output)?,
                ))
            })
            .collect::<Vec<_>>();
        let anchor = cable.and_then(|c| {
            let (id, port, output) = match c.anchor {
                Socket::Output(id, p) => (id, p, true),
                Socket::Input(id, p) => (id, p, false),
            };
            let p = positions.get(&id)?;
            let record = self.session().project().graph.node(id).ok()?;
            let descriptor = self.session().schema(record.id);
            let signal = if output {
                descriptor.outputs.get(port)?.signal
            } else {
                descriptor.input(port)?.signal
            };
            Some((
                Position {
                    x: (p.x + if output { NODE_WIDTH } else { 0.0 }) * z + pan.x,
                    y: (p.y + port_y(port)) * z + pan.y,
                },
                output,
                c.pointer,
                signal,
            ))
        });
        let geometry = self.patch_bounds.clone();
        canvas(
            move |bounds, _, _| {
                geometry.set(Some(bounds));
            },
            move |bounds, _, window, _| {
                for (from, to, output, input, signal) in &edges {
                    let a = bounds.origin
                        + point(
                            px((from.x + NODE_WIDTH) * z + pan.x),
                            px((from.y + port_y(*output)) * z + pan.y),
                        );
                    let b = bounds.origin
                        + point(
                            px(to.x * z + pan.x),
                            px((to.y + port_y(*input)) * z + pan.y),
                        );
                    cables::paint_wire(window, a, b, false, z, *signal);
                }
                if let Some((anchor, output, pointer, signal)) = anchor {
                    let anchor = bounds.origin + point(px(anchor.x), px(anchor.y));
                    let pointer = point(px(pointer.x), px(pointer.y));
                    let (a, b) = if output {
                        (anchor, pointer)
                    } else {
                        (pointer, anchor)
                    };
                    cables::paint_wire(window, a, b, true, z, signal);
                }
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    }
}
