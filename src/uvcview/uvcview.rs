use std::c_str::CString;
use std::cast::transmute;
use std::default::Default;
use std::fmt;
use std::io::{IoResult,IoError,OtherIoError,TypeUnknown,MismatchedFileTypeForOperation};
use std::io;
use std::libc::consts::os::posix88::{EINVAL};
use std::libc::{c_int,O_RDWR};
use std::libc;
use std::os;
use v4l2;
use v4l2::{v4l2_capability,v4l2_crop,v4l2_cropcap,v4l2_format,v4l2_ioctl};

pub struct UvcView {
    device_path: Path,
    fd: c_int,
    width: u32,
    height: u32,
}

impl Default for UvcView {
    fn default() -> UvcView {
        UvcView {
            device_path: Path::new("/dev/video0"),
            fd: -1,
            width: 640,
            height: 480,
        }
    }
}

impl fmt::Show for UvcView {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f.buf, "device_path : {}\nfd : {}\nwidth : {}\nheight : {}",
               self.device_path.display(), self.fd, self.width, self.height)
    }
}

pub fn errno_msg() -> ~str {
    let errno = os::errno();
    let err_msg = unsafe {
        CString::new(libc::strerror(errno as c_int), false)
            .as_str().unwrap_or("unknown error");
    };
    format!("{} ({}", err_msg, errno)
}

impl UvcView {
    pub fn open<'a>(&'a mut self) -> IoResult<&'a mut UvcView> {
        match self.device_path.stat() {
            Ok(stat) => {
                if stat.kind != io::TypeUnknown/*TypeCharacter?*/ {
                    return Err(IoError {
                        kind: io::MismatchedFileTypeForOperation,
                        desc: "open(): invalid file type",
                        detail: Some(format!("{} is not device", self.device_path.display()))
                    });
                }
            }
            Err(mut e) => {
                e.detail = Some(~"open() failed");
                return Err(e);
            }
        }

        static O_NONBLOCK: c_int = 04000;
        match self.device_path.with_c_str(|path| {
            unsafe { libc::open(path, O_RDWR | O_NONBLOCK, 0) }
        }) {
            -1 => {
                return Err(IoError {
                    kind: io::OtherIoError,
                    desc: "open() failed",
                    detail: Some(errno_msg())
                });
            }
            fd => {
                self.fd = fd;
                return Ok(self);
            }
        }
    }

    pub fn init<'a>(&'a mut self) -> IoResult<&'a mut UvcView> {
        let mut cap: v4l2::v4l2_capability = Default::default();
        match v4l2_ioctl(self.fd, v4l2::VIDIOC_QUERYCAP, unsafe { transmute(&mut cap) }) {
            Ok(_) => {
                if (cap.capabilities & v4l2::V4L2_CAP_VIDEO_CAPTURE) == 0 {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): V4L2_CAP_VIDEO_CAPTURE not supported",
                        detail: Some(format!("{} is no video capture device", self.device_path.display()))
                    });
                }
                if (cap.capabilities & v4l2::V4L2_CAP_STREAMING) == 0 {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): V4L2_CAP_STREAMING not supported",
                        detail: Some(format!("{} dose not support streaming i/o", self.device_path.display()))
                    });
                }
            }
            Err(e) => {
                if e == EINVAL {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): VIDIOC_QUERYCAP not supported",
                        detail: Some(format!("{} is no v4l2 device", self.device_path.display()))
                    });
                } else {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): ioctl() returns -1",
                        detail: Some(errno_msg())
                    });
                }
            }
        }

        // Select video input, video standard and tune here.

        let mut cropcap: v4l2::v4l2_cropcap = Default::default();

        match v4l2_ioctl(self.fd, v4l2::VIDIOC_CROPCAP, unsafe { transmute(&mut cropcap) }) {
            Ok(_) => {
                let mut crop: v4l2::v4l2_crop = Default::default();
                crop._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
                crop.c = cropcap.defrect;


                match v4l2_ioctl(self.fd, v4l2::VIDIOC_S_CROP, unsafe { transmute(&mut crop) }) {
                    Ok(_) => {}
                    Err(EINVAL) => {
                        // Cropping not supported.
                    }
                    Err(_) => {
                        // Errors ignored.
                    }
                }
            }
            Err(_) => {
                // Errors ignored.
            }
        }

        let mut fmt: v4l2_format = Default::default();
        fmt._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
        let pix = fmt.fmt.pix();
        unsafe {
            (*pix).width = self.width;
            (*pix).height= self.height;
            (*pix).pixelformat = v4l2::V4L2_PIX_FMT_YUYV;
            (*pix).field = v4l2::V4L2_FIELD_INTERLACED; // TODO
        }

        match v4l2_ioctl(self.fd, v4l2::VIDIOC_S_FMT, unsafe { transmute(&mut fmt) }) {
            Ok(_) => {}
            Err(_) => {
                return Err(IoError {
                    kind: io::OtherIoError,
                    desc: "init(): ioctl() returns -1",
                    detail: Some(errno_msg())
                });
            }
        }

        // Note VIDIOC_S_FMT may change width and height

        // Buggy driver paranoia
        unsafe {
            let mut min = (*pix).width * 2;
            if (*pix).bytesperline < min {
                (*pix).bytesperline = min;
            }
            min = (*pix).bytesperline * (*pix).height;
            if (*pix).sizeimage < min {
                (*pix).sizeimage = min;
            }

            if (*pix).width != self.width {
                self.width = (*pix).width;
            }
            if (*pix).height != self.height {
                self.height = (*pix).height;
            }
        }

        return Ok(self);
    }
}