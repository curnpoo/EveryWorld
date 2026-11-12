//! `ew`: the EveryWorld engine shell (Phase 1, M2). See `ew --help`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let args = match ew::args::parse(std::env::args().skip(1)) {
        Ok(Some(a)) => a,
        Ok(None) => {
            println!("{}", ew::args::USAGE);
            return ExitCode::SUCCESS;
        }
        Err(e) => {
            eprintln!("ew: {e}\n\n{}", ew::args::USAGE);
            return ExitCode::from(2);
        }
    };
    if args.sim_route {
        return match ew::route::run(&args.packs, &args.layers) {
            Ok(r) => {
                print!("{}", r.lines());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("ew: error: {e}");
                ExitCode::FAILURE
            }
        };
    }
    let result = if args.headless {
        ew::headless::run(&args).map(|r| {
            print!("{}", r.summary);
            println!("ew: wrote {} frames to {}", r.frames, args.out.display());
            for (name, p) in &r.shots {
                println!("ew: {name} screenshot {}", p.display());
            }
        })
    } else {
        ew::windowed::run(args)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("ew: error: {e}");
            ExitCode::FAILURE
        }
    }
}
