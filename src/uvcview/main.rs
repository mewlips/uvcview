#![feature(phase)]

extern crate getopts;
extern crate sdl;
extern crate libc;
#[phase(syntax, link)]
extern crate log;

use getopts::{getopts,optopt,optflag,usage};
use libc::consts::os::c95::EXIT_FAILURE;
use libc::consts::os::posix88::{EINTR};
use std::mem::{transmute};
use std::default::Default;
use std::mem;
use std::os;
use std::ptr::{null};
use uvcview::UvcView;

mod v4l2;
mod uvcview;

pub fn main() {
    let args = os::args();
    let program = args.get(0).clone();
    let description = "UVC Viewer";

    let mut uvcview: UvcView = Default::default();

    let opts = ~[
        optopt("d", "device", format!("set video device path (default: {})",
                              uvcview.device_path.as_str().unwrap_or("<None>")).as_slice(),
               "<device_path>"),
        optopt("x", "width", format!("set width (default: {})",
                                     uvcview.width).as_slice(),
               "<x>"),
        optopt("y", "height", format!("set height (default: {})",
                                      uvcview.height).as_slice(),
               "<y>"),
        optflag("h", "help", "show help messages"),
    ];

    let print_usage = || {
        let brief = format!("{} : {}", program, description);
        print!("{}", usage(brief.as_slice(), opts));
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

    match matches.opt_str("device") {
        Some(device_path) => {
            uvcview.device_path = Path::new(device_path);
        }
        _ => {}
    }
    uvcview.width = matches.opt_str("width").map_or(uvcview.width, |s| {
        from_str::<u32>(s.as_slice()).unwrap_or_else(|| { fail!("invalid option argument") })
    });
    uvcview.height = matches.opt_str("height").map_or(uvcview.height, |s| {
        from_str::<u32>(s.as_slice()).unwrap_or_else(|| { fail!("invalid option argument") })
    });

    match uvcview.open().and_then(|uvcview| {
          uvcview.init()
    }) {
        Ok(_) => {
            info!("{}", uvcview);
            info!("success");
        }
        Err(e) => {
            info!("{}", uvcview);
            fail!("{}", e);
        }
    }

    match sdl::init(&[sdl::InitVideo]) {
        true => {}
        false => {
            fail!("sdl::init() failed");
        }
    }

    sdl::wm::set_caption("uvcview", "uvcview");

    let width = uvcview.width as uint;
    let height = uvcview.height as uint;
/*    let surface = match sdl::video::Surface::new(
                            &[sdl::video::HWSurface],
                            width as int, height as int, 24,
                            0xff, 0xff00, 0xff0000, 0) {
        Ok(surface) => surface,
        Err(err) => fail!("Surface::new() failed. {}", err)
    };
    */
    let surface = match sdl::video::set_video_mode(
            width as int, height as int, 24,
            [sdl::video::HWSurface], [sdl::video::DoubleBuf]) {
        Ok(surface) => surface,
        Err(err) => fail!("sdl::video::set_video_mode() failed! {}", err)
    };
    uvcview.set_surface(surface);

    uvcview.start_capturing();
    main_loop(&mut uvcview);
    uvcview.stop_capturing();
}

fn main_loop(uvcview: &mut UvcView) {
    loop {
        match sdl::event::poll_event() {
            sdl::event::QuitEvent => {
                return;
            }
            _ => {
            }
        }
        loop {
            let mut set: FdSet = unsafe { mem::zeroed() };
            let mut tv = libc::timeval { tv_sec: 2, tv_usec: 0 };

            FdSet(&mut set, uvcview.fd);

            let result = unsafe {
                select(uvcview.fd + 1, transmute(&mut set),
                       null(), null(), transmute(&mut tv))
            };
            match result {
                -1 => {
                    if os::errno() == EINTR as int {
                        continue
                    }
                    fail!("select() failed");
                }
                0 => {
                    fail!("select() timeout");
                }
                _ => {
                    if uvcview.read_frame() {
                        break;
                    }

                    // EAGAIN - continue select loop
                }
            }
        }
    }
}

pub static FD_SETSIZE: uint = 1024;

pub struct FdSet {
    fds_bits: [u64, ..(FD_SETSIZE / 64)]
}

pub fn FdSet(set: &mut FdSet, fd: i32) {
    set.fds_bits[(fd / 64) as uint] |= (1 << (fd % 64)) as u64;
}

extern {
    pub fn select(nfds: libc::c_int,
                  readfds: *FdSet,
                  writefds: *FdSet,
                  errorfds: *FdSet,
                  timeout: *libc::timeval) -> libc::c_int;
}
