use crate::Content;

use serde::Deserialize;

/// Configuration for `zxcv`.
///
/// # Examples
///
/// ```toml
/// [argv]
/// text = ["less", "--", "%f"]
/// ```
///
/// # `[argv]`
///
/// The argv section defines which command to run for a given content type.
///
/// | Key | Default |
/// | --- | ------- |
/// | audio | `["mpv", "--profile=builtin-pseudo-gui", "--", "%u"]` |
/// | image | `["mupdf", "--", "%f"]` |
/// | pdf | `["mupdf", "--", "%f"]` |
/// | text | `["xterm", "-e", "%p", "--", "%f"]` |
/// | video | `["mpv", "--", "%u"]` |
///
/// The argv array accepts `%` substitutions depending on the content type. The `%` substitution
/// must be the entirety of the array element; concatenation with other strings is not supported.
///
/// | Content Type | Flag | Description |
/// | ------------ | ---- | ----------- |
/// | Audio | `%u` | URL of the audio. |
/// | Image | `%f` | Filename of a temporary file containing the image. |
/// | PDF | `%f` | Filename of a temporary file containing the PDF. |
/// | Text | `%f` | Filename of a temporary file containing the text. |
/// | Text | `%p` | Value of the `PAGER` environment variable or an empty string if unset. |
/// | Video | `%u` | URL of the video. |
#[derive(Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    argv: Argv,
}

#[derive(Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
struct Argv {
    audio: Vec<String>,
    image: Vec<String>,
    pdf: Vec<String>,
    text: Vec<String>,
    video: Vec<String>,
}

impl Default for Argv {
    fn default() -> Self {
        Self {
            audio: ["mpv", "--profile=builtin-pseudo-gui", "--", "%u"]
                .iter()
                .map(|&s| s.to_owned())
                .collect(),
            image: ["mupdf", "--", "%f"]
                .iter()
                .map(|&s| s.to_owned())
                .collect(),
            pdf: ["mupdf", "--", "%f"]
                .iter()
                .map(|&s| s.to_owned())
                .collect(),
            text: ["xterm", "-e", "%p", "--", "%f"]
                .iter()
                .map(|&s| s.to_owned())
                .collect(),
            video: ["mpv", "--", "%u"].iter().map(|&s| s.to_owned()).collect(),
        }
    }
}

impl Config {
    /// Parse value from a TOML str.
    ///
    /// # Errors
    ///
    /// Returns an error if there is an issue parsing the TOML string.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zxcv::Config;
    /// #
    /// assert!(Config::from_toml(r#"
    /// [argv]
    /// text = ["less", "--", "%f"]
    /// "#).is_ok())
    /// ```
    #[inline]
    pub fn from_toml(config: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(config)
    }

    pub(crate) fn get_argv(&self, content: &Content) -> &[String] {
        match content {
            Content::Audio(_) => &self.argv.audio,
            Content::Image(_) => &self.argv.image,
            Content::Pdf(_) => &self.argv.pdf,
            Content::Text(_) => &self.argv.text,
            Content::Video(_) => &self.argv.video,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    #[test]
    fn empty_is_default() {
        assert_eq!(Config::from_toml("").unwrap(), Config::default());
    }

    #[test]
    fn err_on_invalid() {
        assert!(Config::from_toml("[foo]\ntext = [\"baz\"]\n").is_err());
        assert!(Config::from_toml("[argv]\nbar = [\"baz\"]\n").is_err());
        assert!(Config::from_toml("[argv]\ntext = \"baz\"\n").is_err());
        assert!(Config::from_toml("[argv]\ntext = [\"baz\"]\n").is_ok());
    }
}
