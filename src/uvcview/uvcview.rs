use libc::consts::os::posix88::{EINVAL,MAP_SHARED,EAGAIN};
use libc::{c_int,O_RDWR};
use libc;
use std::mem::transmute;
use std::default::Default;
use std::fmt;
use std::io::{IoResult,IoError,OtherIoError,TypeUnknown,MismatchedFileTypeForOperation};
use std::io;
use std::os::error_string;
use sdl;
use std::os;
use std::os::{MemoryMap,MapReadable,MapWritable,MapFd,MapNonStandardFlags};
use v4l2;
use v4l2::{v4l2_capability,v4l2_crop,v4l2_cropcap,v4l2_format,v4l2_ioctl};

struct Buffer {
    pub memory_map: MemoryMap,
    pub length: u32,
}

pub struct UvcView {
    pub device_path: Path,
    pub fd: c_int,
    pub width: u32,
    pub height: u32,
    pub buffers: Vec<Buffer>,
    pub surface: Option<sdl::video::Surface>,
}

impl Default for UvcView {
    fn default() -> UvcView {
        UvcView {
            device_path: Path::new("/dev/video0"),
            fd: -1,
            width: 1280,
            height: 720,
            buffers: vec!(),
            surface: None,
        }
    }
}

impl fmt::Show for UvcView {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "device_path : {}\nfd : {}\nwidth : {}\nheight : {}",
               self.device_path.display(), self.fd, self.width, self.height)
    }
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
                e.detail = Some("open() failed".to_owned());
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
                    detail: Some(error_string(os::errno() as uint))
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
                        detail: Some(error_string(e as uint))
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
            Err(errno) => {
                return Err(IoError {
                    kind: io::OtherIoError,
                    desc: "init(): ioctl() returns -1",
                    detail: Some(error_string(errno as uint))
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

        /*
        let mut frmsize: v4l2::v4l2_frmivalenum = Default::default();

        match v4l2_ioctl(self.fd, v4l2::VIDIOC_ENUM_FRAMEINTERVALS, unsafe { transmute(&mut frmsize) }) {
            Ok(_) => {}
            Err(e) => {
                fail!("VIDIOC_ENUM_FRAMEINTERVALS failed! {}", e);
            }
        }*/
        //println!("frmsize.he = {}", frmsize.he);

        // mmap initialization

        let mut req: v4l2::v4l2_requestbuffers = Default::default();

        req.count = 4;
        req._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
        req.memory = v4l2::V4L2_MEMORY_MMAP;

        match v4l2_ioctl(self.fd, v4l2::VIDIOC_REQBUFS, unsafe { transmute(&mut req) }) {
            Ok(_) => {}
            Err(errno) => {
                if errno == EINVAL {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): ioctl() returns -1",
                        detail: Some(format!("{} does not support memory mapping",
                                     self.device_path.display()))
                    });
                } else {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init(): ioctl() returns -1",
                        detail: Some(error_string(errno as uint))
                    });
                }
            }
        }

        if req.count < 2 {
            return Err(IoError {
                kind: io::OtherIoError,
                desc: "init() error",
                detail: Some(format!("Insufficient buffer memory on {}", self.device_path.display()))
            });
        }

        let mut count = 0;
        while count < req.count {
            let mut buf: v4l2::v4l2_buffer = Default::default();
            buf._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
            buf.memory = v4l2::V4L2_MEMORY_MMAP;
            buf.index = count;

            match v4l2_ioctl(self.fd, v4l2::VIDIOC_QUERYBUF, unsafe { transmute(&mut buf) }) {
                Ok(_) => {}
                Err(errno) => {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init() error",
                        detail: Some(error_string(errno as uint)),
                    });
                }
            }

            match MemoryMap::new(buf.length as uint,
                                 &[MapReadable, MapWritable, MapFd(self.fd),
                                   MapNonStandardFlags(MAP_SHARED)]) {
                Ok(m) => {
                    self.buffers.push(Buffer {
                        memory_map: m,
                        length: buf.length
                    });
                }
                Err(e) => {
                    return Err(IoError {
                        kind: io::OtherIoError,
                        desc: "init() error",
                        detail: Some(format!("MemoryMap::new() failed. {}", e))
                    });
                }
            }

            count += 1;
        }

        return Ok(self);
    }

    pub fn set_surface(&mut self, surface: sdl::video::Surface) {
        self.surface = Some(surface);
    }

    pub fn start_capturing(&mut self) {
        let mut i = 0;
        for _ in self.buffers.iter() {
            let mut buf: v4l2::v4l2_buffer = Default::default();
            buf._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
            buf.memory = v4l2::V4L2_MEMORY_MMAP;
            buf.index = i;

            match v4l2::v4l2_ioctl(self.fd, v4l2::VIDIOC_QBUF, unsafe { transmute(&mut buf) }) {
                Ok(_) => {}
                Err(e) => {
                    fail!("VIDIOC_QBUF failed. {}", e);
                }
            }

            i = i + 1;
        }

        let mut buf_type: v4l2::v4l2_buf_type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;

        match v4l2::v4l2_ioctl(self.fd, v4l2::VIDIOC_STREAMON, unsafe { transmute(&mut buf_type) }) {
            Ok(_) => {}
            Err(e) => {
                fail!("VIDIOC_STERAMON failed. {}", error_string(e as uint));
            }
        }
    }

    pub fn stop_capturing(&mut self) {
        let mut buf_type: v4l2::v4l2_buf_type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;

        match v4l2::v4l2_ioctl(self.fd, v4l2::VIDIOC_STREAMOFF, unsafe { transmute(&mut buf_type) }) {
            Ok(_) => {}
            Err(e) => {
                fail!("VIDIOC_STREAMOFF failed. {}", error_string(e as uint));
            }
        }
    }

    pub fn read_frame(&mut self) -> bool {
        let mut buffer: v4l2::v4l2_buffer = Default::default();

        buffer._type = v4l2::V4L2_BUF_TYPE_VIDEO_CAPTURE;
        buffer.memory = v4l2::V4L2_MEMORY_MMAP;

        match v4l2::v4l2_ioctl(self.fd, v4l2::VIDIOC_DQBUF, unsafe { transmute(&mut buffer) }) {
            Ok(_) => {}
            Err(EAGAIN) => {
                return false;
            }
            Err(e) => {
                fail!("VIDIOC_DQBUF failed. {}", error_string(e as uint));
            }
        }

        if buffer.index >= self.buffers.len() as u32 {
            fail!();
        }

        self.process_image(buffer.index);

        match v4l2::v4l2_ioctl(self.fd, v4l2::VIDIOC_QBUF, unsafe { transmute(&mut buffer) }) {
            Ok(_) => {}
            Err(e) => {
                fail!("VIDIOC_QBUF failed. {}", error_string(e as uint));
            }
        }
        return true;
    }

    fn yuv422_to_rgb(dest: *mut u8, src: *mut u8) {
        unsafe {
            let y0 = *src as f64;
            let cb = *src.offset(1) as f64;
            let y1 = *src.offset(2) as f64;
            let cr = *src.offset(3) as f64;

            *dest.offset(0) = (y0 + 1.77200 * (cb - 128.0)) as u8;
            *dest.offset(1) = (y0 - 0.34414 * (cb - 128.0) - 0.71414 * (cr - 128.0)) as u8;
            *dest.offset(2) = (y0 + 1.40200 * (cr - 128.0)) as u8;

            *dest.offset(3) = (y1 + 1.77200 * (cb - 128.0)) as u8;
            *dest.offset(4) = (y1 - 0.34414 * (cb - 128.0) - 0.71414 * (cr - 128.0)) as u8;
            *dest.offset(5) = (y1 + 1.40200 * (cr - 128.0)) as u8;

        }
    }

    fn process_image(&mut self, buffer_index: u32) {
        println!("buffer_index = {}", buffer_index);
        match self.surface {
            Some(ref surface) => {
                surface.with_lock(|pixels| {
                    let buffer = self.buffers.get(buffer_index as uint);
                    let dest = pixels.as_mut_ptr();
                    let src = buffer.memory_map.data;
                    unsafe {
                        let mut y: int = 0;
                        while y < self.height as int {
                            let mut x = 0;
                            while x < self.width as int {
                                UvcView::yuv422_to_rgb(
                                    dest.offset((y * self.width as int + x) * 3),
                                    src.offset((y * self.width as int + x) * 2));
                                x += 2;
                            }
                            y += 1;
                        }
                    }
                });
                surface.flip();
            }
            None => {}
        }
    }
}

impl Drop for UvcView {
    fn drop(&mut self) {
        if self.fd != -1 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}
