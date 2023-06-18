use wayland_clipboard_listener::{WlClipboardCopyStream, WlClipboardListenerError};
fn main() -> Result<(), WlClipboardListenerError> {
    let args = std::env::args();
    if args.len() != 2 {
        println!("You need to pass a string to it");
        return Ok(());
    }
    let context: &str = &args.last().unwrap();
    let mut stream = WlClipboardCopyStream::init()?;
    stream.copy_to_clipboard(context.as_bytes().to_vec())?;
    Ok(())
}
