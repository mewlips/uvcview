extern crate getopts;
extern crate sdl;

use getopts::{getopts,optopt,optflag,usage};
use std::default::Default;
use std::libc::consts::os::c95::EXIT_FAILURE;
use std::os;
use uvcview::UvcView;

mod v4l2;
mod uvcview;

pub fn main() {
    let args = os::args();
    let program = args[0].clone();
    let description = "UVC Viewer";

    let mut uvcview: UvcView = Default::default();

    let opts = ~[
        optopt("d", "device", format!("set video device path (default: {})",
                                      uvcview.device_path.as_str()),
               "<device_path>"),
        optopt("x", "width", format!("set width (default: {})",
                                     uvcview.width),
               "<x>"),
        optopt("y", "height", format!("set height (default: {})",
                                      uvcview.height),
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

    match matches.opt_str("device") {
        Some(device_path) => {
            uvcview.device_path = Path::new(device_path);
        }
        _ => {}
    }
    uvcview.width = matches.opt_str("width").map_or(uvcview.width, |s| {
        from_str::<u32>(s).unwrap_or_else(|| { fail!("invalid option argument") })
    });
    uvcview.height = matches.opt_str("height").map_or(uvcview.height, |s| {
        from_str::<u32>(s).unwrap_or_else(|| { fail!("invalid option argument") })
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

    // TODO
}


