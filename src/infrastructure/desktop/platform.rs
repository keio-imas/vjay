use crate::{gpu::presentation::GpuPresentation, storage::write_ppm, ui::Editor};
gpui::actions!(vjay, [Quit]);
use gpui::{App, AppContext, Application, Bounds, Window, WindowBounds, WindowOptions, px, size};
use std::{cell::RefCell, path::PathBuf, rc::Rc, time::Instant};
use vjay_application::ports::{Platform, PortResult, RuntimeFactory, SessionFactory};

pub type SharedPreview = Rc<RefCell<dyn GpuPresentation>>;
pub type DesktopSession = (
    vjay_application::session::Session,
    Option<SharedPreview>,
    Option<Rc<dyn OutputFactory>>,
);
pub struct DesktopRuntime {
    pub factory: Box<dyn RuntimeFactory>,
    pub preview: Option<SharedPreview>,
    pub outputs: Option<Rc<dyn OutputFactory>>,
}
pub trait OutputFactory {
    fn create(
        &self,
        window: &Window,
        source: vjay_domain::graph::NodeId,
    ) -> PortResult<SharedPreview>;
}
type DesktopRuntimeBuilder = Box<dyn FnOnce(&mut Window) -> PortResult<DesktopRuntime>>;

pub struct GpuiPlatform {
    runtime: DesktopRuntimeBuilder,
    output_on_start: bool,
}

impl GpuiPlatform {
    pub fn new(runtime: impl FnOnce(&mut Window) -> PortResult<DesktopRuntime> + 'static) -> Self {
        Self {
            runtime: Box::new(runtime),
            output_on_start: false,
        }
    }
    pub fn with_output_window(mut self, enabled: bool) -> Self {
        self.output_on_start = enabled;
        self
    }
}

impl Platform for GpuiPlatform {
    fn run(self: Box<Self>, create: SessionFactory) -> PortResult<()> {
        let result = Rc::new(RefCell::new(Ok(())));
        let errors = result.clone();
        Application::new().run(move |cx: &mut App| {
            cx.on_action(|_: &Quit, cx| cx.quit());
            cx.bind_keys([
                gpui::KeyBinding::new("cmd-q", Quit, None),
                gpui::KeyBinding::new("ctrl-q", Quit, None),
            ]);
            let bounds = Bounds::centered(None, size(px(1440.0), px(940.0)), cx);
            let opened = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(1000.0), px(700.0))),
                    titlebar: Some(gpui::TitlebarOptions {
                        title: Some("VJAY / Patch Studio".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                move |window, cx| {
                    let initialization = (self.runtime)(window).and_then(|runtime| {
                        create(runtime.factory.as_ref())
                            .map(|session| (session, runtime.preview, runtime.outputs))
                    });
                    cx.new(|cx| Editor::new(initialization, window, cx, self.output_on_start))
                },
            );
            if let Err(error) = opened {
                *errors.borrow_mut() = Err(error.to_string());
                cx.quit();
            }
            cx.on_window_closed(|cx| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();
            cx.activate(true);
        });
        result.borrow().clone()
    }
}

pub struct HeadlessPlatform {
    pub runtime: Box<dyn RuntimeFactory>,
    pub frames: u32,
    pub output: Option<PathBuf>,
}

impl Platform for HeadlessPlatform {
    fn run(self: Box<Self>, create: SessionFactory) -> PortResult<()> {
        if self.frames == 0 {
            return Err("At least one frame is required".into());
        }
        let mut session = create(self.runtime.as_ref())?;
        session.render_at(0.0)?;
        drop(session.capture()?); // Drain warm-up GPU work before starting the clock.
        let start = Instant::now();
        for i in 0..self.frames {
            session.render_at(i as f64 / 60.0)?;
        }
        let frame = session.capture()?;
        let elapsed = start.elapsed();
        if let Some(path) = self.output {
            write_ppm(&path, &frame)?;
            println!("Saved {}", path.display());
        }
        let checksum = frame.rgba.iter().fold(0_u64, |sum, &byte| {
            sum.wrapping_mul(31).wrapping_add(byte as u64)
        });
        let plan = session.plan().ok_or("Missing execution plan")?;
        println!(
            "{} | {}x{} | {} passes / {} textures | {} frames + readback in {:.2} ms | checksum {checksum:016x}",
            session.backend_description(),
            frame.extent.width,
            frame.extent.height,
            plan.steps.len(),
            plan.texture_count,
            self.frames,
            elapsed.as_secs_f64() * 1000.0
        );
        Ok(())
    }
}
