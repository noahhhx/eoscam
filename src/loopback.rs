use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::fd::AsRawFd;
use std::path::PathBuf;

const V4L2_BUF_TYPE_VIDEO_OUTPUT: u32 = 2;
const V4L2_FIELD_NONE: u32 = 1;
const V4L2_COLORSPACE_SRGB: u32 = 8;
const V4L2_PIX_FMT_YUYV: u32 = u32::from_le_bytes(*b"YUYV");

#[repr(C)]
#[derive(Clone, Copy)]
struct V4l2PixFormat {
    width: u32,
    height: u32,
    pixelformat: u32,
    field: u32,
    bytesperline: u32,
    sizeimage: u32,
    colorspace: u32,
    priv_: u32,
    flags: u32,
    ycbcr_enc: u32,
    quantization: u32,
    xfer_func: u32,
}

// The kernel's union contains pointer-bearing structs, hence the 8-byte alignment.
#[repr(C)]
union V4l2FormatUnion {
    pix: V4l2PixFormat,
    raw: [u8; 200],
    _align: [u64; 25],
}

#[repr(C)]
struct V4l2Format {
    type_: u32,
    fmt: V4l2FormatUnion,
}

const _: () = assert!(std::mem::size_of::<V4l2Format>() == 208);

nix::ioctl_readwrite!(vidioc_s_fmt, b'V', 5, V4l2Format);

/// Device numbers are not stable across boots, the name set via `card_label` is.
pub fn find_by_name(name: &str) -> Option<PathBuf> {
    fs::read_dir("/sys/class/video4linux")
        .ok()?
        .flatten()
        .find(|entry| fs::read_to_string(entry.path().join("name")).is_ok_and(|n| n.trim() == name))
        .map(|entry| PathBuf::from("/dev").join(entry.file_name()))
}

pub struct Loopback {
    file: File,
    pub path: PathBuf,
}

impl Loopback {
    pub fn open(path: PathBuf, width: u32, height: u32) -> io::Result<Self> {
        let file = OpenOptions::new().read(true).write(true).open(&path)?;
        let mut format = V4l2Format {
            type_: V4L2_BUF_TYPE_VIDEO_OUTPUT,
            fmt: V4l2FormatUnion {
                pix: V4l2PixFormat {
                    width,
                    height,
                    pixelformat: V4L2_PIX_FMT_YUYV,
                    field: V4L2_FIELD_NONE,
                    bytesperline: width * 2,
                    sizeimage: width * height * 2,
                    colorspace: V4L2_COLORSPACE_SRGB,
                    priv_: 0,
                    flags: 0,
                    ycbcr_enc: 0,
                    quantization: 0,
                    xfer_func: 0,
                },
            },
        };
        unsafe { vidioc_s_fmt(file.as_raw_fd(), &mut format) }.map_err(io::Error::from)?;
        Ok(Self { file, path })
    }

    pub fn write_frame(&mut self, frame: &[u8]) -> io::Result<()> {
        self.file.write_all(frame)
    }
}
