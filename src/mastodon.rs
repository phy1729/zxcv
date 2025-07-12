use scraper::Html;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;
use crate::html;

#[derive(Debug, PartialEq)]
enum Path<'a> {
    Status { status_id: &'a str },
}

fn parse_path(url: &Url) -> Option<Path<'_>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    Some(
        if path_segments.len() == 2 && path_segments[0].starts_with('@') {
            Path::Status {
                status_id: path_segments[1],
            }
        } else {
            return None;
        },
    )
}

pub(crate) fn try_process(
    agent: &Agent,
    url: &Url,
    tree: &Html,
) -> Option<anyhow::Result<Content>> {
    // Akkoma implements the Mastodon API with some differences.
    let is_akkoma = html::select_single_element(tree, "noscript")
        .map(|e| e.inner_html().contains("Akkoma"))
        == Some(true);

    // Iceshrimp implements the Mastodon API.
    let is_iceshrimp = html::select_single_element(tree, "meta[name=\"application-name\"]")
        .and_then(|e| e.attr("content"))
        == Some("Iceshrimp");

    let is_mastodon = html::select_single_element(tree, "div#mastodon").is_some();

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

    let path = parse_path(url)?;
    let api_base = url.join("/api/v1/").expect("URL is valid");

    Some((|| match path {
        Path::Status { status_id } => {
            let status: Status = agent
                .get(api_base.join(&format!("statuses/{status_id}"))?.as_str())
                .call()?
                .body_mut()
                .read_json()?;
            let context: StatusContext = agent
                .get(
                    api_base
                        .join(&format!("statuses/{status_id}/context"))?
                        .as_str(),
                )
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
        }
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

#[cfg(test)]
mod tests {
    use super::Path;
    use crate::tests::parse_path_tests;

    parse_path_tests!(
        super::parse_path,
        "https://example.com{}",
        (
            status,
            "/@example/17291729",
            Some(Path::Status {
                status_id: "17291729"
            })
        ),
        (unknown, "/unknown", None),
    );
}
