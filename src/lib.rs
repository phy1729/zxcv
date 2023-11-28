//! `zxcv` (z xssential content viewer) is a command for viewing the essential content of a URL.
//!
//! `zxcv` takes the essential content of a web page (e.g. the text of a pastebin link or the video
//! of a youtube link) and runs an appropriate command to display that content locally (e.g.
//! `less`, `mupdf`, or `mpv`).
//!
//! Use the `-c` flag to pull the URL from the clipboard.
#![warn(
    missing_debug_implementations,
    missing_docs,
    unsafe_code,
    clippy::cargo,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::pedantic,
    clippy::str_to_string,
    clippy::tests_outside_test_module,
    clippy::unwrap_used,
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
use std::process::Command;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use scraper::ElementRef;
use scraper::Html;
use scraper::Node;
use scraper::Selector;
use tempfile::NamedTempFile;
use url::Url;

mod github;
use crate::github::process_github_gist;

mod mastodon;
mod nextcloud;
mod stackoverflow;
mod wikimedia;

enum Content {
    Image(Box<dyn Read>),
    Pdf(Box<dyn Read>),
    Text(TextType),
    Video(String), // URL
}

enum TextType {
    Article(Article),
    Post(Post),
    PostThread(PostThread),
    Raw(String),
}

struct Article {
    title: String,
    body: String,
}

struct Post {
    author: String,
    body: String,
}

impl Display for Post {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "{}",
            textwrap::fill(&format!("<{}> {}", self.author, self.body), 80)
        )
    }
}

struct PostThread {
    main: Post,
    before: Vec<Post>,
    after: Vec<Post>,
}

impl Display for TextType {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            Self::Article(article) => {
                write!(
                    f,
                    "{}\n\n{}",
                    article.title,
                    textwrap::fill(&article.body, 80)
                )
            }
            Self::Post(post) => write!(f, "{post}"),
            Self::PostThread(thread) => {
                let post_chain = thread
                    .before
                    .iter()
                    .chain(iter::once(&thread.main))
                    .chain(&thread.after);
                post_chain
                    .enumerate()
                    .try_for_each(|(i, p)| write!(f, "{}{p}", if i == 0 { "" } else { "\n\n" }))
            }
            Self::Raw(string) => write!(f, "{string}"),
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
pub fn show_url(url: &str) -> anyhow::Result<()> {
    let mut url = Url::parse(url)?;
    if url.cannot_be_a_base() {
        bail!("Non-absolute URL");
    }
    if !matches!(url.scheme(), "http" | "https") {
        bail!("Unsupported URL scheme");
    }

    show_content(get_content(&mut url)?)
}

fn get_content(url: &mut Url) -> anyhow::Result<Content> {
    Ok(if let Some(hostname) = url.host_str() {
        match hostname {
            "bpa.st" => {
                if !url.path().ends_with("/raw") {
                    url.set_path(&(url.path().to_owned() + "/raw"));
                };
                process_generic(url)?
            }

            "p.dav1d.de" => {
                if let Some((raw_path, _)) = url.path().rsplit_once('.') {
                    #[allow(clippy::unnecessary_to_owned)]
                    url.set_path(&raw_path.to_owned());
                }
                process_generic(url)?
            }

            "paste.debian.net" => {
                if !url.path().starts_with("/plain") {
                    url.path_segments_mut()
                        .expect("URL is not cannot-be-a-base")
                        .pop_if_empty();
                    let Some(id) = url.path_segments().and_then(Iterator::last) else {
                        bail!("Unknown Debian paste URL");
                    };
                    url.set_path(&format!("/plain/{id}"));
                }
                process_generic(url)?
            }

            "dpaste.com" => {
                if !url.path().ends_with(".txt") {
                    url.set_path(&(url.path().to_owned() + ".txt"));
                };
                process_generic(url)?
            }

            "gist.github.com" => process_github_gist(url)?,

            "ibb.co" => image_via_selector(url, "#image-viewer-container > img")?,

            "datatracker.ietf.org" => {
                if let Some(id) = url.path().strip_prefix("/doc/html/") {
                    process_generic(
                        &Url::parse(&format!("https://www.ietf.org/archive/id/{id}.txt"))
                            .expect("URL is valid"),
                    )?
                } else {
                    bail!("Unknown IETF URL");
                }
            }

            "marc.info" => {
                url.query_pairs_mut().append_pair("q", "mbox");
                process_generic(url)?
            }

            "pastebin.com" => {
                if url.path_segments().map_or(0, Iterator::count) == 1 {
                    url.set_path(&format!("/raw{}", url.path()));
                    process_generic(url)?
                } else {
                    bail!("Unknown pastebin URL");
                }
            }

            "play.integer32.com" | "play.rust-lang.org" => {
                let gist = url
                    .query_pairs()
                    .find(|(k, _)| k == "gist")
                    .with_context(|| "Rust playground URL missing gist param")?
                    .1;
                let url = Url::parse(&format!("https://gist.github.com/rust-play/{gist}"))
                    .expect("URL is valid");
                process_github_gist(&url)?
            }

            "en.wikipedia.org" => wikimedia::process(url)?,

            "xkcd.com" => image_via_selector(url, "#comic > img")?,

            "youtu.be" | "youtube.com" | "www.youtube.com" => Content::Video(url.to_string()),

            _ => {
                if let Some(result) = stackoverflow::process(url) {
                    return result;
                }

                process_generic(url)?
            }
        }
    } else {
        process_generic(url)?
    })
}

fn process_generic(url: &Url) -> anyhow::Result<Content> {
    let response = ureq::get(url.as_str()).call()?;
    let content_type = response.content_type().to_owned();

    Ok(match content_type.as_str() {
        "application/pdf" => Content::Pdf(response.into_reader()),
        "image/gif" | "image/jpeg" | "image/png" | "image/svg+xml" => {
            Content::Image(response.into_reader())
        }
        "text/html" => process_html(url, &Html::parse_document(&response.into_string()?))?,
        "text/plain" | "text/x-shellscript" => {
            Content::Text(TextType::Raw(response.into_string()?))
        }
        "video/mp4" | "video/quicktime" | "video/webm" => Content::Video(url.to_string()),
        _ => bail!("Content type {content_type} is not supported."),
    })
}

fn process_html(url: &Url, tree: &Html) -> anyhow::Result<Content> {
    for process in [
        mastodon::process,
        nextcloud::process,
        process_single_video,
        process_single_pre,
    ] {
        if let Some(result) = process(url, tree) {
            return result;
        }
    }

    Ok(Content::Text(TextType::Raw(tree.html())))
}

fn process_single_video(_: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    let Some(video) = select_single_element(tree, "video") else {
        return None;
    };

    Some((|| {
        if let Some(src) = video.attr("src") {
            return Ok(Content::Video(src.to_owned()));
        }

        for child in video.children() {
            if let Node::Element(element) = child.value() {
                if element.name() == "source" {
                    if !matches!(element.attr("type"), Some("video/mp4")) {
                        continue;
                    }
                    if let Some(url) = element.attr("src") {
                        return Ok(Content::Video(url.to_owned()));
                    }
                }
            }
        }
        bail!("No supported video formats");
    })())
}

fn process_single_pre(_: &Url, tree: &Html) -> Option<anyhow::Result<Content>> {
    select_single_element(tree, "pre").map(|p| Ok(Content::Text(TextType::Raw(p.inner_html()))))
}

/// Display the image specfied by `selector`.
///
/// # Panics
///
/// It is the caller's responsibility to ensure the `selector` is valid.
fn image_via_selector(url: &Url, selector: &str) -> anyhow::Result<Content> {
    let response = ureq::get(url.as_str()).call()?;
    let tree = Html::parse_document(&response.into_string()?);
    let Some(img) = select_single_element(&tree, selector) else {
        bail!("Expected one image matching selector {selector};");
    };
    let url = url.join(
        img.value()
            .attr("src")
            .expect("img element must have a src"),
    )?;
    process_generic(&url)
}

fn show_content(mut content: Content) -> anyhow::Result<()> {
    let argv = match content {
        Content::Image(_) | Content::Pdf(_) => vec!["mupdf", "--", "%f"],
        Content::Text(_) => vec!["xterm", "-e", "%p", "--", "%f"],
        Content::Video(_) => vec!["mpv", "--", "%u"],
    };

    let (file, mut replacements): (Option<NamedTempFile>, HashMap<char, OsString>) = match content {
        Content::Image(ref mut reader) | Content::Pdf(ref mut reader) => {
            let mut file = NamedTempFile::new()?;
            io::copy(reader, &mut file)?;
            (Some(file), [].into())
        }

        Content::Text(text) => {
            let mut file = NamedTempFile::new()?;
            write!(file, "{text}")?;
            let pager = env::var("PAGER").unwrap_or_else(|_| "less".to_owned());
            if pager == "less" {
                env::set_var(
                    "LESS",
                    env::var("LESS")
                        .unwrap_or_else(|_| String::new())
                        .chars()
                        .filter(|c| !matches!(c, 'E' | 'e' | 'F'))
                        .collect::<String>(),
                );
            }
            (Some(file), [('p', pager.into())].into())
        }

        Content::Video(url) => (None, [('u', url.into())].into()),
    };

    if let Some(file) = &file {
        replacements.insert('f', file.path().into());
    }

    let mut command = Command::new(argv[0]);
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

/// Return the single element matched by `selector` or `None` if there are zero or more than one
/// matches.
///
/// # Panics
///
/// It is the caller's responsibility to ensure the `selector` is valid.
fn select_single_element<'a>(tree: &'a Html, selector_string: &str) -> Option<ElementRef<'a>> {
    let selector = Selector::parse(selector_string).expect("Caller must supply a valid selector");
    let mut iter = tree.select(&selector).fuse();
    match (iter.next(), iter.next()) {
        (Some(element), None) => Some(element),
        _ => None,
    }
}

fn render_html_text(html: &str) -> String {
    Html::parse_fragment(html)
        .root_element()
        .descendants()
        .filter_map(|e| match e.value() {
            Node::Text(t) => Some(&**t),
            Node::Element(e) if e.name() == "p" => Some("\n\n"),
            _ => None,
        })
        .skip_while(|&s| s == "\n\n")
        .collect::<String>()
}
