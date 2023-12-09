use anyhow::bail;
use anyhow::Context;
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
        let repo_path = select_single_element(tree, "table.tabs a:first-child")
            .context("cgit page missing summary link")?
            .attr("href")
            .expect("a element has href attribute");

        let path_segments: Vec<_> = url
            .path()
            .strip_prefix(repo_path)
            .context("cgit URL path does not start with repo path")?
            .split('/')
            .collect();

        if path_segments.len() >= 2 && path_segments[0] == "tree" {
            let url = url.join(&format!(
                "{}/plain/{}",
                repo_path,
                path_segments[1..].join("/")
            ))?;
            process_generic(&url)
        } else {
            bail!("Unknown cgit URL");
        }
    })())
}
