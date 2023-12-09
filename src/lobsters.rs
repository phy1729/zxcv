use std::fmt::Write;

use anyhow::bail;
use serde::Deserialize;
use url::Url;

use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(url: &mut Url) -> anyhow::Result<Content> {
    if !url.path().starts_with("/s/") {
        bail!("Unknown lobsters URL");
    }

    if !url.path().ends_with(".json") {
        url.set_path(&(url.path().to_owned() + ".json"));
    }

    let story: Story = ureq::get(url.as_str()).call()?.into_json()?;
    let mut body = story.title.clone();
    if !story.description_plain.is_empty() {
        write!(body, "\n{}", story.description_plain).expect("write! to String cannot fail");
    }

    Ok(Content::Text(TextType::PostThread(PostThread {
        before: vec![],
        main: Post {
            author: story.submitter_user.username,
            body,
            urls: vec![story.url],
        },
        after: story
            .comments
            .into_iter()
            .map(|c| Post {
                author: c.commenting_user.username,
                body: c.comment_plain,
                urls: vec![],
            })
            .collect(),
    })))
}

#[derive(Debug, Deserialize)]
struct Story {
    comments: Vec<Comment>,
    description_plain: String,
    submitter_user: User,
    title: String,
    url: String,
}

#[derive(Debug, Deserialize)]
struct Comment {
    comment_plain: String,
    commenting_user: User,
}

#[derive(Debug, Deserialize)]
struct User {
    username: String,
}
