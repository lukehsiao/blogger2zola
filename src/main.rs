#![forbid(unsafe_code)]
#![forbid(warnings)]

use anyhow::Result;
use structopt::StructOpt;

use blogger2zola::{run, Args};

fn main() -> Result<()> {
    pretty_env_logger::init_timed();
    let args = Args::from_args();

    run(args)
}
