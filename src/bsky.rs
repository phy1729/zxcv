use anyhow::bail;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::Collection;
use crate::Content;
use crate::Item;
use crate::Post;
use crate::PostThread;
use crate::TextType;

const API_BASE: &str = "https://public.api.bsky.app";

#[derive(Debug, PartialEq)]
enum Path<'a> {
    List { profile: &'a str, list: &'a str },
    Post { profile: &'a str, post: &'a str },
    Profile { profile: &'a str },
}

fn parse_path(url: &Url) -> Option<Path<'_>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    Some(
        if path_segments.len() == 4 && path_segments[0] == "profile" && path_segments[2] == "lists"
        {
            Path::List {
                profile: path_segments[1],
                list: path_segments[3],
            }
        } else if path_segments.len() == 4
            && path_segments[0] == "profile"
            && path_segments[2] == "post"
        {
            Path::Post {
                profile: path_segments[1],
                post: path_segments[3],
            }
        } else if path_segments.len() == 2 && path_segments[0] == "profile" {
            Path::Profile {
                profile: path_segments[1],
            }
        } else {
            return None;
        },
    )
}

pub(crate) fn process(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    let path = parse_path(url)?;

    Some((|| match path {
        Path::List { profile, list } => {
            let profile = get_profile(agent, profile)?;
            let list: GetListResponse = agent
                .get(format!("{API_BASE}/xrpc/app.bsky.graph.getList"))
                .query(
                    "list",
                    format!("at://{}/app.bsky.graph.list/{}", profile.did, list),
                )
                .call()?
                .body_mut()
                .read_json()?;

            Ok(Content::Collection(Collection {
                title: Some(list.list.name),
                description: list.list.description,
                items: list
                    .items
                    .into_iter()
                    .map(|item| Item {
                        url: format!("https://bsky.app/profile/{}", item.subject.handle),
                        title: Some(item.subject.display_name.unwrap_or(item.subject.handle)),
                        description: Some(item.subject.description),
                    })
                    .collect(),
            }))
        }

        Path::Post { profile, post } => {
            let profile = get_profile(agent, profile)?;
            let thread: GetPostThreadResponse = agent
                .get(format!("{API_BASE}/xrpc/app.bsky.feed.getPostThread"))
                .query(
                    "uri",
                    format!("at://{}/app.bsky.feed.post/{}", profile.did, post),
                )
                .call()?
                .body_mut()
                .read_json()?;

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
        }

        Path::Profile { profile } => {
            let profile = get_profile(agent, profile)?;
            let posts: GetAuthorFeedResponse = agent
                .get(format!("{API_BASE}/xrpc/app.bsky.feed.getAuthorFeed"))
                .query("actor", profile.did)
                .call()?
                .body_mut()
                .read_json()?;

            Ok(Content::Text(TextType::PostThread(PostThread {
                before: vec![],
                main: Post {
                    author: profile.display_name.unwrap_or(profile.handle),
                    body: profile.description,
                    urls: vec![],
                },
                after: posts.feed.into_iter().map(|p| p.post.render()).collect(),
            })))
        }
    })())
}

fn get_profile(agent: &Agent, profile: &str) -> anyhow::Result<ProfileView> {
    Ok(agent
        .get(format!("{API_BASE}/xrpc/app.bsky.actor.getProfile"))
        .query("actor", profile)
        .call()?
        .body_mut()
        .read_json()?)
}

#[derive(Debug, Deserialize)]
struct GetAuthorFeedResponse {
    feed: Vec<FeedViewPost>,
}

#[derive(Debug, Deserialize)]
struct GetListResponse {
    list: ListView,
    items: Vec<ListItemView>,
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

// app.bsky.actor.defs#profileView
// app.bsky.actor.defs#profileViewDetailed
#[derive(Debug, Deserialize)]
struct ProfileView {
    did: String,
    handle: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
    description: String,
}

// app.bsky.actor.defs#profileViewBasic
#[derive(Debug, Deserialize)]
struct ProfileViewBasic {
    handle: String,
    #[serde(rename = "displayName")]
    display_name: Option<String>,
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

// app.bsky.embed.video#view
#[derive(Debug, Deserialize)]
struct Video {
    playlist: String,
}

// app.bsky.feed.defs#feedViewPost
#[derive(Debug, Deserialize)]
struct FeedViewPost {
    post: PostView,
}

// app.bsky.feed.defs#postView
#[derive(Debug, Deserialize)]
struct PostView {
    author: ProfileViewBasic,
    record: BskyPost,
    embed: Option<Embed>,
}

impl PostView {
    fn render(self) -> Post {
        let mut urls: Vec<_> = self
            .record
            .facets
            .into_iter()
            .flatten()
            .flat_map(|f| f.features)
            .filter_map(|f| {
                if let FaucetFeature::Link(link) = f {
                    Some(link.uri)
                } else {
                    None
                }
            })
            .collect();
        urls.extend(self.embed.map(Embed::urls).unwrap_or_default());
        Post {
            author: self.author.display_name.unwrap_or(self.author.handle),
            body: self.record.text,
            urls,
        }
    }
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
    #[serde(rename = "app.bsky.embed.video#view")]
    Video(Video),
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
            Self::Video(v) => vec![v.playlist],
        }
    }
}

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
                }
            }
            self.stack.pop();
        }
    }
}

// app.bsky.feed.post
#[derive(Debug, Deserialize)]
struct BskyPost {
    text: String,
    facets: Option<Vec<Facet>>,
}

// app.bsky.graph.defs#listItemView
#[derive(Debug, Deserialize)]
struct ListItemView {
    subject: ProfileView,
}

// app.bsky.graph.defs#listView
#[derive(Debug, Deserialize)]
struct ListView {
    name: String,
    description: Option<String>,
}

// app.bsky.richtext.facet
#[derive(Debug, Deserialize)]
struct Facet {
    features: Vec<FaucetFeature>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "$type")]
enum FaucetFeature {
    #[serde(rename = "app.bsky.richtext.facet#mention")]
    Mention(Ignore),
    #[serde(rename = "app.bsky.richtext.facet#link")]
    Link(FacetLink),
    #[serde(rename = "app.bsky.richtext.facet#tag")]
    Tag(Ignore),
    #[serde(rename = "app.bsky.richtext.facet#byteSlice")]
    ByteSlice(Ignore),
}

// app.bsky.richtext.facet#link
#[derive(Debug, Deserialize)]
struct FacetLink {
    uri: String,
}

#[cfg(test)]
mod tests {
    use super::Path;
    use crate::tests::parse_path_tests;

    parse_path_tests!(
        super::parse_path,
        "https://bsky.app{}",
        (
            list,
            "/profile/example.bsky.social/lists/17296c1",
            Some(Path::List {
                profile: "example.bsky.social",
                list: "17296c1"
            })
        ),
        (
            post,
            "/profile/example.bsky.social/post/17296c1",
            Some(Path::Post {
                profile: "example.bsky.social",
                post: "17296c1"
            })
        ),
        (
            profile,
            "/profile/example.bsky.social",
            Some(Path::Profile {
                profile: "example.bsky.social"
            })
        ),
        (unknown, "/unknown", None),
    );
}
