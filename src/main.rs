use wayland_clipboard_listener::WlClipboardListenerStream;
use wayland_clipboard_listener::WlListenType;

use std::{thread, time};

fn main() {
    let stream = WlClipboardListenerStream::init(WlListenType::ListenOnSelect).unwrap();

    for context in stream.flatten() {
        thread::sleep(time::Duration::from_millis(100));
        println!("{context:?}");
    }
}
