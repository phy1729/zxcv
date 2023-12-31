use anyhow::Context;
use scraper::Html;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::render_html_text;
use crate::select_single_element;
use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(agent: &Agent, url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    // Akkoma implements the Mastodon API with some differences.
    let is_akkoma = select_single_element(tree, "noscript")
        .map(|e| e.inner_html().contains("Akkoma"))
        == Some(true);

    let is_mastodon = select_single_element(tree, "div#mastodon").is_some();

    // Sharkey implements the Mastodon API.
    let is_sharkey = select_single_element(tree, "meta[name=\"application-name\"]")
        .and_then(|e| e.attr("content"))
        == Some("Sharkey");

    if !(is_akkoma || is_mastodon || is_sharkey) {
        return None;
    }

    Some((|| {
        let post_id = url
            .path_segments()
            .and_then(|mut s| s.nth(1))
            .context("Mastodon URL without post id")?;
        let api_base = url.join("/api/v1/statuses/")?;
        let status: Status = agent
            .request_url("GET", &api_base.join(post_id)?)
            .call()?
            .into_json()?;
        let context: StatusContext = agent
            .request_url("GET", &api_base.join(&format!("{post_id}/context"))?)
            .call()?
            .into_json()?;

        Ok(Content::Text(TextType::PostThread(PostThread {
            before: context.ancestors.into_iter().map(Into::into).collect(),
            main: status.into(),
            after: context.descendants.into_iter().map(Into::into).collect(),
        })))
    })())
}

#[derive(Debug, Deserialize)]
struct Status {
    content: String,
    account: Account,
    media_attachments: Vec<MediaAttachment>,
}

impl From<Status> for Post {
    fn from(status: Status) -> Self {
        Self {
            author: status.account.display_name,
            body: render_html_text(&status.content),
            urls: status
                .media_attachments
                .into_iter()
                .map(|a| a.url)
                .collect(),
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
