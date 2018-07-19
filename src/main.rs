#![feature(alloc_system)]
extern crate alloc_system;

extern crate atom_syndication;
extern crate failure;
extern crate futures;
extern crate htmlescape;
extern crate hyper;
extern crate select;
extern crate structopt;
#[macro_use]
extern crate structopt_derive;
extern crate tokio_core;

use atom_syndication::{Content, ContentBuilder, Feed, LinkBuilder};
use failure::{err_msg, Error};
use futures::future::{join_all, result};
use futures::{Future, Stream};
use hyper::Uri;
use hyper::client::{Client, HttpConnector};
use select::document::Document;
use select::predicate::Class;
use structopt::StructOpt;
use tokio_core::reactor::Core;

use std::fs::File;
use std::process;
use std::str::FromStr;
use std::str;

const SOURCE_URL: &str = "http://dilbert.com/feed";

#[derive(StructOpt)]
#[structopt(name = "dilbert-feed", about = "Generate Dilbert Atom feed with images.")]
struct Command {
    #[structopt(short = "u", long = "url", help = "URL for the feed")]
    url: Option<String>,
    #[structopt(help = "Output file")]
    output: Option<String>,
}

struct ComicInfo {
    title: Option<String>,
    image_url: Option<String>
}

fn extract_comic_info(
    client: &Client<HttpConnector>,
    url: Uri,
) -> impl Future<Item = ComicInfo, Error = Error> {
    client
        .get(url)
        .and_then(|resp| resp.body().concat2().map(|chunk| chunk.to_vec()))
        .from_err()
        .and_then(|bytes| {
            let source = str::from_utf8(&bytes)?;
            let document = Document::from(source);
            let image_url = document
                .find(Class("img-comic"))
                .next()
                .and_then(|image| image.attr("src"))
                .map(|url| url.to_owned());
            let title = document
                .find(Class("comic-title-name"))
                .next()
                .map(|title| title.text());
            Ok(ComicInfo { title, image_url })
        })
}

fn create_content(url: &str) -> Content {
    ContentBuilder::default()
        .content_type("html".to_owned())
        .value(htmlescape::encode_minimal(&format!(
            r#"<img src="{}">"#,
            url
        )))
        .build()
        .unwrap()
}

fn create_feed(
    client: Client<HttpConnector>,
    feed_url: Option<String>,
) -> impl Future<Item = Feed, Error = Error> {
    let uri = Uri::from_str(SOURCE_URL).unwrap();

    client
        .get(uri)
        .and_then(|resp| resp.body().concat2().map(|chunk| chunk.to_vec()))
        .from_err()
        .and_then(|bytes| String::from_utf8(bytes).map_err(From::from))
        .and_then(|source| Feed::from_str(&source).map_err(|_| err_msg("invalid feed")))
        .and_then(move |mut feed| {
            feed.set_links(
                feed_url
                    .iter()
                    .cloned()
                    .map(|url| {
                        LinkBuilder::default()
                            .href(url)
                            .rel("self")
                            .mime_type(Some("application/atom+xml".to_owned()))
                            .build()
                            .unwrap()
                    })
                    .collect::<Vec<_>>(),
            );

            result(
                feed.entries()
                    .iter()
                    .cloned()
                    .filter_map(|mut entry| -> Option<Result<_, Error>> {
                        let url = entry
                            .links()
                            .iter()
                            .next()
                            .ok_or(err_msg("missing link"))
                            .map(|link| link.href())
                            .and_then(|address| Uri::from_str(address).map_err(From::from));
                        let url = match url {
                            Ok(url) => url,
                            Err(err) => return Some(Err(err)),
                        };
                        let future =
                            extract_comic_info(&client, url.clone()).map(move |info| {
                                let content = create_content(&info.image_url?);
                                entry.set_content(content);
                                entry.set_id(url.as_ref().to_owned());
                                if let Some(title) = info.title {
                                    entry.set_title(title);
                                }
                                Some(entry)
                            });
                        Some(Ok(future))
                    })
                    .collect(),
            ).and_then(|entries: Vec<_>| {
                join_all(entries).and_then(|entries| {
                    let entries: Vec<_> = entries.into_iter().filter_map(|entry| entry).collect();
                    feed.set_entries(entries);
                    Ok(feed)
                })
            })
        })
}

fn process() -> Result<(), Error> {
    let options = Command::from_args();
    let mut core = Core::new()?;
    let client = Client::new(&core.handle());
    let future = create_feed(client, options.url.clone()).and_then(
        |feed| {
            if let Some(path) = options.output {
                let mut file = File::create(path)?;
                feed.write_to(file).map_err(|_| err_msg("failed to serialize feed"))?;
            } else {
                println!("{}", feed.to_string())
            }
            Ok(())
        },
    );
    core.run(future)
}

fn main() {
    if let Err(err) = process() {
        eprintln!("{}", err);
        process::exit(1);
    }
}
