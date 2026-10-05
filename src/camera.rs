use std::fs;
use std::thread;
use std::time::Duration;

use gphoto2::{Camera, Context};

const CANON_USB_VENDOR: &str = "04a9";

/// gvfs grabs PTP cameras as soon as they appear, which stops libgphoto2 claiming them.
const GVFS_PROCESSES: [&str; 2] = ["gvfs-gphoto2-volume-monitor", "gvfsd-gphoto2"];

fn canon_usb_absent() -> bool {
    let Ok(devices) = fs::read_dir("/sys/bus/usb/devices") else {
        return false;
    };
    !devices.flatten().any(|dev| {
        fs::read_to_string(dev.path().join("idVendor")).is_ok_and(|v| v.trim() == CANON_USB_VENDOR)
    })
}

fn stop_gvfs_gphoto2() -> bool {
    let Ok(procs) = fs::read_dir("/proc") else {
        return false;
    };
    let mut killed = false;
    for entry in procs.flatten() {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|p| p.parse::<i32>().ok())
        else {
            continue;
        };
        let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
            continue;
        };
        let exe = cmdline.split(|&b| b == 0).next().unwrap_or_default();
        let exe = String::from_utf8_lossy(exe);
        if GVFS_PROCESSES.iter().any(|name| exe.ends_with(name)) {
            eprintln!("stopping {exe} (pid {pid}) so it releases the camera");
            killed |= unsafe { libc::kill(pid, libc::SIGTERM) } == 0;
        }
    }
    killed
}

pub fn open(context: &Context) -> gphoto2::Result<Option<Camera>> {
    if canon_usb_absent() {
        return Ok(None);
    }
    let Some(descriptor) = context
        .list_cameras()
        .wait()?
        .find(|c| c.model.starts_with("Canon"))
    else {
        return Ok(None);
    };

    if stop_gvfs_gphoto2() {
        thread::sleep(Duration::from_secs(1));
    }
    eprintln!("opening {} on {}", descriptor.model, descriptor.port);
    context.get_camera(&descriptor).wait().map(Some)
}
