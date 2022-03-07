use anyhow::Result;
use reqwest::blocking::ClientBuilder;
use select::{
    document::Document,
    predicate::{Attr, Class, Name},
};
use url::Url;

const NITTER_API_ENDPOINT_BASE: &str = "https://nitter.net/";

fn parse_html() -> Result<()> {
    let api = Url::parse(NITTER_API_ENDPOINT_BASE)?;
    let base_url = Url::options().base_url(Some(&api));

    let mut endpoint = base_url.parse("occultb0t")?;
    let res = reqwest::blocking::get(endpoint.clone())?.text()?;
    let doc = Document::from(res.as_str());

    // Find all timelines.
    doc.find(Class("timeline-item")).for_each(|node| {
        // Inside a timeline.
        let (link, date) = node
            .find(Class("tweet-date"))
            .map(|node| {
                node.find(Name("a"))
                    .map(|node| (node.attr("href").unwrap(), node.attr("title").unwrap()))
                    .next()
                    .unwrap()
            })
            .next()
            .unwrap();

        println!("link: {}", base_url.parse(link).unwrap().as_str());
        println!("date: {}", date);

        let content = node
            .find(Class("tweet-content"))
            .map(|node| node.inner_html())
            .next()
            .unwrap();
        if !content.is_empty() {
            println!("content: {}", content);
        }

        let quote_link = node
            .find(Class("quote-link"))
            .map(|node| node.attr("href").unwrap())
            .next();
        if let Some(quote_link) = quote_link {
            println!(
                "quote link: {}",
                base_url.parse(quote_link).unwrap().as_str()
            );
        }

        let image_attachments: Vec<&str> = node
            .find(Class("still-image"))
            .map(|node| node.attr("href").unwrap())
            .collect();
        for image_attachment in image_attachments {
            println!(
                "image attachment: {}",
                base_url.parse(image_attachment).unwrap().as_str()
            );
        }

        let gif_attachment = node
            .find(Name("source"))
            .map(|node| node.attr("src").unwrap())
            .next();
        if let Some(gif_attachment) = gif_attachment {
            println!(
                "gif attachment: {}",
                base_url.parse(gif_attachment).unwrap().as_str()
            );
        }

        println!();
    });

    doc.find(Class("timeline-item"));

    // The html doc normally contains two pagination cursors, one points to previous,
    // the other points to next. We only want the next so we choose 'last' of the iterator.
    let next_page = doc.find(Class("show-more")).last().map(|node| {
        node.find(Name("a"))
            .next()
            .unwrap()
            .attr("href")
            .unwrap()
            .strip_prefix('?')
            .unwrap()
    });
    if next_page.is_some() {
        endpoint.set_query(next_page);
        println!("next page: {}", endpoint.as_str());
    }

    Ok(())
}

fn loop_pages() -> Result<()> {
    let client = ClientBuilder::new().build()?;
    let api = Url::parse(NITTER_API_ENDPOINT_BASE)?;
    let base_url = Url::options().base_url(Some(&api));

    let mut endpoint = base_url.parse("occultb0t")?;
    let res = client.get(endpoint.clone()).send()?.text()?;
    let doc = Document::from(res.as_str());
    // The html doc normally contains two pagination cursors, one points to previous,
    // the other points to next. We only want the next so we choose 'last' of the iterator.
    let mut next_page = doc.find(Class("show-more")).last().map(|node| {
        node.find(Name("a"))
            .next()
            .unwrap()
            .attr("href")
            .unwrap()
            .strip_prefix('?')
            .unwrap()
            .to_owned()
    });
    let mut pages = 0;

    while next_page.is_some() {
        pages += 1;
        println!("pages: {}", pages);

        endpoint.set_query(next_page.as_deref());
        println!("next page: {}", endpoint.as_str());

        let res = client.get(endpoint.clone()).send()?.text()?;
        let doc = Document::from(res.as_str());
        println!(
            "show more doc: {}",
            doc.find(Class("show-more")).last().unwrap().html().as_str()
        );
        let page = doc.find(Attr("class", "show-more")).last().map(|node| {
            node.find(Name("a"))
                .next()
                .unwrap()
                .attr("href")
                .unwrap()
                .strip_prefix('?')
                .unwrap()
                .to_owned()
        });
        next_page = page.clone();

        if page.is_none() {
            println!("final page: {}", res.as_str());
        }
    }
    println!("total pages: {}", pages);

    Ok(())
}

fn main() {
    parse_html().unwrap();

    loop_pages().unwrap();
}
