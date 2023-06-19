use libc::{STDIN_FILENO, STDOUT_FILENO};
use nix::{
    fcntl::OFlag,
    unistd::{close, dup2, fork, ForkResult},
};
use wayland_clipboard_listener::{WlClipboardCopyStream, WlClipboardListenerError};

use std::io::{stdin, Read};

fn main() -> Result<(), WlClipboardListenerError> {
    let args = std::env::args();
    let context = {
        let len = args.len();
        if len != 2 {
            let mut context = vec![];
            stdin().lock().read_to_end(&mut context).unwrap();
            context
        } else {
            args.last().unwrap().as_bytes().to_vec()
        }
    };
    if context.is_empty() {
        eprintln!("You need to pass something in");
        return Ok(());
    }

    let mut stream = WlClipboardCopyStream::init()?;

    if let Ok(ForkResult::Child) = unsafe { fork() } {
        if let Ok(dev_null) =
            nix::fcntl::open("/dev/null", OFlag::O_RDWR, nix::sys::stat::Mode::empty())
        {
            let _ = dup2(dev_null, STDIN_FILENO);
            let _ = dup2(dev_null, STDOUT_FILENO);
            let _ = close(dev_null);
            stream.copy_to_clipboard(context, false)?;
        }
    }

    Ok(())
}
