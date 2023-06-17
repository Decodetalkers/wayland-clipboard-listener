use wayland_clipboard_listener::WlClipboardListenerStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let mut stream = WlClipboardListenerStream::init(WlListenType::ListenOnCopy).unwrap();

    stream.copy_to_clipboard(b"gammer".to_vec()).unwrap();
    println!("{:?}", stream.get_clipboard().unwrap());
    for context in stream.flatten() {
        println!("{context:?}");
    }
}
