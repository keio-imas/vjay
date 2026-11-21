use super::*;

impl Editor {
    pub fn new(
        initialization: PortResult<crate::platform::DesktopSession>,
        window: &mut Window,
        cx: &mut Context<Self>,
        output_on_start: bool,
    ) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let mut editor = match initialization {
            Ok((session, preview, outputs)) => {
                let selected = session
                    .project()
                    .graph
                    .connections()
                    .find(|edge| Some(edge.to) == session.project().output)
                    .map(|edge| edge.from);
                let extent = session.extent();
                Self {
                    zoom: session.project().workspace.patch_zoom,
                    session: Some(session),
                    preview,
                    selection: {
                        let mut selection = selection::NodeSelection::default();
                        if let Some(id) = selected {
                            selection.only(id);
                        }
                        selection
                    },
                    outputs,
                    output_extent: std::rc::Rc::new(std::cell::Cell::new(extent)),
                    ..Self::empty(focus)
                }
            }
            Err(message) => Self {
                message,
                ..Self::empty(focus)
            },
        };
        let search = cx.new(|cx| text_field::TextField::new(String::new(), false, cx));
        cx.observe(&search, |_, _, cx| cx.notify()).detach();
        editor.search = Some(search);
        editor.playback = Some(cx.spawn(async move |entity, cx| {
            let origin = std::time::Instant::now();
            let mut pacing = vjay_application::engine::frame_pacing::FramePacer::default();
            let mut next_ui = 0.0;
            loop {
                let interval = match entity.update(cx, |this, _| {
                    this.session
                        .as_ref()
                        .map_or(std::time::Duration::from_millis(16), |s| s.frame_interval())
                }) {
                    Ok(interval) => interval,
                    Err(_) => break,
                };
                let delay = pacing.delay(origin.elapsed().as_secs_f64(), interval);
                cx.background_executor().timer(delay).await;
                if entity
                    .update(cx, |this, cx| {
                        let now = origin.elapsed().as_secs_f64();
                        pacing.started(now);
                        if let Some(session) = this.session.as_mut() {
                            this.render_error = session.tick().err();
                            this.output_extent.set(session.extent());
                        }
                        // Native output keeps its configured rate. Input handlers
                        // still request immediate updates independently of meters.
                        if now >= next_ui {
                            cx.notify();
                            next_ui = now + (1.0 / 30.0);
                        }
                        pacing.finished(origin.elapsed().as_secs_f64());
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        if output_on_start {
            cx.defer_in(window, |this, _, cx| this.open_output(cx));
        }
        editor
    }
    fn empty(focus: FocusHandle) -> Self {
        Self {
            settings: None,
            session: None,
            preview: None,
            message: String::new(),
            render_error: None,
            selection: Default::default(),
            marquee: None,
            node_move: None,
            drag: None,
            perform: false,
            zoom: 1.0,
            pan: Position { x: 0.0, y: 0.0 },
            cable: None,
            outputs: None,
            output_windows: Vec::new(),
            playback: None,
            focus,
            output_extent: std::rc::Rc::new(std::cell::Cell::new(
                vjay_application::ports::Extent {
                    width: 960,
                    height: 540,
                },
            )),
            search: None,
            category: None,
            category_menu: false,
            selected_control: None,
            control_menu: false,
            insert_menu: None,
            patch_menu: None,
            configuration: None,
            palette: None,
            parameter_menu: None,
            binding_picker: None,
            font_picker: None,
            patch_bounds: Default::default(),
        }
    }
}
