use std::collections::HashMap;

use anyhow::bail;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Article;
use crate::Content;
use crate::TextType;
use crate::LINE_LENGTH;

pub(crate) fn process(agent: &Agent, url: &Url) -> Option<anyhow::Result<Content>> {
    let raw_title = url.path_segments().and_then(|mut s| s.nth(1))?;

    Some((|| {
        let api_url = url.join("/w/api.php")?;
        let title = percent_encoding::percent_decode_str(raw_title).decode_utf8()?;
        let response: Response = agent
            .get(api_url.as_str())
            .query_pairs([
                ("action", "query"),
                ("format", "json"),
                ("titles", &title),
                ("prop", "revisions"),
                ("rvprop", "content"),
                ("rvslots", "main"),
            ])
            .call()?
            .body_mut()
            .read_json()?;

        let mut pages: Vec<_> = response.query.pages.into_values().collect();
        let Some(mut page) = pages.pop() else {
            bail!("Unexpected wikimedia pages value {pages:?}");
        };

        let Some(mut revision) = page.revisions.pop() else {
            bail!("Unexpected wikimedia revisions {:?}", page.revisions);
        };

        if let Some(slot) = revision.slots.remove("main") {
            Ok(Content::Text(TextType::Article(Article {
                title: page.title,
                body: textwrap::fill(&slot.star, LINE_LENGTH),
            })))
        } else {
            bail!(
                "Wikimedia revision lacks main slot. {:?}",
                page.revisions[0].slots
            );
        }
    })())
}

#[derive(Debug, Deserialize)]
struct Response {
    query: ResponseQuery,
}

#[derive(Debug, Deserialize)]
struct ResponseQuery {
    pages: HashMap<String, ResponsePage>,
}

#[derive(Debug, Deserialize)]
struct ResponsePage {
    title: String,
    revisions: Vec<Revision>,
}

#[derive(Debug, Deserialize)]
struct Revision {
    slots: HashMap<String, Slot>,
}

#[derive(Debug, Deserialize)]
struct Slot {
    #[serde(rename = "*")]
    star: String,
}
