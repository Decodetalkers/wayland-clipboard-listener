use wayland_clipboard_listener::WlClipboardListenerStream;

fn main() {
    let stream = WlClipboardListenerStream::init().unwrap();

    for context in stream.flatten().flatten() {
        println!("{context:?}");
    }
}
