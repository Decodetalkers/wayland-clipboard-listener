use wayland_clipboard_listener::WlClipboardListenerStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let stream = WlClipboardListenerStream::init(WlListenType::ListenOnCopy).unwrap();
    for context in stream.flatten().flatten() {
        println!("{context:?}");
    }
}
