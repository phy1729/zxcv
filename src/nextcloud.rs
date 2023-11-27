use anyhow::bail;
use anyhow::Context;
use scraper::Html;
use url::Url;

use crate::process_generic;
use crate::select_single_element;
use crate::Content;

pub(crate) fn process(_: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    if select_single_element(tree, "meta[name=\"apple-itunes-app\"]")
        .and_then(|e| e.attr("content"))
        != Some("app-id=1125420102")
    {
        return None;
    }

    Some((|| {
        let Some(download_input) = select_single_element(tree, "input#downloadURL") else {
            bail!("Nextcloud page without downloadURL input");
        };
        process_generic(&Url::parse(
            download_input
                .value()
                .attr("value")
                .context("downloadURL input missing value")?,
        )?)
    })())
}
