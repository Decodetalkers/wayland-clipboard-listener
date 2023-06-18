use wayland_clipboard_listener::WlClipboardListenerStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let stream = WlClipboardListenerStream::init(WlListenType::ListenOnCopy).unwrap();

    for context in stream.flatten() {
        println!("{context:?}");
    }
    //let mut stream = WlClipboardListenerStream::init(WlListenType::ListenOnCopy).unwrap();
    //stream.copy_to_clipboard(b"gammer".to_vec()).unwrap();
}
