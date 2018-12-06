use atom_syndication::{Content, ContentBuilder, Feed, LinkBuilder};
use failure::{err_msg, Error};
use hyper::body::Body;
use hyper::client::connect::Connect;
use hyper::client::Client;
use hyper::{Response, Uri};
use hyper_tls::HttpsConnector;
use select::document::Document;
use select::predicate::Class;
use structopt::StructOpt;
use tokio::prelude::*;
use tokio::runtime::Runtime;

use std::alloc::System;
use std::fs::File;
use std::process;
use std::str;
use std::str::FromStr;

const SOURCE_URL: &str = "https://dilbert.com/feed";
const ICON_URL: &str = "https://dilbert.com/favicon.ico";

#[global_allocator]
static ALLOCATOR: System = System;

#[derive(StructOpt)]
#[structopt(
    name = "dilbert-feed",
    about = "Generate Dilbert Atom feed with images."
)]
struct Command {
    #[structopt(short = "u", long = "url", help = "URL for the feed")]
    url: Option<String>,
    #[structopt(short = "e", long = "embed", help = "Embed images")]
    embed: bool,
    #[structopt(help = "Output file")]
    output: Option<String>,
}

struct ComicInfo {
    title: Option<String>,
    image_url: Option<String>,
}

fn concat_body(response: Response<Body>) -> impl Future<Item = Vec<u8>, Error = hyper::Error> {
    response.into_body().concat2().map(|chunk| chunk.to_vec())
}

fn extract_comic_info<T>(
    client: &Client<T>,
    url: Uri,
) -> impl Future<Item = ComicInfo, Error = Error>
where
    T: 'static + Sync + Connect,
{
    client
        .get(url)
        .and_then(concat_body)
        .from_err()
        .and_then(|bytes| {
            let source = str::from_utf8(&bytes)?;
            let document = Document::from(source);
            let image_url = document
                .find(Class("img-comic"))
                .next()
                .and_then(|image| image.attr("src"))
                .map(|url| {
                    let mut url = url.to_owned();
                    if url.starts_with("//") {
                        url.insert_str(0, "https:");
                    }
                    url
                });
            let title = document
                .find(Class("comic-title-name"))
                .next()
                .map(|title| title.text().trim().to_owned())
                .filter(|title| !title.is_empty());
            Ok(ComicInfo { title, image_url })
        })
}

fn create_data_url<T>(client: &Client<T>, url: Uri) -> impl Future<Item = String, Error = Error>
where
    T: 'static + Sync + Connect,
{
    client
        .get(url)
        .and_then(concat_body)
        .from_err()
        .map(|bytes| {
            let mime = tree_magic::from_u8(&bytes);
            let encoded = base64::encode(&bytes);
            format!("data:{};base64,{}", mime, encoded)
        })
}

fn create_content(url: &str) -> Content {
    ContentBuilder::default()
        .content_type("html".to_owned())
        .value(htmlescape::encode_minimal(&format!(
            r#"<img src="{}">"#,
            url
        ))).build()
        .unwrap()
}

fn create_feed<T>(
    client: Client<T>,
    feed_url: Option<String>,
    embed_images: bool,
) -> impl Future<Item = Feed, Error = Error>
where
    T: 'static + Sync + Connect,
{
    let uri = Uri::from_str(SOURCE_URL).unwrap();

    client
        .get(uri)
        .and_then(concat_body)
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
                    }).collect::<Vec<_>>(),
            );
            feed.set_icon(Some(ICON_URL.to_owned()));

            let entries: Result<Vec<_>, _> = feed
                .entries()
                .iter()
                .cloned()
                .map(|mut entry| {
                    let url = entry
                        .links()
                        .iter()
                        .next()
                        .ok_or_else(|| err_msg("entry without a link"))
                        .map(|link| link.href())
                        .and_then(|address| Uri::from_str(address).map_err(From::from))?;
                    let client2 = client.clone();
                    let extractor = extract_comic_info(&client, url.clone())
                        .and_then(move |info| {
                            if embed_images {
                                let image_url = info
                                    .image_url
                                    .clone()
                                    .ok_or_else(|| err_msg("entry without a comic"));
                                future::Either::A(
                                    future::result(image_url)
                                        .and_then(|url| Uri::from_str(&url).map_err(From::from))
                                        .and_then(move |url| create_data_url(&client2, url))
                                        .map(|url| ComicInfo {
                                            image_url: Some(url),
                                            ..info
                                        }),
                                )
                            } else {
                                future::Either::B(future::ok(info))
                            }
                        }).map(move |info| {
                            let content = create_content(&info.image_url?);
                            entry.set_content(content);
                            entry.set_id(url.to_string());
                            if let Some(title) = info.title {
                                entry.set_title(title);
                            }
                            Some(entry)
                        });
                    Ok(extractor)
                }).collect();

            future::result(entries).and_then(|entries| {
                future::join_all(entries).and_then(|entries| {
                    let entries: Vec<_> = entries.into_iter().filter_map(|entry| entry).collect();
                    feed.set_entries(entries);
                    Ok(feed)
                })
            })
        })
}

fn process() -> Result<(), Error> {
    let options = Command::from_args();
    let runtime = Runtime::new()?;
    let https = HttpsConnector::new(num_cpus::get())?;
    let client = Client::builder().build(https);
    let feed_creator = create_feed(client, options.url.clone(), options.embed).and_then(|feed| {
        if let Some(path) = options.output {
            let file = File::create(path)?;
            feed.write_to(file)
                .map_err(|_| err_msg("failed to serialize feed"))?;
        } else {
            println!("{}", feed.to_string())
        }
        Ok(())
    });
    runtime.block_on_all(feed_creator)
}

fn main() {
    if let Err(err) = process() {
        eprintln!("{}", err);
        process::exit(1);
    }
}
