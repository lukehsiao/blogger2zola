#![forbid(unsafe_code)]
// #![forbid(warnings)]

use std::fs::{self, File};
use std::io::{copy, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
use atom_syndication::{Entry, Feed};
use log::{info, warn};
use select::document::Document;
use select::predicate::Name;
use structopt::clap::AppSettings;
use structopt::StructOpt;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum DownloadError {
    #[error("Error {status}: Image path is redirected.")]
    REDIRECT { status: u16 },
    #[error("Error: This URL does not point to an image.")]
    NONIMAGE,
    #[error("Error {status}: Unable to download image.")]
    UNKNOWN { status: u16 },
}

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

fn pandoc_html_to_md(input: &str) -> Result<String> {
    let mut child = Command::new("pandoc")
        .arg("--from=html")
        .arg("--to=gfm")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("Child process stdin has not been captured."))?
        .write_all(input.as_bytes())?;

    let output = child.wait_with_output()?;

    if output.status.success() {
        let raw_output = String::from_utf8(output.stdout)?;
        Ok(raw_output)
    } else {
        let err = String::from_utf8(output.stderr)?;
        Err(anyhow!("External command failed:\n {}", err))
    }
}

fn download_and_save_image(path: &PathBuf, url: &str) -> Result<PathBuf> {
    info!("Downloading: {}", url);

    // Don't follow redirects since for old posts, most likely the image is
    // gone.
    let r = ureq::get(url).redirects(0).timeout_connect(1_000).call();

    match r.status() {
        200..=299 => match r.header("Content-Type") {
            Some(s) if s.starts_with("image") => {
                let img_path = r
                    .get_url()
                    .rsplit('/')
                    .next()
                    .expect("No slash (/) in url.")
                    .to_lowercase();

                let filename = path.join(&img_path);
                let mut dest = File::create(&filename)?;

                let mut reader = r.into_reader();

                copy(&mut reader, &mut dest)?;
                Ok(PathBuf::from(img_path))
            }
            // Allow one level of search for deeper image links. This is common on
            // blogspot.
            Some(s) if s.starts_with("text") => {
                let content = r.into_string()?;
                if let Some(deep_url) = Document::from(content.as_str())
                    .find(Name("img"))
                    .filter_map(|n| n.attr("src"))
                    .next()
                {
                    download_and_save_image(path, deep_url)
                } else {
                    Err(DownloadError::NONIMAGE.into())
                }
            }
            _ => Err(DownloadError::NONIMAGE.into()),
        },
        r if r >= 300 && r <= 399 => Err(DownloadError::REDIRECT { status: r }.into()),
        s => Err(DownloadError::UNKNOWN { status: s }.into()),
    }
}

fn process_post(args: &Args, entry: Entry) -> Result<()> {
    info!("Processing {}...\n", entry.title);

    let slug = slug::slugify(&entry.title);
    let slug = format!("{}_{}", entry.updated().format("%F"), slug);

    let path: PathBuf = [&args.outdir, &PathBuf::from(slug)].iter().collect();

    if let Err(e) = fs::create_dir(&path) {
        warn!("Directory exists: {}", e);
    }

    let html_content = match &entry.content {
        Some(c) => c.value().unwrap().to_string(),
        None => return Err(anyhow!("No post content.")),
    };

    let mut markdown_content = pandoc_html_to_md(html_content.as_str())?;

    // Download all the IMGs
    for node in Document::from(html_content.as_str()).find(Name("img")) {
        match (
            node.attr("src"),
            node.parent().expect("img w/o href").attr("href"),
        ) {
            (Some(thumb), Some(orig)) => {
                match download_and_save_image(&path, &orig) {
                    Err(e) => warn!("{}", e),
                    Ok(p) => {
                        // Rewrite the paths in the HTML
                        markdown_content = markdown_content
                            .as_str()
                            .replace(thumb, p.to_str().unwrap());
                        markdown_content =
                            markdown_content.as_str().replace(orig, p.to_str().unwrap());
                    }
                }
            }
            _ => continue,
        }
    }

    // Write the file with TOML frontmatter
    // TODO: Totally ignoring tags/categories for now.
    let result = format!(
        r#"+++
title= "{}"
date = {}
draft = True

[extra]
author = "{}"
+++

{}"#,
        entry.title(),
        entry.updated().to_rfc3339(),
        entry.authors()[0].name(),
        markdown_content
    );
    fs::write(path.join("index.md"), result)?;

    Ok(())
}

pub fn run(args: Args) -> Result<()> {
    let scheme = String::from("http://schemas.google.com/g/2005#kind");
    let term = String::from("http://schemas.google.com/blogger/2008/kind#post");
    println!("Hello world.");
    info!("{:#?}", args);

    // Create the output directory
    if let Err(e) = fs::create_dir(&args.outdir) {
        warn!("Output directory: {}", e);
    }

    // Parse the XML file
    let xmlfile = File::open(&args.xml)?;
    let feed = Feed::read_from(BufReader::new(xmlfile))?;

    // TODO: Perhaps export post comments as well.
    for entry in feed.entries {
        if entry
            .categories
            .iter()
            .any(|c| match (c.scheme(), c.term()) {
                (Some(s), t) if s == scheme && t == term => true,
                _ => false,
            })
            && !entry.title().is_empty()
        {
            process_post(&args, entry)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_get_external_ip() {}
}
