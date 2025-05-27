use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

pub(crate) fn process(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    if !url.path().starts_with("/s/") {
        return None;
    }

    Some((|| {
        if !url.path().ends_with(".json") {
            url.path_segments_mut()
                .expect("cannot_be_a_base is checked earlier")
                .pop_if_empty();
            url.set_path(&(url.path().to_owned() + ".json"));
        }

        let story: Story = agent.get(url.as_str()).call()?.body_mut().read_json()?;

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
