use anyhow::bail;
use scraper::Html;
use url::Url;

use crate::process_generic;
use crate::select_single_element;
use crate::Content;

pub(crate) fn process(url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    if select_single_element(tree, "meta[name=\"generator\"]")
        .and_then(|e| e.attr("content"))
        .map(|c| c.starts_with("cgit "))
        != Some(true)
    {
        return None;
    }

    Some((|| {
        let path_segments: Vec<_> = url
            .path_segments()
            .unwrap_or_else(|| "".split('/'))
            .collect();

        let Some(repo_index) = path_segments.iter().position(|s| s.ends_with(".git")) else {
            bail!("cgit URL missing repository");
        };

        if path_segments.len() > repo_index + 2 && path_segments[repo_index + 1] == "tree" {
            let url = url.join(&format!(
                "/{}/plain/{}",
                path_segments[0..=repo_index].join("/"),
                path_segments[repo_index + 2..].join("/")
            ))?;
            process_generic(&url)
        } else {
            bail!("Unknown cgit URL");
        }
    })())
}
