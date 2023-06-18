use wayland_clipboard_listener::WlClipboardPasteStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();

    for context in stream.paste_stream().flatten() {
        println!("{context:?}")
    }
}
