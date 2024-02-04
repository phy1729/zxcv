use anyhow::bail;
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
    if html::select_single_element(tree, "meta[name=\"generator\"]")
        .and_then(|e| e.attr("content"))
        .map(|c| c.starts_with("Discourse "))
        != Some(true)
    {
        return None;
    }

    Some((|| {
        let path_segments: Vec<_> = url
            .path_segments()
            .unwrap_or_else(|| "".split('/'))
            .collect();

        if path_segments.len() == 3 && path_segments[0] == "t" {
            let mut topic: Topic = agent
                .request_url(
                    "GET",
                    &url.join(&format!("/t/{}.json", path_segments[2]))
                        .expect("URL is valid"),
                )
                .call()?
                .into_json()?;

            Ok(Content::Text(TextType::PostThread(PostThread {
                before: vec![],
                main: topic.post_stream.posts.remove(0).render(url),
                after: topic
                    .post_stream
                    .posts
                    .into_iter()
                    .map(|p| p.render(url))
                    .collect(),
            })))
        } else {
            bail!("Unknown discourse URL");
        }
    })())
}

#[derive(Debug, Deserialize)]
struct DiscoursePost {
    cooked: String,
    username: String,
}

impl DiscoursePost {
    fn render(self, url: &Url) -> Post {
        Post {
            author: self.username,
            body: html::render(&self.cooked, url),
            urls: vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
struct PostStream {
    posts: Vec<DiscoursePost>,
}

#[derive(Debug, Deserialize)]
struct Topic {
    post_stream: PostStream,
}
