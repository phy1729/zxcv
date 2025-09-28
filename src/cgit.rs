use scraper::Html;
use scraper::Selector;
use ureq::Agent;
use url::Url;

use crate::Content;
use crate::html;
use crate::process_generic;

pub(crate) fn try_process(
    agent: &Agent,
    url: &Url,
    tree: &Html,
) -> Option<anyhow::Result<Content>> {
    if !html::select_single_element(tree, "meta[name=\"generator\"]")
        .and_then(|e| e.attr("content"))
        .is_some_and(|c| c.starts_with("cgit "))
    {
        return None;
    }

    let selector = Selector::parse("table.tabs a").expect("valid selector");
    let summary_links: Vec<_> = tree
        .select(&selector)
        .filter(|e| e.inner_html() == "summary")
        .collect();
    let Ok([summary_link]): Result<[_; 1], _> = summary_links.try_into() else {
        return None;
    };
    let repo_path = summary_link
        .attr("href")
        .expect("a element has href attribute");

    let path_segments: Vec<_> = url.path().strip_prefix(repo_path)?.split('/').collect();

    if path_segments.len() >= 2 && path_segments[0] == "tree" {
        let url = url
            .join(&format!(
                "{}/plain/{}",
                repo_path,
                path_segments[1..].join("/")
            ))
            .expect("URL is valid");
        Some(process_generic(agent, &url))
    } else {
        None
    }
}
