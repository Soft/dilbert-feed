extern crate atom_syndication;
extern crate reqwest;
extern crate select;

use atom_syndication::{Feed, Content};
use std::io::BufReader;
use select::document::Document;
use select::predicate::Class;

const SOURCE_URL: &str = "http://dilbert.com/feed";

fn extract_image_url(url: &str) -> String {
    let response = reqwest::get(url).unwrap();
    let response = Document::from_read(response).unwrap();
    let image = response.find(Class("img-comic")).next().unwrap();
    image.attr("src").unwrap().to_owned()
}

fn create_content(url: &str) -> Content {
    let mut content = Content::default();
    content.set_content_type(Some("image/png".to_owned()));
    content.set_src(url.to_owned());
    content
}

fn main() {
    let response = reqwest::get(SOURCE_URL).unwrap();
    let response = BufReader::new(response);

    let mut feed = Feed::read_from(response)
        .unwrap()
        .clone();

    let entries: Vec<_> = feed.entries()
        .iter()
        .cloned()
        .map(|mut entry| {
            let image_url = {
                let link = entry.links().iter().next().unwrap();
                extract_image_url(link.href())
            };
            let content = create_content(&image_url);
            entry.set_content(content);
            entry
        })
        .collect();
    feed.set_entries(entries);
    
    println!("{}", feed.to_string());
}
