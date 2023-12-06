# zxcv

`zxcv` (z xssential content viewer) is a command for viewing the essential
content of a URL.

`zxcv` takes the essential content of a web page (e.g. the text of a pastebin
link or the video of a youtube link) and runs an appropriate command to display
that content locally (e.g. `less`, `mupdf`, or `mpv`).

## Configuration

A configuration file may be passed via the `-f` flag. The configuration file is in
[TOML](https://toml.io) format and the accepted sections are documented with
the Config struct.
