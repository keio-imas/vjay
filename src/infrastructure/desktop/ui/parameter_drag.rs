use super::*;
use gpui::Modifiers;

#[derive(Clone, Copy)]
struct Axis {
    raw: f32,
    range: [f32; 2],
    shown: f32,
    snapped: bool,
}
impl Axis {
    fn new(raw: f32, range: [f32; 2]) -> Self {
        Self {
            raw,
            range,
            shown: raw,
            snapped: false,
        }
    }
    fn adjust(&mut self, delta: f32, modifiers: Modifiers) -> f32 {
        if self.snapped && !(modifiers.control || modifiers.platform) {
            self.raw = self.shown;
        }
        let sensitivity = if modifiers.shift { 0.1 } else { 1.0 };
        self.raw = (self.raw + delta * sensitivity * (self.range[1] - self.range[0]))
            .clamp(self.range[0], self.range[1]);
        self.value(modifiers)
    }
    fn value(&mut self, modifiers: Modifiers) -> f32 {
        self.snapped = modifiers.control || modifiers.platform;
        self.shown = if self.snapped {
            let target = (self.range[1] - self.range[0]) / 20.0;
            let power = 10.0_f32.powf(target.log10().floor());
            let normalized = target / power;
            let step = power
                * if normalized < 1.5 {
                    1.0
                } else if normalized < 3.5 {
                    2.0
                } else {
                    5.0
                };
            ((self.raw / step).round() * step).clamp(self.range[0], self.range[1])
        } else {
            self.raw
        };
        self.shown
    }
}

#[derive(Clone, Copy)]
pub(super) struct ValueDrag {
    pub node: NodeId,
    pub parameter: usize,
    axis: Axis,
    pointer: Position,
    vertical: bool,
}
impl ValueDrag {
    fn at(mut self, pointer: Position) -> Self {
        self.pointer = pointer;
        self
    }
    pub fn new(
        node: NodeId,
        parameter: usize,
        value: f32,
        pointer: Position,
        range: [f32; 2],
        vertical: bool,
    ) -> Self {
        Self {
            node,
            parameter,
            axis: Axis::new(value, range),
            pointer,
            vertical,
        }
    }
    pub fn update(&mut self, pointer: Position, modifiers: Modifiers) -> f32 {
        let delta = if self.vertical {
            self.pointer.y - pointer.y
        } else {
            pointer.x - self.pointer.x
        };
        self.pointer = pointer;
        self.axis.adjust(delta / 200.0, modifiers)
    }
}

impl Editor {
    pub(super) fn number_drag(
        &self,
        field: Stateful<Div>,
        drag: ValueDrag,
        control: Option<u64>,
        cx: &Context<Self>,
    ) -> Stateful<Div> {
        field
            .cursor_ns_resize()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    if let Some(id) = control {
                        this.focus_control(id, window, cx);
                    } else {
                        window.focus(&this.focus, cx);
                    }
                    this.drag = Some(Drag::Value(drag.at(position(event.position))));
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .on_click(
                cx.listener(move |this, event: &gpui::ClickEvent, window, cx| {
                    if let gpui::ClickEvent::Mouse(mouse) = event {
                        let modifiers = mouse.down.modifiers;
                        let delta = mouse.up.position - mouse.down.position;
                        if modifiers.shift
                            || modifiers.control
                            || modifiers.platform
                            || f32::from(delta.x).abs() + f32::from(delta.y).abs() > 3.0
                        {
                            return;
                        }
                    }
                    this.open_configuration(
                        configuration::Target::Number(drag.node, drag.parameter),
                        window,
                        cx,
                    );
                }),
            )
    }
}

#[derive(Clone, Copy)]
pub(super) struct XYDrag {
    pub node: NodeId,
    pub parameter: usize,
    axes: [Axis; 2],
    pointer: Position,
    bounds: Bounds<Pixels>,
}
impl XYDrag {
    pub fn new(
        node: NodeId,
        parameter: usize,
        pointer: Position,
        values: [f32; 2],
        ranges: [[f32; 2]; 2],
        bounds: Bounds<Pixels>,
    ) -> Self {
        Self {
            node,
            parameter,
            pointer,
            axes: std::array::from_fn(|i| Axis::new(values[i], ranges[i])),
            bounds,
        }
    }
    pub fn begin(&mut self, modifiers: Modifiers) -> [f32; 2] {
        if !modifiers.shift {
            let ratios = [
                (self.pointer.x - f32::from(self.bounds.left()))
                    / f32::from(self.bounds.size.width).max(1.0),
                1.0 - (self.pointer.y - f32::from(self.bounds.top()))
                    / f32::from(self.bounds.size.height).max(1.0),
            ];
            for (axis, ratio) in self.axes.iter_mut().zip(ratios) {
                axis.raw = axis.range[0] + ratio.clamp(0.0, 1.0) * (axis.range[1] - axis.range[0]);
            }
        }
        std::array::from_fn(|i| self.axes[i].value(modifiers))
    }
    pub fn update(&mut self, pointer: Position, modifiers: Modifiers) -> [f32; 2] {
        let delta = [
            (pointer.x - self.pointer.x) / f32::from(self.bounds.size.width).max(1.0),
            (self.pointer.y - pointer.y) / f32::from(self.bounds.size.height).max(1.0),
        ];
        self.pointer = pointer;
        std::array::from_fn(|i| self.axes[i].adjust(delta[i], modifiers))
    }
}
