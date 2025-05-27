use anyhow::Context;
use scraper::Html;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::html;
use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(agent: &Agent, url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    // Akkoma implements the Mastodon API with some differences.
    let is_akkoma = html::select_single_element(tree, "noscript")
        .map(|e| e.inner_html().contains("Akkoma"))
        == Some(true);

    let is_mastodon = html::select_single_element(tree, "div#mastodon").is_some();

    // Iceshrimp implements the Mastodon API.
    let is_iceshrimp = html::select_single_element(tree, "meta[name=\"application-name\"]")
        .and_then(|e| e.attr("content"))
        == Some("Iceshrimp");

    // Pleroma implements the Mastodon API with some differences.
    let is_pleroma = html::select_single_element(tree, "noscript")
        .map(|e| e.inner_html().contains("Pleroma"))
        == Some(true);

    // Sharkey implements the Mastodon API.
    let is_sharkey = html::select_single_element(tree, "meta[name=\"application-name\"]")
        .and_then(|e| e.attr("content"))
        == Some("Sharkey");

    if !(is_akkoma || is_iceshrimp || is_mastodon || is_pleroma || is_sharkey) {
        return None;
    }

    Some((|| {
        let post_id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .context("Mastodon URL without post id")?;
        let api_base = url.join("/api/v1/statuses/")?;
        let status: Status = agent
            .get(api_base.join(post_id)?.as_str())
            .call()?
            .body_mut()
            .read_json()?;
        let context: StatusContext = agent
            .get(api_base.join(&format!("{post_id}/context"))?.as_str())
            .call()?
            .body_mut()
            .read_json()?;

        Ok(Content::Text(TextType::PostThread(PostThread {
            title: None,
            before: context
                .ancestors
                .into_iter()
                .map(|s| s.render(url))
                .collect(),
            main: status.render(url),
            after: context
                .descendants
                .into_iter()
                .map(|s| s.render(url))
                .collect(),
        })))
    })())
}

#[derive(Debug, Deserialize)]
struct Status {
    content: String,
    account: Account,
    media_attachments: Vec<MediaAttachment>,
}

impl Status {
    fn render(self, url: &Url) -> Post {
        Post {
            author: self.account.display_name,
            body: html::render(&self.content, url),
            urls: self.media_attachments.into_iter().map(|a| a.url).collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Account {
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct MediaAttachment {
    url: String,
}

#[derive(Debug, Deserialize)]
struct StatusContext {
    ancestors: Vec<Status>,
    descendants: Vec<Status>,
}
