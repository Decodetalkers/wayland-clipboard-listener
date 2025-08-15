use std::io::{stdout, Write};

use wayland_clipboard_listener::{
    ClipBoardListenMessage, WlClipboardListenerError, WlClipboardPasteStream, WlListenType,
};

fn main() -> Result<(), WlClipboardListenerError> {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy)?;
    let Some(ClipBoardListenMessage { context, .. }) = stream.try_get_clipboard()? else {
        eprintln!("Warning, no context in clipboard");
        return Ok(());
    };
    let context = context.context;
    stdout().write_all(&context).unwrap();
    Ok(())
}
