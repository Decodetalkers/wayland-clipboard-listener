use waylandcopy::WaylandCopyStream;

fn main() {
    let stream = WaylandCopyStream::init().unwrap();

    for context in stream.flatten().flatten() {
        println!("{context}");
    }
}
