use anyhow::bail;
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

    if path_segments.len() != 4 || path_segments[0] != "profile" || path_segments[2] != "post" {
        return None;
    }

    Some((|| {
        let profile: Profile = agent
            .get("https://public.api.bsky.app/xrpc/app.bsky.actor.getProfile")
            .query("actor", path_segments[1])
            .call()?
            .into_json()?;

        let thread: GetPostThreadResponse = agent
            .get("https://public.api.bsky.app/xrpc/app.bsky.feed.getPostThread")
            .query(
                "uri",
                &format!(
                    "at://{}/app.bsky.feed.post/{}",
                    profile.did, path_segments[3]
                ),
            )
            .call()?
            .into_json()?;

        let mut thread_view = match thread.thread {
            PostViewEnum::Thread(t) => t,
            PostViewEnum::NotFound(_) => bail!("Post could not be found"),
            PostViewEnum::Blocked(_) => bail!("Post was blocked"),
        };

        let mut parents: Vec<_> = thread_view
            .take_parents()
            .map(|p| p.post.render())
            .collect();
        parents.reverse();

        let replies: Vec<_> = thread_view
            .take_replies()
            .map(|r| r.post.render())
            .collect();

        Ok(Content::Text(TextType::PostThread(PostThread {
            before: parents,
            main: thread_view.post.render(),
            after: replies,
        })))
    })())
}

#[derive(Debug, Deserialize)]
struct Profile {
    did: String,
    handle: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GetPostThreadResponse {
    thread: PostViewEnum,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
enum PostViewEnum {
    #[serde(rename = "app.bsky.feed.defs#threadViewPost")]
    Thread(ThreadViewPost),
    #[serde(rename = "app.bsky.feed.defs#notFoundPost")]
    NotFound(Ignore),
    #[serde(rename = "app.bsky.feed.defs#blockedPost")]
    Blocked(Ignore),
}

#[derive(Debug, Deserialize)]
struct Ignore {}

// app.bsky.feed.defs#threadViewPost
#[derive(Debug, Deserialize)]
struct ThreadViewPost {
    post: PostView,
    replies: Option<Vec<PostViewEnum>>,
    parent: Option<Box<PostViewEnum>>,
}

impl ThreadViewPost {
    fn take_parents(&mut self) -> TakeParents {
        TakeParents {
            next: self.parent.take(),
        }
    }

    fn take_replies(&mut self) -> TakeReplies {
        TakeReplies {
            stack: vec![self.replies.take().unwrap_or_default().into_iter()],
        }
    }
}

struct TakeParents {
    next: Option<Box<PostViewEnum>>,
}

impl Iterator for TakeParents {
    type Item = ThreadViewPost;

    fn next(&mut self) -> Option<Self::Item> {
        let mut item = match *self.next.take()? {
            PostViewEnum::Thread(v) => Some(v),
            PostViewEnum::NotFound(_) | PostViewEnum::Blocked(_) => None,
        }?;
        self.next = item.parent.take();
        Some(item)
    }
}

struct TakeReplies {
    stack: Vec<<Vec<PostViewEnum> as IntoIterator>::IntoIter>,
}

impl Iterator for TakeReplies {
    type Item = ThreadViewPost;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            for item in self.stack.last_mut()? {
                if let PostViewEnum::Thread(mut thread) = item {
                    self.stack
                        .push(thread.replies.take().unwrap_or_default().into_iter());
                    return Some(thread);
                };
            }
            self.stack.pop();
        }
    }
}

// app.bsky.feed.defs#postView
#[derive(Debug, Deserialize)]
struct PostView {
    author: Profile,
    record: BskyPost,
    embed: Option<Embed>,
}

impl PostView {
    fn render(self) -> Post {
        Post {
            author: self.author.display_name.unwrap_or(self.author.handle),
            body: self.record.text,
            urls: self.embed.map(Embed::urls).unwrap_or_default(),
        }
    }
}

// app.bsky.feed.post
#[derive(Debug, Deserialize)]
struct BskyPost {
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
enum Embed {
    #[serde(rename = "app.bsky.embed.images#view")]
    Images(Images),
    #[serde(rename = "app.bsky.embed.external#view")]
    External(External),
    #[serde(rename = "app.bsky.embed.record#view")]
    Record(EmbedRecord),
    #[serde(rename = "app.bsky.embed.recordWithMedia#view")]
    RecordWithMedia(RecordWithMedia),
}

impl Embed {
    fn urls(self) -> Vec<String> {
        match self {
            Self::External(e) => vec![e.external.uri],
            Self::Images(i) => i.images.into_iter().map(|i| i.fullsize).collect(),
            Self::Record(_) => vec![],
            Self::RecordWithMedia(r) => match r.media {
                Media::External(e) => vec![e.external.uri],
                Media::Images(i) => i.images.into_iter().map(|i| i.fullsize).collect(),
            },
        }
    }
}

// app.bsky.embed.external#view
#[derive(Debug, Deserialize)]
struct External {
    external: ViewExternal,
}

// app.bsky.embed.external#viewExternal
#[derive(Debug, Deserialize)]
struct ViewExternal {
    uri: String,
}

// app.bsky.embed.images#view
#[derive(Debug, Deserialize)]
struct Images {
    images: Vec<ViewImage>,
}

// app.bsky.embed.images#viewImage
#[derive(Debug, Deserialize)]
struct ViewImage {
    fullsize: String,
}

// app.bsky.embed.record#view
#[derive(Debug, Deserialize)]
struct EmbedRecord {}

// app.bsky.embed.recordWithMedia#view
#[derive(Debug, Deserialize)]
struct RecordWithMedia {
    media: Media,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
enum Media {
    #[serde(rename = "app.bsky.embed.images#view")]
    Images(Images),
    #[serde(rename = "app.bsky.embed.external#view")]
    External(External),
}
