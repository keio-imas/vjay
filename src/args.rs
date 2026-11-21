use std::path::PathBuf;
use vjay_application::ports::{Extent, PortResult};

pub struct Args {
    pub headless: bool,
    pub frames: u32,
    pub extent: Extent,
    pub size_explicit: bool,
    pub output: Option<PathBuf>,
    pub project: PathBuf,
    pub video: Option<PathBuf>,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub output_window: bool,
    pub ytdlp: PathBuf,
}

impl Args {
    pub fn parse() -> PortResult<Option<Self>> {
        let mut args = Self {
            headless: false,
            frames: 3,
            size_explicit: false,
            extent: Extent::new(960, 540)?,
            output: None,
            project: "vjay-project.json".into(),
            video: None,
            ffmpeg: "ffmpeg".into(),
            ffprobe: "ffprobe".into(),
            output_window: false,
            ytdlp: {
                let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(".venv-web/bin/yt-dlp");
                if local.is_file() {
                    local
                } else {
                    "yt-dlp".into()
                }
            },
        };
        let mut input = std::env::args().skip(1);
        while let Some(flag) = input.next() {
            match flag.as_str() {
                "--help" | "-h" => {
                    println!(
                        "VJAY v{} / GPUI + WebGPU\n\n  cargo run --release\n  cargo run --release -- --headless --frames 120 --size 1280x720 --output /tmp/vjay.ppm\n\nOptions:\n  --video PATH     Import a video as the program source\n  --output-window  Open the independent output window at startup\n  --yt-dlp PATH    URL video importer (default: project venv or PATH)\n  --ffmpeg PATH    FFmpeg executable (default: ffmpeg)\n  --ffprobe PATH   ffprobe executable (default: ffprobe)\n  --headless       Render without opening a window\n  --frames N       Number of headless frames (1..3600, default 3)\n  --size WxH       Render resolution (1..4096 each, default: saved video settings)\n  --output PATH    Save the final headless frame as PPM\n  --project PATH   Project to load if present; GUI Save/Load destination\n  -V, --version    Show version\n  -h, --help       Show help",
                        env!("CARGO_PKG_VERSION")
                    );
                    return Ok(None);
                }
                "--headless" => args.headless = true,
                "--output-window" => args.output_window = true,
                "--version" | "-V" => {
                    println!("VJAY v{}", env!("CARGO_PKG_VERSION"));
                    return Ok(None);
                }
                "--video" => args.video = Some(input.next().ok_or("Missing video path")?.into()),
                "--yt-dlp" => args.ytdlp = input.next().ok_or("Missing yt-dlp path")?.into(),
                "--ffmpeg" => args.ffmpeg = input.next().ok_or("Missing FFmpeg path")?.into(),
                "--ffprobe" => args.ffprobe = input.next().ok_or("Missing ffprobe path")?.into(),
                "--frames" => {
                    args.frames = input
                        .next()
                        .ok_or("Missing frame count")?
                        .parse()
                        .map_err(|_| "Invalid frame count")?
                }
                "--output" => args.output = Some(input.next().ok_or("Missing output path")?.into()),
                "--project" => args.project = input.next().ok_or("Missing project path")?.into(),
                "--size" => {
                    args.size_explicit = true;
                    let size = input.next().ok_or("Missing render size")?;
                    let (width, height) = size.split_once('x').ok_or("Use --size WIDTHxHEIGHT")?;
                    args.extent = Extent::new(
                        width.parse().map_err(|_| "Invalid width")?,
                        height.parse().map_err(|_| "Invalid height")?,
                    )?;
                }
                _ => return Err(format!("Unknown option {flag}; use --help")),
            }
        }
        if !(1..=3600).contains(&args.frames) {
            return Err("Frames must be between 1 and 3600".into());
        }
        if !args.headless && args.output.is_some() {
            return Err("--output requires --headless".into());
        }
        Ok(Some(args))
    }
}
