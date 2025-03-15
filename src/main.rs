//! The entrypoint for `zxcv`.
use std::env;

use anyhow::bail;
use anyhow::Context;
use getopt::Opt;
use getopt::Parser;
use pledge::pledge_promises;

use zxcv::show_url;
use zxcv::Config;

fn main() -> anyhow::Result<()> {
    let mut config_file = None;

    let mut args: Vec<String> = env::args().collect();
    let mut opts = Parser::new(&args, "f:");
    loop {
        match opts.next().transpose()? {
            None => break,
            Some(opt) => match opt {
                Opt('f', Some(arg)) => config_file = Some(arg),
                _ => unreachable!(),
            },
        }
    }
    args = args.split_off(opts.index());

    let config = if let Some(config_file) = config_file {
        Config::from_toml(
            &std::fs::read_to_string(config_file).context("Failed to open config file")?,
        )?
    } else {
        Config::default()
    };

    let [url] = args.as_slice() else {
        bail!("One argument is required");
    };

    pledge_promises!(Stdio Tmppath Inet Dns Proc Exec)
        .or_else(pledge::Error::ignore_platform)
        .expect("Initial pledge cannot fail");

    show_url(&config, url)
}
