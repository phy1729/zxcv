//! `zxcv` (z xssential content viewer) is a command for viewing the essential content of a URL.
//!
//! `zxcv` takes the essential content of a web page (e.g. the text of a pastebin link or the video
//! of a youtube link) and runs an appropriate command to display that content locally (e.g.
//! `less`, `mupdf`, or `mpv`).
//!
//! # Configuration
//!
//! A configuration file may be passed via the `-f` flag. The configuration file is in
//! [TOML](https://toml.io) format and the accepted sections are documented at [Config].
#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    clippy::cargo,
    clippy::pedantic,
    clippy::str_to_string,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
    clippy::use_self,
    rustdoc::missing_crate_level_docs,
    rustdoc::unescaped_backticks
)]
#![allow(
    clippy::case_sensitive_file_extension_comparisons,
    clippy::multiple_crate_versions
)]

use std::collections::HashMap;
use std::env;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::io;
use std::io::Read;
use std::io::Write;
use std::iter;
use std::iter::Iterator;
use std::num::NonZeroUsize;
use std::process::Command;

use anyhow::anyhow;
use anyhow::bail;
use scraper::Html;
use tempfile::NamedTempFile;
use ureq::Agent;
use ureq::AgentBuilder;
use url::Url;

mod config;
pub use config::Config;

mod bsky;
mod cgit;
mod discourse;
mod gitea;
mod github;
mod html;
mod lobsters;
mod mastodon;
mod nextcloud;
mod stackoverflow;
mod wikimedia;

const LINE_LENGTH: usize = 80;

enum Content {
    Audio(Url),
    Image(Box<dyn Read>),
    Pdf(Box<dyn Read>),
    Text(TextType),
    Video(Url),
}

enum TextType {
    Article(Article),
    Post(Post),
    PostThread(PostThread),
    Raw(Vec<u8>),
}

struct Article {
    title: String,
    body: String,
}

struct Post {
    author: String,
    body: String,
    urls: Vec<String>,
}

impl Display for Post {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            textwrap::fill(&format!("<{}> {}", self.author, self.body), LINE_LENGTH)
        )?;
        if !self.urls.is_empty() {
            writeln!(f)?;
            self.urls.iter().try_for_each(|u| write!(f, "\n{u}"))?;
        }
        Ok(())
    }
}

struct PostThread {
    main: Post,
    before: Vec<Post>,
    after: Vec<Post>,
}

impl TextType {
    fn write(&self, writer: &mut impl Write) -> io::Result<()> {
        match self {
            Self::Article(article) => {
                write!(writer, "{}\n\n{}", article.title, article.body)
            }
            Self::Post(post) => write!(writer, "{post}"),
            Self::PostThread(thread) => {
                let post_chain = thread
                    .before
                    .iter()
                    .chain(iter::once(&thread.main))
                    .chain(&thread.after);
                post_chain.enumerate().try_for_each(|(i, p)| {
                    write!(writer, "{}{p}", if i == 0 { "" } else { "\n\n" })
                })
            }
            Self::Raw(bytes) => writer.write_all(bytes),
        }
    }
}

/// Open a program to show the content of a URL.
///
/// # Errors
///
/// This function may error for a variety of reasons including but not limited to
/// - Unsupported URL
/// - Supported domain in an unknown URL format
/// - Transport error retrieving the URL or a related URL or making an API call
/// - Unexpected HTML structure or API response
/// - The display program exited non-zero.
///
/// The particular `Error` that `anyhow` wraps is not part of API stability promises and may change
/// without a major version bump.
pub fn show_url(config: &Config, url: &str) -> anyhow::Result<()> {
    let mut url = Url::parse(url)?;
    if url.cannot_be_a_base() {
        bail!("Non-absolute URL");
    }
    if !matches!(url.scheme(), "http" | "https") {
        bail!("Unsupported URL scheme");
    }

    show_content(config, get_content(&mut url)?)
}

#[allow(clippy::too_many_lines)]
fn get_content(url: &mut Url) -> anyhow::Result<Content> {
    let agent = AgentBuilder::new()
        .user_agent(&format!("zxcv/{}", env!("CARGO_PKG_VERSION")))
        .build();

    if rewrite_url(url) {
        return process_generic(&agent, url);
    };

    if let Some(content) = process_specific(&agent, url) {
        return content;
    }

    process_generic(&agent, url)
}

fn rewrite_url(url: &mut Url) -> bool {
    let Some(hostname) = url.host_str() else {
        return false;
    };

    #[allow(clippy::match_same_arms)]
    match hostname {
        "bpa.st" => {
            if !(url.path().starts_with("/raw/") || url.path().ends_with("/raw")) {
                url.set_path(&(url.path().to_owned() + "/raw"));
            };
        }

        "p.dav1d.de" => {
            if let Some((raw_path, _)) = url.path().rsplit_once('.') {
                #[allow(clippy::unnecessary_to_owned)]
                url.set_path(&raw_path.to_owned());
            }
        }

        "paste.debian.net" => {
            if !url.path().starts_with("/plain") {
                url.path_segments_mut()
                    .expect("URL is not cannot-be-a-base")
                    .pop_if_empty();
                let Some(id) = url.path_segments().and_then(Iterator::last) else {
                    return false;
                };
                url.set_path(&format!("/plain/{id}"));
            }
        }

        "dpaste.com" => {
            if !url.path().ends_with(".txt") {
                url.set_path(&(url.path().to_owned() + ".txt"));
            };
        }

        "dpaste.org" => {
            if !url.path().ends_with("/raw") {
                url.set_path(&(url.path().to_owned() + "/raw"));
            };
        }

        "datatracker.ietf.org" => {
            if let Some(id) = url.path().strip_prefix("/doc/html/") {
                url.set_path(&format!("/archive/id/{id}.txt"));
                url.set_host(Some("www.ietf.org"))
                    .expect("hostname is valid");
            } else {
                return false;
            }
        }

        "marc.info" => {
            if url.query_pairs().any(|(k, _)| k == "q") {
                let pairs: Vec<_> = url
                    .query_pairs()
                    .filter(|(k, _)| k != "q")
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect();
                url.query_pairs_mut().clear().extend_pairs(pairs);
            }
            url.query_pairs_mut().append_pair("q", "mbox");
        }

        "paste.mozilla.org" | "pastebin.mozilla.org" => {
            if !url.path().ends_with("/raw") {
                url.set_path(&(url.path().to_owned() + "/raw"));
            };
        }

        "pastebin.com" => {
            if !url.path().starts_with("/raw") {
                url.set_path(&("/raw".to_owned() + url.path()));
            }
        }
        _ => return false,
    };
    true
}

fn process_specific(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    let hostname = url.host_str()?;

    #[allow(clippy::match_same_arms)]
    match hostname {
        "bsky.app" => bsky::process(agent, url),

        "github.com" => github::process(agent, url),

        "gist.github.com" => github::gist::process(agent, url),

        "ibb.co" => Some(image_via_selector(
            agent,
            url,
            "#image-viewer-container > img",
        )),

        "lobste.rs" => lobsters::process(agent, url),

        "mypy-play.net" => {
            let gist_pair = url.query_pairs().find(|(k, _)| k == "gist")?;
            Some(github::gist::process_by_id(agent, &gist_pair.1))
        }

        "play.integer32.com" | "play.rust-lang.org" => {
            let gist_pair = url.query_pairs().find(|(k, _)| k == "gist")?;
            Some(github::gist::process_by_id(agent, &gist_pair.1))
        }

        "soundcloud.com" | "m.soundcloud.com" => Some(Ok(Content::Audio(url.clone()))),

        "twitch.tv" => Some(Ok(Content::Video(url.clone()))),

        "en.wikipedia.org" => wikimedia::process(agent, url),

        "xkcd.com" => Some(image_via_selector(agent, url, "#comic > img")),

        "youtu.be" | "youtube.com" | "m.youtube.com" | "music.youtube.com" | "www.youtube.com" => {
            Some(Ok(Content::Video(url.clone())))
        }

        _ => {
            if let Some(result) = stackoverflow::process(agent, url) {
                return Some(result);
            }

            None
        }
    }
}

fn process_generic(agent: &Agent, url: &Url) -> anyhow::Result<Content> {
    let response = agent.request_url("GET", url).call()?;
    let content_type = response.content_type();
    let final_url =
        Url::parse(response.get_url()).expect("ureq internally stores the url as a Url");

    Ok(match content_type {
        "application/pdf" => Content::Pdf(response.into_reader()),
        "audio/mpeg" | "audio/ogg" => Content::Audio(final_url),
        "image/gif" | "image/jpeg" | "image/png" | "image/svg+xml" => {
            Content::Image(response.into_reader())
        }
        "text/html" => process_html(
            agent,
            &final_url,
            &Html::parse_document(&response.into_string()?),
        )?,
        _ if content_type.starts_with("text/") => {
            Content::Text(TextType::Raw(read_raw_response(response)?))
        }
        "video/mp4" | "video/quicktime" | "video/webm" => Content::Video(final_url),
        _ => bail!("Content type {content_type} is not supported."),
    })
}

fn process_html(agent: &Agent, url: &Url, tree: &Html) -> anyhow::Result<Content> {
    for process in [
        cgit::process,
        discourse::process,
        gitea::process,
        mastodon::process,
        nextcloud::process,
        process_main_text,
        process_body,
    ] {
        if let Some(result) = process(agent, url, tree) {
            return result;
        }
    }

    Ok(Content::Text(TextType::Raw(tree.html().into())))
}

fn process_main_text(_: &Agent, url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    process_article_selectors(&["main", "article", "div[role=\"main\"]"], url, tree)
}

fn process_body(_: &Agent, url: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    process_article_selectors(&["body"], url, tree)
}

fn process_article_selectors(
    selectors: &[&str],
    url: &Url,
    tree: &Html,
) -> Option<anyhow::Result<Content>> {
    let element = selectors
        .iter()
        .find_map(|t| html::select_single_element(tree, t))?;

    Some(Ok(Content::Text(TextType::Article(Article {
        // You may assume just title suffices, but some pages have an additional title outside of
        // head. Also can't use use "head title" as some pages put their title in body.
        title: ["title", "head title"]
            .iter()
            .find_map(|t| html::select_single_element(tree, t))
            .map(|e| e.inner_html().trim().to_owned())
            .unwrap_or_default(),
        body: html::render_node(*element, url, NonZeroUsize::new(LINE_LENGTH)),
    }))))
}

/// Display the image specfied by `selector`.
///
/// # Panics
///
/// It is the caller's responsibility to ensure the `selector` is valid.
fn image_via_selector(agent: &Agent, url: &Url, selector: &str) -> anyhow::Result<Content> {
    let response = agent.request_url("GET", url).call()?;
    let tree = Html::parse_document(&response.into_string()?);
    let Some(img) = html::select_single_element(&tree, selector) else {
        bail!("Expected one image matching selector {selector};");
    };
    let url = url.join(
        img.value()
            .attr("src")
            .expect("img element must have a src"),
    )?;
    process_generic(agent, &url)
}

fn show_content(config: &Config, mut content: Content) -> anyhow::Result<()> {
    let argv = config.get_argv(&content);

    // replacements are documented with Config.
    let (file, mut replacements): (Option<NamedTempFile>, HashMap<char, OsString>) = match content {
        Content::Audio(url) | Content::Video(url) => (None, [('u', url.as_str().into())].into()),

        Content::Image(ref mut reader) | Content::Pdf(ref mut reader) => {
            let mut file = NamedTempFile::new()?;
            io::copy(reader, &mut file)?;
            (Some(file), [].into())
        }

        Content::Text(text) => {
            let mut file = NamedTempFile::new()?;
            text.write(&mut file)?;
            let pager = env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
            if pager == "less" {
                env::set_var(
                    "LESS",
                    env::var("LESS")
                        .unwrap_or_else(|_| String::new())
                        .chars()
                        .filter(|c| !matches!(c, 'E' | 'e' | 'F'))
                        // r is unsafe with untrusted input.
                        .map(|c| if c == 'r' { 'R' } else { c })
                        .collect::<String>(),
                );
            }
            (Some(file), [('p', pager.into())].into())
        }
    };

    if let Some(file) = &file {
        replacements.insert('f', file.path().into());
    }

    let mut command = Command::new(&argv[0]);
    command.args(
        argv[1..]
            .iter()
            .map(|arg| {
                if arg.chars().count() == 2 && arg.starts_with('%') {
                    let char = arg.chars().nth(1).expect("length checked above");
                    replacements
                        .get(&char)
                        .ok_or_else(|| anyhow!("%{char} is not valid for this content type"))
                        .map(AsRef::as_ref)
                } else {
                    Ok(arg.as_ref())
                }
            })
            .collect::<anyhow::Result<Vec<&OsStr>>>()?,
    );

    let exit_status = command.status()?;
    if exit_status.success() {
        Ok(())
    } else {
        bail!("Command exited {exit_status}");
    }
}

fn read_raw_response(response: ureq::Response) -> io::Result<Vec<u8>> {
    const MAX_RAW_LEN: u32 = 1024 * 1024;
    let capacity = response
        .header("Content-Length")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut body: Vec<u8> = Vec::with_capacity(std::cmp::min(capacity, MAX_RAW_LEN as usize));
    response
        .into_reader()
        .take(u64::from(MAX_RAW_LEN))
        .read_to_end(&mut body)?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::rewrite_url;

    macro_rules! rewrite_tests {
        ($(($name: ident, $url: expr, $expected: expr),)*) => {
            $(
                #[test]
                fn $name() {
                    rewrite_test($url, $expected);
                }
            )*
        }
    }

    fn rewrite_test(url: &str, expected: &str) {
        let mut url = Url::parse(url).unwrap();
        let expected = Url::parse(expected).unwrap();

        assert!(rewrite_url(&mut url));
        assert_eq!(url, expected);
        assert!(rewrite_url(&mut url));
        assert_eq!(url, expected);
    }

    rewrite_tests!(
        (
            bpa_st,
            "https://bpa.st/example",
            "https://bpa.st/example/raw"
        ),
        (
            bpa_st_pre_raw,
            "https://bpa.st/raw/example",
            "https://bpa.st/raw/example"
        ),
        (
            dav1d_de,
            "https://p.dav1d.de/example.rs",
            "https://p.dav1d.de/example"
        ),
        (
            paste_debian_net,
            "https://paste.debian.net/1729/",
            "https://paste.debian.net/plain/1729"
        ),
        (
            dpaste_com,
            "https://dpaste.com/example",
            "https://dpaste.com/example.txt"
        ),
        (
            dpaste_org,
            "https://dpaste.org/example",
            "https://dpaste.org/example/raw"
        ),
        (
            marc_info,
            "https://marc.info/?l=example&m=1729&w=2",
            "https://marc.info/?l=example&m=1729&w=2&q=mbox"
        ),
        (
            paste_mozilla_org,
            "https://paste.mozilla.org/example",
            "https://paste.mozilla.org/example/raw"
        ),
        (
            pastebin_com,
            "https://pastebin.com/example",
            "https://pastebin.com/raw/example"
        ),
    );

    #[test]
    fn rewrite_unknown() {
        let mut url = Url::parse("https://example.com").unwrap();
        let expected = url.clone();

        assert!(!rewrite_url(&mut url));
        assert_eq!(url, expected);
    }

    #[test]
    fn rewrite_ip() {
        let mut url = Url::parse("https://192.0.2.17/example").unwrap();
        let expected = url.clone();

        assert!(!rewrite_url(&mut url));
        assert_eq!(url, expected);
    }
}
