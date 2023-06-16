use wayland_clipboard_listener::WlClipboardListenerStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let stream = WlClipboardListenerStream::init(WlListenType::ListenOnSelect).unwrap();

    for context in stream.flatten() {
        println!("{context:?}");
    }
}
