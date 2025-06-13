# wayland-clipboard-listener

I would rather to use animate girl to name the crate, but it will cannot show how the crate do

it impl `wlr-data-control-unstable-v1`, you can see the protocol under

[wlr-data-control-unstable-v1](https://wayland.app/protocols/wlr-data-control-unstable-v1)

you can use it on sway or kde

it is GPL-v3, if you use the code, you need to provide your code, you should not fuck to use the code to listen on people's clipboard and upload it to you company or government, you must fuck to opensource your code.

## General Induction
impl wlr-data-control-unstable-v1, handle the clipboard on sway, hyperland or kde animpl the
protocol.
You can view the protocol in [wlr-data-control-unstable-v1](https://wayland.app/protocols/wlr-data-control-unstable-v1). Here we simply explain it.

This protocol involves there register: WlSeat, ZwlrDataControlManagerV1,
ZwlrDataControlDeviceV1, and zwlrDataControlOfferV1, seat is used to create a device, and the
device will handle the copy and paste,

when you want to use this protocol, you need to init these first, then enter the eventloop, you
can view our code, part of `init()`

### Paste
Copy is mainly in the part of device dispatch and dataoffer one, there are two road to finished
a copy event, this is decided by the time you send the receive request of ZwlrDataControlDeviceV1;

#### Road 1

* 1. first, the event enter to DataOffer event of zwlrDataControlOfferV1, it will send a
zwlrDataControlOfferV1 object, this will include the data message of clipboard, if you send
this time, you will not know the mimetype. In this time, the data includes the text selected
and copied, here you can pass a file description to receive, and mimetype of TEXT, because at
this time you do not know any mimetype of the data
* 2. it will enter the event of zwlrDataControlOfferV1, there the mimetype be send, but before
, you ignore the mimetype
* 3. it enter the selection, follow the document of the protocol, you need to destroy the offer,
if there is one,
* 4. the main loop is end, then you need to run roundtrip, again, for the pipeline finished,
then you will receive the text. Note, if in this routine, you need to check the mimetype in the
end, because the data in pipeline maybe not text

### Road 2
it is similar with Road 1, but send receive request when receive selection event, this time you
will receive mimetype. Here you can only receive the data which is by copy

### Copy

Paste with wlr-data-control-unstable-v1, need data provider alive, you can make an experiment,
if you copy a text from firefox, and kill firefox, you will find, you cannot paste! It is
amazing, doesn't it? So the copy event need to keep alive if the data is still available. You
will find that if you copy a text with wl-copy, it will always alive in htop, if you view the
code, you will find it fork itself, and live in the backend, until you copy another text from
other place, it will die.

Then the problem is, how to copy the data, and when to kill the progress?

Copy event involves ZwlrDataControlDeviceV1 and ZwlrDataControlSourceV1.

* 1. if you want to send the data, you need to create a new ZwlrDataControlSourceV1, use the
create_data_source function of zwlr_data_control_manager_v1, create a new one, and set the
mimetype to it , use `offer` request. You can set multi times,
* 2. start a never end loop of blocking_dispatch, but it is not never end loop, it should break
when receive cancelled event of ZwlrDataControlSourceV1, this means another data is copied, the
progress is not needed anymore
   * 2.1 in blocking_dispatches at the beginning, you will receive some signals of send, with
   mimetype and a file description, write the data to the fd, then copy will finished, data
   will in clipboard
   * 2.2 when received cancelled, exit the progress

A simple example to create a clipboard listener is following:

```rust
use wayland_clipboard_listener::WlClipboardPasteStream;
use wayland_clipboard_listener::WlListenType;

fn main() {
    let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();
    for context in stream.paste_stream().flatten().flatten() {
        println!("{context:?}");
    }
}

```

A simple example to create a wl-copy is following:
``` rust
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
```

Thanks to wl-clipboard-rs, and smithay.

You can take a look to the repo following:

* [wl-clipboard-rs](https://github.com/YaLTeR/wl-clipboard-rs)

* [smithay](https://github.com/Smithay/smithay)
