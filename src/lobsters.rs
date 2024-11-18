use std::fmt::Write;

use anyhow::bail;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(agent: &Agent, url: &mut Url) -> anyhow::Result<Content> {
    if !url.path().starts_with("/s/") {
        bail!("Unknown lobsters URL");
    }

    (|| {
        if !url.path().ends_with(".json") {
            url.path_segments_mut()
                .expect("cannot_be_a_base is checked earlier")
                .pop_if_empty();
            url.set_path(&(url.path().to_owned() + ".json"));
        }

        let story: Story = agent.request_url("GET", url).call()?.into_json()?;
        let mut body = story.title.clone();
        if !story.description_plain.is_empty() {
            write!(body, "\n{}", story.description_plain).expect("write! to String cannot fail");
        }

        Ok(Content::Text(TextType::PostThread(PostThread {
            before: vec![],
            main: Post {
                author: story.submitter_user,
                body,
                urls: vec![story.url],
            },
            after: story
                .comments
                .into_iter()
                .map(|c| Post {
                    author: c.commenting_user,
                    body: c.comment_plain,
                    urls: vec![],
                })
                .collect(),
        })))
    })()
}

#[derive(Debug, Deserialize)]
struct Story {
    comments: Vec<Comment>,
    description_plain: String,
    submitter_user: String,
    title: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Comment {
    comment_plain: String,
    commenting_user: String,
}
