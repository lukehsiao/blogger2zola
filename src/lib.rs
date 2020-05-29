#![forbid(unsafe_code)]
// #![forbid(warnings)]

use std::path::PathBuf;

use anyhow::Result;
// use log::{debug, info};
use structopt::clap::AppSettings;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
#[structopt(
    about,
    setting(AppSettings::ColoredHelp),
    setting(AppSettings::ColorAuto)
)]
pub struct Args {
    /// The location of the Blogger XML file.
    #[structopt(parse(from_os_str))]
    pub xml: PathBuf,

    /// The directory to save the Markdown files and images.
    #[structopt(parse(from_os_str))]
    pub outdir: PathBuf,
}

pub fn run(args: Args) -> Result<()> {
    print!("Hello world.");
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_external_ip() {}
}
