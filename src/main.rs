use clap::Parser;
use cargo_vendor_filterer::{run, Args, SELF_NAME};

/// Output
fn main() {
    let mut args = std::env::args().collect::<Vec<_>>();
    // When invoked as a subcommand of `cargo`, it passes the subcommand name as
    // the second argument, which is a bit inconvenient for us.  Special case that.
    if args.get(1).map(|s| s.as_str()) == Some(SELF_NAME) {
        args.remove(1);
    }

    if let Err(e) = run(Args::parse_from(args)) {
        // I prefer seeing errors like error: While processing foo: No such file or directory
        // instead of multi-line.
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}
