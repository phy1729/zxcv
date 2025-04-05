use anyhow::Context;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use ureq::Agent;
use url::Url;

use crate::process_generic;
use crate::Collection;
use crate::Content;
use crate::Item;

const API_BASE: &str = "https://api.imgur.com/3";
const IMGUR_PUBLIC_CLIENT_ID: &str = "546c25a59c58ad7";

#[derive(Debug)]
enum Path<'a> {
    Album(&'a str),
    Image(&'a str),
    Gallery(&'a str),
}

#[derive(Debug)]
enum Kind {
    Album(Album),
    Image(Image),
}

fn parse_path(url: &Url) -> Option<Path<'_>> {
    let path_segments: Vec<_> = url
        .path_segments()
        .unwrap_or_else(|| "".split('/'))
        .collect();

    let (kind, full_id): (fn(_) -> _, _) = if path_segments.len() == 1 {
        Some((Path::Image as _, path_segments[0]))
    } else if path_segments.len() == 2 && path_segments[0] == "a" {
        Some((Path::Album as _, path_segments[1]))
    } else if path_segments.len() == 2 && path_segments[0] == "gallery" {
        Some((Path::Gallery as _, path_segments[1]))
    } else {
        None
    }?;

    Some(kind(full_id.rsplit_once('-').map_or(full_id, |(_, id)| id)))
}

pub(crate) fn process(agent: &Agent, url: &mut Url) -> Option<anyhow::Result<Content>> {
    let path = parse_path(url)?;

    Some((|| {
        let result = match path {
            Path::Album(album_hash) => {
                Kind::Album(request(agent, &format!("{API_BASE}/album/{album_hash}"))?)
            }

            Path::Gallery(gallery_hash) => {
                if let Ok(album) =
                    request(agent, &format!("{API_BASE}/gallery/album/{gallery_hash}"))
                {
                    Kind::Album(album)
                } else {
                    Kind::Image(request(
                        agent,
                        &format!("{API_BASE}/gallery/image/{gallery_hash}"),
                    )?)
                }
            }

            Path::Image(image_hash) => {
                Kind::Image(request(agent, &format!("{API_BASE}/image/{image_hash}"))?)
            }
        };

        match result {
            Kind::Album(album) => {
                if album.images.len() == 1 {
                    process_generic(
                        agent,
                        &Url::parse(&album.images[0].link)
                            .context("Imgur API returned invalid URL")?,
                    )
                } else {
                    Ok(Content::Collection(Collection {
                        title: if album.title.is_empty() {
                            None
                        } else {
                            Some(album.title)
                        },
                        items: album
                            .images
                            .into_iter()
                            .map(|i| Item {
                                title: i.title,
                                url: i.link,
                            })
                            .collect(),
                    }))
                }
            }

            Kind::Image(image) => process_generic(
                agent,
                &Url::parse(&image.link).context("Imgur API returned invalid URL")?,
            ),
        }
    })())
}

fn request<T: DeserializeOwned>(agent: &Agent, url: &str) -> anyhow::Result<T> {
    let result: Response<T> = agent
        .get(url)
        .header(
            "Authorization",
            &format!("Client-ID {IMGUR_PUBLIC_CLIENT_ID}"),
        )
        .call()?
        .body_mut()
        .read_json()?;
    Ok(result.data)
}

#[derive(Debug, Deserialize)]
struct Response<T> {
    data: T,
}

#[derive(Debug, Deserialize)]
struct Album {
    title: String,
    images: Vec<AlbumImage>,
}

#[derive(Debug, Deserialize)]
struct AlbumImage {
    title: Option<String>,
    link: String,
}

#[derive(Debug, Deserialize)]
struct Image {
    link: String,
}

#[cfg(test)]
mod tests {
    use url::Url;

    use super::parse_path;
    use super::Path;

    macro_rules! parse_path_tests {
        ($(($name: ident, $path: expr, $expected: pat),)*) => {
            $(
                #[test]
                fn $name() {
                    assert!($path.starts_with('/'));
                    let url = Url::parse(&format!("https://imgur.com{}", $path)).unwrap();
                    assert!(matches!(parse_path(&url), $expected));
                }
            )*
        }
    }

    parse_path_tests!(
        (album, "/a/abcdefg", Some(Path::Album("abcdefg"))),
        (
            album_title,
            "/a/title-abcdefg",
            Some(Path::Album("abcdefg"))
        ),
        (gallery, "/gallery/abcdefg", Some(Path::Gallery("abcdefg"))),
        (
            gallery_title,
            "/gallery/foo-bar-baz-abcdefg",
            Some(Path::Gallery("abcdefg"))
        ),
        (image, "/abcdefg", Some(Path::Image("abcdefg"))),
        (image_title, "/title-abcdefg", Some(Path::Image("abcdefg"))),
        (unknown, "/unknown/path", None),
    );
}
