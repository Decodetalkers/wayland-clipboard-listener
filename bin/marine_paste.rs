use std::io::{stdout, Write};

use wayland_clipboard_listener::{
    ClipBoardListenMessage, WlClipboardListenerError, WlClipboardListenerStream, WlListenType,
};

fn main() -> Result<(), WlClipboardListenerError> {
    let mut stream = WlClipboardListenerStream::init(WlListenType::ListenOnCopy)?;
    let Some(ClipBoardListenMessage { context, .. }) = stream.get_clipboard()? else {
        eprintln!("Warning, no context in clipboard");
        return Ok(());
    };
    let context = match context {
        wayland_clipboard_listener::ClipBoardListenContext::Text(text) => text.as_bytes().to_vec(),
        wayland_clipboard_listener::ClipBoardListenContext::File(bites) => bites.clone(),
    };
    stdout().write_all(&context).unwrap();
    Ok(())
}
