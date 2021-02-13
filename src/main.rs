use anyhow::Result;
use atom_syndication::{Content, ContentBuilder, Entry, Feed, LinkBuilder};
use futures::future;
use select::document::Document;
use select::predicate::Class;
use std::fs::File;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;

const SOURCE_URL: &str = "https://dilbert.com/feed";
const ICON_URL: &str = "https://dilbert.com/favicon.ico";

#[derive(StructOpt)]
#[structopt(
    name = "dilbert-feed",
    about = "Generate Dilbert Atom feed with images."
)]
struct Args {
    #[structopt(short = "u", long = "url", help = "URL for the feed")]
    url: Option<String>,
    #[structopt(short = "e", long = "embed", help = "Embed images")]
    embed: bool,
    #[structopt(help = "Output file")]
    output: Option<PathBuf>,
}

struct Comic {
    title: Option<String>,
    image_url: Option<String>,
}

async fn fetch_comic(client: &reqwest::Client, url: &str) -> Result<Comic> {
    let doc = Document::from(client.get(url).send().await?.text().await?.as_ref());
    let image_url = doc
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
    let title = doc
        .find(Class("comic-title-name"))
        .next()
        .map(|title| title.text().trim().to_owned())
        .filter(|title| !title.is_empty());
    Ok(Comic { title, image_url })
}

async fn create_data_url(client: &reqwest::Client, url: &str) -> Result<String> {
    let bytes = client.get(url).send().await?.bytes().await?;
    let mime = tree_magic::from_u8(&bytes);
    let encoded = base64::encode(&bytes);
    Ok(format!("data:{};base64,{}", mime, encoded))
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

async fn process_entry(
    client: &reqwest::Client,
    embed_images: bool,
    mut entry: Entry,
) -> Result<Entry> {
    let entry_url = {
        let entry_link = entry.links().iter().next();
        match entry_link {
            Some(link) => link.href().to_owned(),
            _ => return Ok(entry),
        }
    };
    let comic = fetch_comic(&client, &entry_url).await?;
    let mut image_url = match comic.image_url {
        Some(url) => url,
        _ => return Ok(entry),
    };
    if embed_images {
        image_url = create_data_url(&client, &image_url).await?;
    }
    if let Some(title) = comic.title {
        entry.set_title(title);
    }
    entry.set_content(create_content(&image_url));
    entry.set_id(entry_url);
    Ok(entry)
}

async fn create_feed(feed_url: Option<String>, embed_images: bool) -> Result<Feed> {
    let client = reqwest::Client::new();
    let mut feed = Feed::from_str(client.get(SOURCE_URL).send().await?.text().await?.as_ref())?;
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
    feed.set_icon(Some(ICON_URL.to_owned()));
    let entry_futures = feed
        .entries()
        .to_owned()
        .into_iter()
        .map(|entry| process_entry(&client, embed_images, entry));
    let entries = future::try_join_all(entry_futures).await?;
    feed.set_entries(entries);
    Ok(feed)
}

fn run() -> Result<()> {
    let args = Args::from_args();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let feed = create_feed(args.url, args.embed).await?;
        if let Some(path) = args.output {
            let file = File::create(path)?;
            feed.write_to(file)?;
        } else {
            println!("{}", feed.to_string())
        }
        Ok(())
    })
}

fn main() {
    if let Err(err) = run() {
        eprintln!("dilbert-feed: {}", err);
        std::process::exit(1);
    }
}
