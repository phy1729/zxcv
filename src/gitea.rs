use anyhow::bail;
use base64::Engine;
use scraper::Html;
use serde::Deserialize;
use url::Url;

use crate::select_single_element;
use crate::Content;
use crate::TextType;

pub(crate) fn process(url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    if select_single_element(tree, "meta[name=\"keywords\"]").and_then(|e| e.attr("content"))
        != Some("go,git,self-hosted,gitea")
    {
        return None;
    }

    Some((|| {
        let path_segments: Vec<_> = url
            .path_segments()
            .unwrap_or_else(|| "".split('/'))
            .collect();
        let api_base = url.join("/api/v1/")?;

        if path_segments.len() >= 6 && path_segments[2] == "src" {
            let path = path_segments[5..].join("/");
            let content: ContentsResponse = ureq::get(
                api_base
                    .join(&format!(
                        "repos/{}/{}/contents/{path}",
                        path_segments[0], path_segments[1]
                    ))?
                    .as_str(),
            )
            .query("ref", path_segments[4])
            .call()?
            .into_json()?;
            if content.r#type == "file" {
                Ok(Content::Text(TextType::Raw(
                    base64::engine::general_purpose::STANDARD.decode(content.content)?,
                )))
            } else {
                bail!("Unknown Gitea content type: {}", content.r#type);
            }
        } else {
            bail!("Unknown Gitea URL");
        }
    })())
}

#[derive(Debug, Deserialize)]
struct ContentsResponse {
    content: String,
    r#type: String,
}
