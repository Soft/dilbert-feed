extern crate atom_syndication;
extern crate reqwest;
extern crate select;
extern crate failure;
extern crate htmlescape;
extern crate rayon;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use std::fs::File;
use std::io::BufReader;
use std::process;
use atom_syndication::{Feed, Content, LinkBuilder};
use failure::{Error, err_msg};
use select::document::Document;
use select::predicate::Class;
use structopt::StructOpt;
use rayon::prelude::*;

const SOURCE_URL: &str = "http://dilbert.com/feed";

#[derive(StructOpt)]
#[structopt(name = "dilbert-feed", about = "Generate Dilbert Atom feed with images.")]
struct Command {
    #[structopt(short = "u", long = "url", help = "URL for the feed")]
    url: Option<String>,
    #[structopt(help = "Output file")]
    output: Option<String>
}

fn extract_image_url(url: &str) -> Result<String, Error> {
    let response = reqwest::get(url)?;
    let response = Document::from_read(response)?;
    let image = response.find(Class("img-comic"))
        .next().ok_or(err_msg("missing image"))?;
    Ok(image.attr("src")
       .ok_or(err_msg("invalid image"))?
       .to_owned())
}

fn create_content(url: &str) -> Content {
    let mut content = Content::default();
    content.set_content_type(Some("html".to_owned()));
    content.set_value(htmlescape::encode_minimal(
        &format!(r#"<img src="{}">"#, url)));
    content
}

fn create_feed(url: Option<&str>) -> Result<Feed, Error> {
    let response = reqwest::get(SOURCE_URL)?;
    let response = BufReader::new(response);

    let mut feed = Feed::read_from(response)
        .map_err(|_| err_msg("invalid feed"))?
        .clone();
    feed.set_links(url.iter()
                   .cloned()
                   .map(|url| LinkBuilder::default()
                        .href(url)
                        .rel("self")
                        .mime_type(Some("application/atom+xml".to_owned()))
                        .build()
                        .unwrap())
                   .collect::<Vec<_>>());

    let entries: Result<Vec<_>, Error> = feed.entries()
        .par_iter()
        .cloned()
        .map(|mut entry| {
            let url = entry.links().iter().next()
                .ok_or(err_msg("missing link"))?
                .href()
                .to_owned();
            let image_url = extract_image_url(&url)?;
            let content = create_content(&image_url);
            entry.set_content(content);
            entry.set_id(url);
            Ok(entry)
        })
        .collect();
    feed.set_entries(entries?);
    Ok(feed)
}

fn process() -> Result<(), Error> {
    let options = Command::from_args();
    let feed = create_feed(options.url.as_ref().map(|s| &**s))?;
    if let Some(path) = options.output {
        let mut file = File::create(path)?;
        feed.write_to(file)
            .map_err(|_| err_msg("failed to serialize feed"))?;
    } else {
        println!("{}", feed.to_string());
    }
    Ok(())
}

fn main() {
    if let Err(err) = process() {
        eprintln!("{}", err);
        process::exit(1);
    }
}
