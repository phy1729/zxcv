use anyhow::bail;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use url::Url;

use crate::process_generic;
use crate::Content;
use crate::Post;
use crate::PostThread;
use crate::TextType;

const API_BASE: &str = "https://api.github.com";

pub(crate) fn process(url: &mut Url) -> anyhow::Result<Content> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    if path_segments.len() == 4 && path_segments[2] == "commit" {
        if !path_segments[3].contains('.') {
            url.set_path(&(url.path().to_owned() + ".patch"));
        }
        process_generic(url)
    } else if path_segments.len() == 4 && path_segments[2] == "issues" {
        let issue: Issue = request(&format!("{API_BASE}/repos{}", url.path()))?;
        let comments: Vec<Comment> = request(&issue.comments_url)?;

        Ok(Content::Text(TextType::PostThread(PostThread {
            before: vec![],
            main: Post {
                author: issue.user.login,
                body: issue.body,
            },
            after: comments
                .into_iter()
                .map(|c| Post {
                    author: c.user.login,
                    body: c.body,
                })
                .collect(),
        })))
    } else {
        bail!("Unknown GitHub URL");
    }
}

#[allow(clippy::result_large_err)]
fn request<T: DeserializeOwned>(url: &str) -> Result<T, ureq::Error> {
    Ok(ureq::get(url)
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .call()?
        .into_json()?)
}

#[derive(Debug, Deserialize)]
struct Comment {
    body: String,
    user: User,
}

#[derive(Debug, Deserialize)]
struct Issue {
    body: String,
    comments_url: String,
    user: User,
}

#[derive(Debug, Deserialize)]
struct User {
    login: String,
}

pub(crate) mod gist {
    use std::collections::HashMap;

    use anyhow::bail;
    use serde::Deserialize;
    use url::Url;

    use crate::Content;
    use crate::TextType;

    pub(crate) fn process(url: &Url) -> anyhow::Result<Content> {
        let Some(gist_id) = url.path_segments().and_then(|mut p| p.nth(1)) else {
            bail!("Unknown Github Gist URL");
        };
        process_by_id(gist_id)
    }

    pub(crate) fn process_by_id(gist_id: &str) -> anyhow::Result<Content> {
        let gist: Gist = super::request(&format!("{}/gists/{gist_id}", super::API_BASE))?;
        if gist.files.len() != 1 {
            todo!("Handle more than one file in a gist")
        }
        let file = gist.files.into_values().next().expect("Checked above");
        Ok(Content::Text(TextType::Raw(file.content)))
    }

    #[derive(Debug, Deserialize)]
    struct Gist {
        files: HashMap<String, File>,
    }

    #[derive(Debug, Deserialize)]
    struct File {
        content: String,
    }
}
