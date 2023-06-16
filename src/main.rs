use wayland_clipboard_listener::WlListenType;
use wayland_clipboard_listener::WlClipboardListenerStream;

fn main() {
    let stream = WlClipboardListenerStream::init(WlListenType::ListenOnHover).unwrap();

    for context in stream.flatten().flatten() {
        println!("{context:?}");
    }
}
