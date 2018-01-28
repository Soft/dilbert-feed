extern crate atom_syndication;
extern crate reqwest;
extern crate select;
extern crate failure;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;

use atom_syndication::{Feed, Content};
use std::io::BufReader;
use std::fs::File;
use select::document::Document;
use select::predicate::Class;
use failure::{Error, err_msg};
use structopt::StructOpt;

const SOURCE_URL: &str = "http://dilbert.com/feed";

#[derive(StructOpt)]
#[structopt(name = "dilbert-feed", about = "Generate Dilbert Atom feed with images.")]
struct Command {
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
    content.set_content_type(Some("image/png".to_owned()));
    content.set_src(url.to_owned());
    content
}

fn create_feed() -> Result<Feed, Error> {
    let response = reqwest::get(SOURCE_URL)?;
    let response = BufReader::new(response);

    let mut feed = Feed::read_from(response)
        .map_err(|_| err_msg("invalid feed"))?
        .clone();

    let entries: Result<Vec<_>, Error> = feed.entries()
        .iter()
        .cloned()
        .map(|mut entry| {
            let image_url = {
                let link = entry.links().iter().next()
                    .ok_or(err_msg("missing link"))?;
                extract_image_url(link.href())
            };
            let content = create_content(&image_url?);
            entry.set_content(content);
            Ok(entry)
        })
        .collect();
    feed.set_entries(entries?);
    Ok(feed)
}

fn process() -> Result<(), Error> {
    let options = Command::from_args();
    let feed = create_feed()?;
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
        eprintln!("{}", err)
    }
}
