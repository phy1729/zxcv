use scraper::Html;
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
        .is_some_and(|c| c.starts_with("gitweb/"))
    {
        return None;
    }

    if url.query()?.split(';').any(|p| p == "a=blob") {
        let query = url.query()?.replace(";a=blob;", ";a=blob_plain;");
        let mut url = url.clone();
        url.set_query(Some(&query));
        Some(process_generic(agent, &url))
    } else {
        None
    }
}
