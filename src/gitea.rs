use anyhow::bail;
use anyhow::Context;
use base64::Engine;
use scraper::Html;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::read_raw_response;
use crate::select_single_element;
use crate::Content;
use crate::TextType;

#[derive(Debug, PartialEq)]
enum Path<'a> {
    Commit(&'a str, &'a str, &'a str),
    Src(&'a str, &'a str, &'a str, &'a str),
}

fn parse_path(url: &Url) -> Option<Path<'_>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    Some(
        if path_segments.len() == 4 && path_segments[2] == "commit" {
            Path::Commit(path_segments[0], path_segments[1], path_segments[3])
        } else if path_segments.len() >= 6 && path_segments[2] == "src" {
            Path::Src(
                path_segments[0],
                path_segments[1],
                url.path()
                    .split_at(
                        url.path()
                            .match_indices('/')
                            .nth(5)
                            .expect("path_segments len checked above")
                            .0,
                    )
                    .1,
                path_segments[4],
            )
        } else {
            return None;
        },
    )
}

pub(crate) fn process(agent: &Agent, url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    if select_single_element(tree, "meta[name=\"keywords\"]")
        .and_then(|e| e.attr("content"))
        .map(|c| c.split(',').any(|t| t == "forgejo" || t == "gitea"))
        != Some(true)
    {
        return None;
    }

    Some((|| {
        let path = parse_path(url).context("Unknown Gitea URL")?;
        let api_base = url.join("/api/v1/").expect("URL is valid");

        match path {
            Path::Commit(owner, repo, sha) => {
                let response = agent
                    .request_url(
                        "GET",
                        &api_base
                            .join(&format!("repos/{owner}/{repo}/git/commits/{sha}.patch"))
                            .expect("URL is valid"),
                    )
                    .call()?;
                Ok(Content::Text(TextType::Raw(read_raw_response(response)?)))
            }
            Path::Src(owner, repo, filepath, r#ref) => {
                let content: ContentsResponse = agent
                    .request_url(
                        "GET",
                        &api_base
                            .join(&format!("repos/{owner}/{repo}/contents{filepath}"))
                            .expect("URL is valid"),
                    )
                    .query("ref", r#ref)
                    .call()?
                    .into_json()?;
                if content.r#type == "file" {
                    Ok(Content::Text(TextType::Raw(
                        base64::engine::general_purpose::STANDARD.decode(content.content)?,
                    )))
                } else {
                    bail!("Unknown Gitea content type: {}", content.r#type);
                }
            }
        }
    })())
}

#[derive(Debug, Deserialize)]
struct ContentsResponse {
    content: String,
    r#type: String,
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::parse_path;
    use super::Path;

    macro_rules! parse_path_tests {
        ($(($name: ident, $path: expr, $expected: pat),)*) => {
            $(
                #[test]
                fn $name() {
                    assert!($path.starts_with('/'));
                    let url = Url::parse(&format!("https://example.com{}", $path)).unwrap();
                    assert!(matches!(parse_path(&url), $expected));
                }
            )*
        }
    }

    parse_path_tests!(
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
            src,
            "/foo/bar/src/branch/ref/some/path",
            Some(Path::Src("foo", "bar", "/some/path", "ref"))
        ),
        (unknown, "/invalid", None),
    );
}
