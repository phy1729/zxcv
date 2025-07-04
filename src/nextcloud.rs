use anyhow::Context;
use base64::Engine;
use scraper::Html;
use ureq::Agent;
use url::Url;

use crate::html;
use crate::process_generic;
use crate::Content;

pub(crate) fn try_process(
    agent: &Agent,
    url: &Url,
    tree: &Html,
) -> Option<anyhow::Result<Content>> {
    if html::select_single_element(tree, "meta[name=\"apple-itunes-app\"]")
        .and_then(|e| e.attr("content"))
        != Some("app-id=1125420102")
    {
        return None;
    }

    if let Some(encoded_token) =
        html::select_single_element(tree, "input#initial-state-files_sharing-sharingToken")
    {
        Some((|| {
            let token: String = encoded_token
                .value()
                .attr("value")
                .context("Missing sharingToken value")
                .and_then(|v| Ok(base64::engine::general_purpose::STANDARD.decode(v)?))
                .and_then(|v| Ok(String::from_utf8(v)?))
                .and_then(|v| Ok(serde_json::from_str(&v)?))
                .context("Invalid sharingToken")?;
            let url = url.join("/public.php/dav/files/")?.join(&token)?;
            process_generic(agent, &url)
        })())
    } else {
        html::select_single_element(tree, "input#downloadURL").map(|download_input| {
            process_generic(
                agent,
                &Url::parse(
                    download_input
                        .value()
                        .attr("value")
                        .context("downloadURL input missing value")?,
                )?,
            )
        })
    }
}
