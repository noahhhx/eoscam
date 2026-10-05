mod camera;
mod frame;
mod loopback;

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use gphoto2::{Camera, Context};

use frame::Converter;
use loopback::Loopback;

const PROBE_INTERVAL: Duration = Duration::from_secs(2);
const IDLE_FRAME_INTERVAL: Duration = Duration::from_millis(200);

const USAGE: &str = "\
Usage: eoscam [OPTIONS]

Streams a Canon EOS camera's liveview to a v4l2loopback device. Writes black
frames until the camera connects, and reconnects when the camera comes back.

Options:
  --name NAME     v4l2loopback card_label to write to [default: EOS Webcam]
  --device PATH   write to this device instead of looking it up by name
  --size WxH      output resolution [default: 1056x704, the 550D liveview size]
  -h, --help      show this help
";

struct Args {
    name: String,
    device: Option<PathBuf>,
    width: u32,
    height: u32,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        name: "EOS Webcam".into(),
        device: None,
        width: 1056,
        height: 704,
    };
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().ok_or(format!("{arg} needs a value"));
        match arg.as_str() {
            "--name" => args.name = value()?,
            "--device" => args.device = Some(value()?.into()),
            "--size" => {
                let size = value()?;
                let (w, h) = size
                    .split_once('x')
                    .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
                    .filter(|&(w, h): &(u32, u32)| w > 0 && h > 0 && w % 2 == 0)
                    .ok_or(format!(
                        "invalid --size {size}, expected e.g. 1056x704 (even width)"
                    ))?;
                (args.width, args.height) = (w, h);
            }
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument {arg}\n\n{USAGE}")),
        }
    }
    Ok(args)
}

fn open_loopback(args: &Args) -> Result<Loopback, String> {
    let path = match &args.device {
        Some(path) => path.clone(),
        None => loopback::find_by_name(&args.name).ok_or(format!(
            "no v4l2loopback device named '{}' (is v4l2loopback loaded with card_label=\"{}\"?)",
            args.name, args.name
        ))?,
    };
    Loopback::open(path.clone(), args.width, args.height)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))
}

fn capture_liveview_jpeg(camera: &Camera, context: &Context) -> gphoto2::Result<Box<[u8]>> {
    camera.capture_preview().wait()?.get_data(context).wait()
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };

    let stop = Arc::new(AtomicBool::new(false));
    for signal in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        signal_hook::flag::register(signal, Arc::clone(&stop)).expect("register signal handler");
    }

    let context = match Context::new() {
        Ok(context) => context,
        Err(e) => {
            eprintln!("cannot create libgphoto2 context: {e}");
            return ExitCode::FAILURE;
        }
    };

    let mut converter = Converter::new(args.width, args.height);
    let mut sink: Option<Loopback> = None;
    let mut camera: Option<Camera> = None;
    let mut next_probe = Instant::now();
    let mut last_logged = String::new();

    eprintln!("eoscam waiting for a Canon camera");
    while !stop.load(Ordering::Relaxed) {
        let Some(out) = sink.as_mut() else {
            match open_loopback(&args) {
                Ok(lb) => {
                    eprintln!("writing to {}", lb.path.display());
                    sink = Some(lb);
                }
                Err(e) => {
                    log_unless_repeated(&mut last_logged, e);
                    thread::sleep(PROBE_INTERVAL);
                }
            }
            continue;
        };

        let frame = match &camera {
            Some(cam) => match capture_liveview_jpeg(cam, &context) {
                Ok(jpeg) => match converter.convert(&jpeg) {
                    Ok(frame) => frame,
                    Err(e) => {
                        log_unless_repeated(
                            &mut last_logged,
                            format!("skipping undecodable frame: {e:?}"),
                        );
                        continue;
                    }
                },
                Err(e) => {
                    eprintln!("camera disconnected ({e}), waiting for it to come back");
                    camera = None;
                    next_probe = Instant::now() + PROBE_INTERVAL;
                    continue;
                }
            },
            None => {
                if Instant::now() >= next_probe {
                    next_probe = Instant::now() + PROBE_INTERVAL;
                    match camera::open(&context) {
                        Ok(Some(cam)) => {
                            eprintln!("camera connected, streaming");
                            last_logged.clear();
                            camera = Some(cam);
                            continue;
                        }
                        Ok(None) => {}
                        Err(e) => log_unless_repeated(
                            &mut last_logged,
                            format!("found camera but could not open it: {e}"),
                        ),
                    }
                }
                thread::sleep(IDLE_FRAME_INTERVAL);
                converter.black()
            }
        };

        if let Err(e) = out.write_frame(frame) {
            log_unless_repeated(
                &mut last_logged,
                format!("write to {} failed: {e}", out.path.display()),
            );
            sink = None;
        }
    }

    eprintln!("stopped");
    ExitCode::SUCCESS
}

fn log_unless_repeated(last_logged: &mut String, msg: String) {
    if msg != *last_logged {
        eprintln!("{msg}");
        *last_logged = msg;
    }
}
