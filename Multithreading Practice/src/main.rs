extern crate reqwest;
extern crate select;
#[macro_use]
extern crate error_chain;

use std::sync::{Arc, Mutex};
use std::{thread, time::Duration};
use select::document::Document;
use select::predicate::Name;

error_chain! {
   foreign_links {
       ReqError(reqwest::Error);
       IoError(std::io::Error);
   }
}

struct Article {
    url: String,
    len: usize,
}

const BATCH_SIZE: usize = 60;

fn main() -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let body = client.get("https://www.baidu.com")
        .timeout(Duration::from_secs(10))
        .send()?
        .text()?;

    let links = Document::from_read(body.as_bytes())?
        .find(Name("a"))
        .filter_map(|n| {
            if let Some(link_str) = n.attr("href") {
                if link_str.starts_with("http") {
                    Some(link_str.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        }).collect::<Vec<String>>();

    let longest_article = Arc::new(Mutex::new(Article {url: "".to_string(), len: 0}));
    let num_batches = links.len() / BATCH_SIZE;
    println!("num_batches: {}", num_batches);

    for batch_idx in 0..num_batches {
        println!("batch_idx: {}", batch_idx);
        let mut reqwesters = Vec::new();
        let start = batch_idx * BATCH_SIZE;
        let end = std::cmp::min((batch_idx + 1) * BATCH_SIZE, links.len());

        for link in &links[start..end] {
            let longest_article_clone = longest_article.clone();
            let link_clone = link.clone();
            let client_clone = client.clone();

            reqwesters.push(thread::spawn(move || {
                let body = client_clone.get(&link_clone)
                    .timeout(Duration::from_secs(10))
                    .send()
                    .and_then(|res| res.text())
                    .unwrap_or_else(|_| "".to_string());

                let curr_len = body.len();
                let mut longest_article_ref = longest_article_clone.lock().unwrap();
                if curr_len > longest_article_ref.len {
                    longest_article_ref.len = curr_len;
                    longest_article_ref.url = link_clone.to_string();
                }
            }));
        }

        for handle in reqwesters {
            handle.join().expect("Panic occurred in thread!");
        }
    }

    let longest_article_ref = longest_article.lock().unwrap();
    println!("{} was the longest article with length {}", longest_article_ref.url, longest_article_ref.len);
    Ok(())
}