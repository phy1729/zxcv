use serde::de::DeserializeOwned;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::process_generic;
use crate::read_raw_response;
use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

const API_BASE: &str = "https://api.github.com";

#[derive(Debug, PartialEq)]
enum Path<'a> {
    Blob(&'a str, &'a str, &'a str, &'a str),
    Commit(&'a str, &'a str, &'a str),
    Compare(&'a str, &'a str, &'a str),
    Issue(&'a str, &'a str, &'a str),
    PullRequest(&'a str, &'a str, &'a str),
    Raw(&'a Url),
    Release(&'a str, &'a str, &'a str),
    Repo(&'a str, &'a str),
}

fn parse_path(url: &Url) -> Option<Path<'_>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    Some(if path_segments.len() == 2 {
        Path::Repo(path_segments[0], path_segments[1])
    } else if path_segments.len() >= 4 && path_segments[2] == "assets" {
        Path::Raw(url)
    } else if path_segments.len() >= 5 && path_segments[2] == "blob" {
        Path::Blob(
            path_segments[0],
            path_segments[1],
            url.path()
                .split_at(
                    url.path()
                        .match_indices('/')
                        .nth(4)
                        .expect("path_segments len checked above")
                        .0,
                )
                .1,
            path_segments[3],
        )
    } else if path_segments.len() == 4 && path_segments[2] == "commit" {
        Path::Commit(
            path_segments[0],
            path_segments[1],
            path_segments[3]
                .split_once('.')
                .map_or(path_segments[3], |(c, _)| c),
        )
    } else if path_segments.len() == 4 && path_segments[2] == "compare" {
        let mut basehead = path_segments[3];
        if let Some((extention_index, _)) = basehead.match_indices('.').nth(3) {
            basehead = &basehead[..extention_index];
        }
        Path::Compare(path_segments[0], path_segments[1], basehead)
    } else if path_segments.len() == 4 && path_segments[2] == "issues" {
        Path::Issue(path_segments[0], path_segments[1], path_segments[3])
    } else if path_segments.len() == 4 && path_segments[2] == "pull" {
        if path_segments[3].contains('.') {
            Path::Raw(url)
        } else {
            Path::PullRequest(path_segments[0], path_segments[1], path_segments[3])
        }
    } else if path_segments.len() >= 5 && path_segments[2] == "raw" {
        Path::Raw(url)
    } else if path_segments.len() == 5
        && path_segments[2] == "releases"
        && path_segments[3] == "tag"
    {
        Path::Release(path_segments[0], path_segments[1], path_segments[4])
    } else {
        return None;
    })
}

pub(crate) fn process(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    let path = parse_path(url)?;

    Some((|| match path {
        Path::Blob(owner, repo_name, filepath, r#ref) => process_generic(
            agent,
            &Url::parse(&format!(
                "https://raw.github.com/{owner}/{repo_name}/{ref}{filepath}"
            ))
            .expect("URL is valid"),
        ),
        Path::Commit(owner, repo_name, commit_hash) => process_generic(
            agent,
            &Url::parse(&format!(
                "https://github.com/{owner}/{repo_name}/commit/{commit_hash}.patch"
            ))
            .expect("URL is valid"),
        ),
        Path::Compare(owner, repo_name, basehead) => process_generic(
            agent,
            &Url::parse(&format!(
                "https://github.com/{owner}/{repo_name}/compare/{basehead}.patch"
            ))
            .expect("URL is valid"),
        ),
        Path::Issue(owner, repo_name, issue_id) => {
            let issue: Issue = request(
                agent,
                &format!("{API_BASE}/repos/{owner}/{repo_name}/issues/{issue_id}"),
            )?;
            let comments: Vec<Comment> = request(agent, &issue.comments_url)?;

            Ok(Content::Text(TextType::PostThread(PostThread {
                title: Some(issue.title),
                before: vec![],
                main: Post {
                    author: issue.user.login,
                    body: issue.body,
                    urls: vec![],
                },
                after: comments.into_iter().map(Into::into).collect(),
            })))
        }
        Path::PullRequest(owner, repo_name, pr_id) => {
            let pull_request: PullRequest = request(
                agent,
                &format!("{API_BASE}/repos/{owner}/{repo_name}/pulls/{pr_id}"),
            )?;
            let mut comments: Vec<Comment> = request(agent, &pull_request.comments_url)?;
            let review_comments: Vec<Comment> = request(agent, &pull_request.review_comments_url)?;
            comments.extend(review_comments);
            comments.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            Ok(Content::Text(TextType::PostThread(PostThread {
                title: Some(pull_request.title),
                before: vec![],
                main: Post {
                    author: pull_request.user.login,
                    body: pull_request.body.unwrap_or_default(),
                    urls: vec![pull_request.patch_url],
                },
                after: comments.into_iter().map(Into::into).collect(),
            })))
        }
        Path::Raw(url) => process_generic(agent, url),
        Path::Release(owner, repo_name, tag) => {
            let release: Release = request(
                agent,
                &format!("{API_BASE}/repos/{owner}/{repo_name}/releases/tags/{tag}"),
            )?;
            Ok(Content::Text(TextType::Post(Post {
                author: release.author.login,
                body: release.body,
                urls: vec![release.tarball_url],
            })))
        }
        Path::Repo(owner, repo_name) => {
            let readme = request_raw(
                agent,
                &format!("{API_BASE}/repos/{owner}/{repo_name}/readme"),
            )?;
            Ok(Content::Text(TextType::Raw(readme)))
        }
    })())
}

fn request<T: DeserializeOwned>(agent: &Agent, url: &str) -> anyhow::Result<T> {
    Ok(agent
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()?
        .body_mut()
        .read_json()?)
}

fn request_raw(agent: &Agent, url: &str) -> anyhow::Result<Vec<u8>> {
    let response = agent
        .get(url)
        .header("Accept", "application/vnd.github.raw")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .call()?;
    Ok(read_raw_response(response)?)
}

#[derive(Debug, Deserialize)]
struct Comment {
    body: String,
    created_at: String, // ISO timestamp
    user: User,
}

impl From<Comment> for Post {
    fn from(comment: Comment) -> Self {
        Self {
            author: comment.user.login,
            body: comment.body,
            urls: vec![],
        }
    }
}

#[derive(Debug, Deserialize)]
struct Issue {
    body: String,
    comments_url: String,
    title: String,
    user: User,
}

#[derive(Debug, Deserialize)]
struct PullRequest {
    body: Option<String>,
    comments_url: String,
    patch_url: String,
    review_comments_url: String,
    title: String,
    user: User,
}

#[derive(Debug, Deserialize)]
struct Release {
    author: User,
    body: String,
    tarball_url: String,
}

#[derive(Debug, Deserialize)]
struct User {
    login: String,
}

#[cfg(test)]
mod tests {
    use super::Path;
    use crate::tests::parse_path_tests;

    parse_path_tests!(
        super::parse_path,
        "https://github.com{}",
        (
            assets,
            "/foo/bar/assets/1729/06c106c1-06c1-46c1-06c1-06c106c106c1",
            Some(Path::Raw(_))
        ),
        (
            blob,
            "/foo/bar/blob/ref/some/path",
            Some(Path::Blob("foo", "bar", "/some/path", "ref"))
        ),
        (
            commit,
            "/foo/bar/commit/06c106c106c106c106c106c106c106c106c106c1",
            Some(Path::Commit(
                "foo",
                "bar",
                "06c106c106c106c106c106c106c106c106c106c1"
            ))
        ),
        (
            commit_patch,
            "/foo/bar/commit/06c106c106c106c106c106c106c106c106c106c1.patch",
            Some(Path::Commit(
                "foo",
                "bar",
                "06c106c106c106c106c106c106c106c106c106c1"
            ))
        ),
        (
            compare,
            "/foo/bar/compare/06c106c106c1...c106c106c106",
            Some(Path::Compare("foo", "bar", "06c106c106c1...c106c106c106"))
        ),
        (
            compare_patch,
            "/foo/bar/compare/06c106c106c1...c106c106c106.patch",
            Some(Path::Compare("foo", "bar", "06c106c106c1...c106c106c106"))
        ),
        (
            issue,
            "/foo/bar/issues/1729",
            Some(Path::Issue("foo", "bar", "1729"))
        ),
        (
            pull_request,
            "/foo/bar/pull/1729",
            Some(Path::PullRequest("foo", "bar", "1729"))
        ),
        (
            pull_request_patch,
            "/foo/bar/pull/1729.patch",
            Some(Path::Raw(_))
        ),
        (raw, "/foo/bar/raw/ref/some/path", Some(Path::Raw(_))),
        (
            release,
            "/foo/bar/releases/tag/v1.72.9",
            Some(Path::Release("foo", "bar", "v1.72.9"))
        ),
        (repo, "/foo/bar", Some(Path::Repo("foo", "bar"))),
        (unknown, "/invalid", None),
    );
}

pub(crate) mod gist {
    use std::collections::HashMap;

    use serde::Deserialize;
    use ureq::Agent;
    use url::Url;

    use crate::Collection;
    use crate::Content;
    use crate::Item;
    use crate::TextType;

    pub(crate) fn process(agent: &Agent, url: &Url) -> Option<anyhow::Result<Content>> {
        let gist_id = url.path_segments().and_then(|mut p| p.nth(1))?;
        Some(process_by_id(agent, gist_id))
    }

    pub(crate) fn process_by_id(agent: &Agent, gist_id: &str) -> anyhow::Result<Content> {
        let gist: Gist = super::request(agent, &format!("{}/gists/{gist_id}", super::API_BASE))?;
        if gist.files.len() == 1 {
            let file = gist.files.into_values().next().expect("Checked above");
            Ok(Content::Text(TextType::Raw(file.content.into())))
        } else {
            Ok(Content::Collection(Collection {
                title: None,
                description: Some(gist.description),
                items: gist
                    .files
                    .into_values()
                    .map(|file| Item {
                        title: Some(file.filename),
                        description: None,
                        url: file.raw_url,
                    })
                    .collect(),
            }))
        }
    }

    #[derive(Debug, Deserialize)]
    struct Gist {
        description: String,
        files: HashMap<String, File>,
    }

    #[derive(Debug, Deserialize)]
    struct File {
        content: String,
        filename: String,
        raw_url: String,
    }
}
