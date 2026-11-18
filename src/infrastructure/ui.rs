use crate::platform::SharedPreview;
use gpui::{
    Bounds, Context, Div, ElementId, Entity, ExternalPaths, FocusHandle, FontWeight, KeyDownEvent,
    KeyUpEvent, MouseButton, MouseDownEvent, MouseMoveEvent, PathBuilder, PathPromptOptions,
    Pixels, Point, ScrollWheelEvent, SharedString, Stateful, Task, TitlebarOptions, Window,
    WindowBounds, WindowHandle, WindowOptions, canvas, div, point, prelude::*, px, relative, rgb,
    size,
};
use vjay_application::project::Point as Position;
use vjay_application::workspace::WorkspaceLayout;
use vjay_application::{ports::PortResult, session::Session};
use vjay_domain::graph::NodeId;

mod binding_picker;
mod cables;
mod chrome;
mod clipboard;
mod collection;
mod control_inspector;
mod controls;
mod dropdowns;
mod font_picker;
mod input;
mod input_sources;
mod inspector;
#[cfg(test)]
mod interaction_tests;
mod keyboard;
mod library;
mod lifecycle;
mod midi;
mod node_previews;
mod output;
mod palette;
mod panels;
mod parameter_drag;
mod projects;
mod rendering;
mod selection;
mod status;
mod uniforms;
mod video_sync;
mod widgets;
use cables::{CableGesture, Socket};
use panels::Panel;
mod configuration;
mod insertion;
mod patch;
mod text_field;
mod zoom;

mod theme;
use theme::*;

#[derive(Clone, Copy)]
enum Drag {
    Nodes,
    Pan(Position, Position),
    Control(u64, Position, Position),
    Resize(u64, Position, Position),
    Value(parameter_drag::ValueDrag),
    XY(parameter_drag::XYDrag),
    Panel(Panel, WorkspaceLayout, Position),
}

pub struct Editor {
    session: Option<Session>,
    preview: Option<SharedPreview>,
    message: String,
    render_error: Option<String>,
    selection: selection::NodeSelection,
    marquee: Option<selection::Marquee>,
    node_move: Option<selection::NodeMove>,
    drag: Option<Drag>,
    perform: bool,
    pan: Position,
    zoom: f32,
    cable: Option<CableGesture>,
    outputs: Option<std::rc::Rc<dyn crate::platform::OutputFactory>>,
    output_windows: Vec<(NodeId, WindowHandle<output::OutputRoot>)>,
    playback: Option<Task<()>>,
    focus: FocusHandle,
    output_extent: std::rc::Rc<std::cell::Cell<vjay_application::ports::Extent>>,
    search: Option<Entity<text_field::TextField>>,
    category: Option<&'static str>,
    category_menu: bool,
    selected_control: Option<u64>,
    control_menu: bool,
    insert_menu: Option<insertion::InsertMenu>,
    patch_menu: Option<clipboard::PatchMenu>,
    configuration: Option<configuration::Configuration>,
    palette: Option<palette::Palette>,
    font_picker: Option<font_picker::FontPicker>,
    binding_picker: Option<binding_picker::BindingPicker>,
    parameter_menu: Option<dropdowns::ParameterMenu>,
    patch_bounds: std::rc::Rc<std::cell::Cell<Option<Bounds<Pixels>>>>,
}

impl Editor {
    fn apply<T>(&mut self, result: PortResult<T>) {
        if let Err(error) = result {
            self.message = error;
        }
    }
    fn session(&self) -> &Session {
        self.session
            .as_ref()
            .expect("only rendered after initialization")
    }
    fn session_mut(&mut self) -> &mut Session {
        self.session
            .as_mut()
            .expect("only edited after initialization")
    }
}

fn button(id: impl Into<ElementId>, text: impl Into<SharedString>) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_sm()
        .cursor_pointer()
        .hover(|style| style.bg(rgb(ELEVATED)))
        .child(text.into())
}

fn position(point: Point<Pixels>) -> Position {
    Position {
        x: point.x.into(),
        y: point.y.into(),
    }
}
