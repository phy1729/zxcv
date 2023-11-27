use std::env;

use anyhow::bail;
use anyhow::Context;
use arboard::Clipboard;
use arboard::GetExtLinux;
use arboard::LinuxClipboardKind;
use getopt::Opt;
use getopt::Parser;
use pledge::pledge_promises;

use zxcv::show_url;

fn main() -> anyhow::Result<()> {
    let mut use_clipboard = false;
    let mut args: Vec<String> = env::args().collect();
    let mut opts = Parser::new(&args, "c");
    loop {
        match opts.next().transpose()? {
            None => break,
            Some(opt) => match opt {
                Opt('c', None) => {
                    use_clipboard = true;
                }
                _ => unreachable!(),
            },
        }
    }
    args = args.split_off(opts.index());

    let url = if use_clipboard {
        if !args.is_empty() {
            bail!("Arguments are not accepted with -c");
        }
        Clipboard::new()?
            .get()
            .clipboard(LinuxClipboardKind::Primary)
            .text()
            .context("Error getting clipboard text")?
    } else {
        if args.len() != 1 {
            bail!("One argument is required");
        }
        args.pop().expect("Checked above")
    };

    pledge_promises!(Stdio Tmppath Inet Dns Proc Exec).expect("Initial pledge cannot fail");

    show_url(&url)
}
