use anyhow::bail;
use url::Url;

use crate::process_generic;
use crate::Content;

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
    } else {
        bail!("Unknown GitHub URL");
    }
}

pub(crate) mod gist {
    use anyhow::Context;
    use scraper::Html;
    use scraper::Selector;
    use url::Url;

    use crate::process_generic;
    use crate::Content;

    pub(crate) fn process(url: &Url) -> anyhow::Result<Content> {
        let response = ureq::get(url.as_str()).call()?;
        let tree = Html::parse_document(&response.into_string()?);
        let selector = Selector::parse("a > span > span.Button-label").expect("selector is valid");
        let results: Vec<_> = tree
            .select(&selector)
            .filter(|b| b.inner_html() == "Raw")
            .collect();
        if results.len() != 1 {
            todo!("Handle more than one file in a gist")
        }
        process_generic(
            &url.join(
                results[0]
                    .parent()
                    .expect("selector has parent")
                    .parent()
                    .expect("selector has grandparent")
                    .value()
                    .as_element()
                    .expect("node is <a> in selector")
                    .attr("href")
                    .context("Raw button did not have href attribute")?,
            )?,
        )
    }
}
