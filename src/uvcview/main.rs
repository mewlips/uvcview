extern crate getopts;

use getopts::{getopts,optopt,optflag,usage};
use std::cast::transmute;
use std::c_str::CString;
use std::default::Default;
use std::io;
use std::libc::consts::os::c95::EXIT_FAILURE;
use std::libc::{c_int,c_uint,c_ulong,c_void,O_RDWR};
use std::libc;
use std::os;

mod v4l2;

pub fn main() {
    let args = os::args();
    let program = args[0].clone();
    let description = "UVC Viewer";

    let default_device_path = ~"/dev/video0";
    let default_width = 640;
    let default_height = 480;

    let opts = ~[
        optopt("d", "device", format!("set video device path (default: {})",
                                      default_device_path),
               "<device_path>"),
        optopt("x", "width", format!("set width (default: {})",
                                     default_width),
               "<x>"),
        optopt("y", "height", format!("set height (default: {})",
                                      default_height),
               "<y>"),
        optflag("h", "help", "show help messages"),
    ];

    let print_usage = || {
        let brief = format!("{} : {}", program, description);
        print!("{}", usage(brief, opts));
    };

    let matches = match getopts(args.tail(), opts) {
        Ok(m) => { m }
        Err(f) => {
            error!("{}\n", f.to_err_msg());
            print_usage();
            os::set_exit_status(EXIT_FAILURE as int);
            return;
        }
    };

    if matches.opt_present("help") {
        print_usage();
        return;
    }

    let device_path = matches.opt_str("device").unwrap_or(default_device_path);
    info!("device_path = {}", device_path);
    let device_path = Path::new(device_path);

    let width = matches.opt_str("width").map_or(default_width, |s| {
        from_str::<int>(s).unwrap_or_else(|| { fail!("invalid option argument") })
    });
    let height = matches.opt_str("height").map_or(default_height, |s| {
        from_str::<int>(s).unwrap_or_else(|| { fail!("invalid option argument") })
    });
    info!("video_size = ({}, {})", width, height);

    let fd = match open_device(&device_path) {
        Some(fd) => { info!("open_device() success"); fd }
        None => fail!("open_device() failed")
    };

    init_device(fd);
}

fn open_device(device_path: &Path) -> Option<c_int> {
    match device_path.stat() {
        Ok(stat) => {
            if stat.kind != io::TypeUnknown/*TypeCharacter?*/ {
                error!("invalid file type");
                return None;
            }
        }
        Err(e) => {
            error!("stat failed: {}", e);
            return None;
        }
    }

    static O_NONBLOCK: c_int = 04000;
    match device_path.with_c_str(|path| {
        unsafe { libc::open(path, O_RDWR | O_NONBLOCK, 0) }
    }) {
        -1 => {
            let errno = os::errno();
            let err_msg = unsafe {
                CString::new(libc::strerror(errno as c_int), false)
                    .as_str().unwrap_or("unknown error");
            };
            error!("open() failed. errno = {}, {}", errno, err_msg);
            None
        }
        fd => Some(fd)
    }
}

fn init_device(fd: c_int) {
    let mut cap: v4l2::v4l2_capability = Default::default();
    unsafe {
        v4l2::xioctl(fd, v4l2::VIDIOC_QUERYCAP, transmute(&mut cap)) 
    };
}

