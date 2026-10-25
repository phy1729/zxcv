use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    if !(path_segments.len() >= 2 && path_segments[0] == "s") {
        return None;
    }
    let story_id = path_segments[1];

    Some((|| {
        let story: Story = agent
            .get(
                url.join(&format!("/s/{story_id}.json"))
                    .expect("URL is valid")
                    .as_str(),
            )
            .call()?
            .body_mut()
            .read_json()?;

        Ok(Content::Text(TextType::PostThread(PostThread {
            title: Some(story.title),
            before: vec![],
            main: Post {
                author: story.submitter_user,
                body: story.description_plain,
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
    })())
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
