//! ## General Induction
//! impl wlr-data-control-unstable-v1, handle the clipboard on sway, hyperland or kde animpl the
//! protocol.
//! You can view the protocol in [wlr-data-control-unstable-v1](https://wayland.app/protocols/wlr-data-control-unstable-v1). Here we simply explain it.
//!
//! This protocol involves there register: WlSeat, ZwlrDataControlManagerV1,
//! ZwlrDataControlDeviceV1, and zwlrDataControlOfferV1, seat is used to create a device, and the
//! device will handle the copy and paste,
//!
//! when you want to use this protocol, you need to init these first, then enter the eventloop, you
//! can view our code, part of `init()`
//!
//! ### Paste
//! Copy is mainly in the part of device dispatch and dataoffer one, there are two road to finished
//! a copy event, this is decided by the time you send the receive request of ZwlrDataControlDeviceV1;
//!
//! #### Road 1
//!
//! * 1. first, the event enter to DataOffer event of zwlrDataControlOfferV1, it will send a
//! zwlrDataControlOfferV1 object, this will include the data message of clipboard, if you send
//! this time, you will not know the mimetype. In this time, the data includes the text selected
//! and copied, here you can pass a file description to receive, and mimetype of TEXT, because at
//! this time you do not know any mimetype of the data
//! * 2. it will enter the event of zwlrDataControlOfferV1, there the mimetype be send, but before
//! , you ignore the mimetype
//! * 3. it enter the selection, follow the document of the protocol, you need to destory the offer,
//! if there is one,
//! * 4. the main loop is end, then you need to run roundtrip, again, for the pipeline finished,
//! then you will receive the text. Note, if in this routine, you need to check the mimetype in the
//! end, because the data in pipeline maybe not text
//!
//! ### Road 2
//! it is similar with Road 1, but send receive request when receive selection event, this time you
//! will receive mimetype. Here you can only receive the data which is by copy
//!
//! ### Copy
//!
//! Paste with wlr-data-control-unstable-v1, need data provider alive, you can make an experiment,
//! if you copy a text from firefox, and kill firefox, you will find, you cannot paste! It is
//! amazing, doesn't it? So the copy event need to keep alive if the data is still available. You
//! will find that if you copy a text with wl-copy, it will always alive in htop, if you view the
//! code, you will find it fork itself, and live in the backend, until you copy another text from
//! other place, it will die.
//!
//! Then the problem is, how to copy the data, and when to kill the progress?
//!
//! Copy event involves ZwlrDataControlDeviceV1 and ZwlrDataControlSourceV1.
//!
//! * 1. if you want to send the data, you need to create a new ZwlrDataControlSourceV1, use the
//! create_data_source function of zwlr_data_control_manager_v1, create a new one, and set the
//! mimetype to it , use `offer` request. You can set muti times,
//! * 2. start a never end loop of blocking_dispatch, but it is not never end loop, it should break
//! when receive cancelled event of ZwlrDataControlSourceV1, this means another data is copied, the
//! progress is not needed anymore
//!    * 2.1 in blocking_dispatchs at the begining, you will receive some signals of send, with
//!    mimetype and a file description, write the data to the fd, then copy will finished, data
//!    will in clipboard
//!    * 2.2 when received cancelled, exit the progress
//!
//! A simple example to create a clipboard listener is following:
//!
//! ```rust, no_run
//! use wayland_clipboard_listener::WlClipboardPasteStream;
//! use wayland_clipboard_listener::WlListenType;
//!
//! fn main() {
//!     let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();
//!     // Optional: set MIME type priority
//!     // stream.set_priority(vec![
//!     //     "image/jpeg".into(),
//!     //     "text/plain;charset=utf-8".into(),
//!     // ]);
//!     for context in stream.paste_stream().flatten().flatten() {
//!         println!("{context:?}");
//!     }
//! }
//!
//! ```
//!
//! A simple example to create a wl-copy is following:
//! ``` rust, no_run
//! use wayland_clipboard_listener::{WlClipboardCopyStream, WlClipboardListenerError};
//! fn main() -> Result<(), WlClipboardListenerError> {
//!     let args = std::env::args();
//!     if args.len() != 2 {
//!         println!("You need to pass a string to it");
//!         return Ok(());
//!     }
//!     let context: &str = &args.last().unwrap();
//!     let mut stream = WlClipboardCopyStream::init()?;
//!     stream.copy_to_clipboard(context.as_bytes().to_vec(), vec!["TEXT"] ,false)?;
//!     Ok(())
//! }

#![allow(clippy::needless_doctest_main)]

mod constvar;
mod dispatch;
use std::io::Read;

use wayland_client::{protocol::wl_seat, Connection, EventQueue};

use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1,
};

use std::sync::{Arc, Mutex};

use thiserror::Error;

use constvar::{IMAGE, TEXT};

/// listentype
/// if ListenOnHover, it wll be useful for translation apps, but in dispatch, we cannot know the
/// mime_types, it can only handle text
///
/// ListenOnCopy will get the full mimetype, but you should copy to enable the listen,
#[derive(Debug)]
pub enum WlListenType {
    ListenOnSelect,
    ListenOnCopy,
}

/// Error
/// it describe three kind of error
/// 1. failed when init
/// 2. failed in queue
/// 3. failed in pipereader
#[derive(Error, Debug)]
pub enum WlClipboardListenerError {
    #[error("Init Failed")]
    InitFailed(String),
    #[error("Error during queue")]
    QueueError(String),
    #[error("PipeError")]
    PipeError,
}

/// context
/// here describe two types of context
/// 1. text, just [String]
/// 2. file , with [`Vec<u8>`]
#[derive(Debug)]
pub enum ClipBoardListenContext {
    Text(String),
    File(Vec<u8>),
}

#[derive(Debug)]
pub struct ClipBoardListenMessage {
    pub mime_types: Vec<String>,
    pub context: ClipBoardListenContext,
}

/// Paste stream
/// it is used to handle paste event
pub struct WlClipboardPasteStream {
    inner: WlClipboardListenerStream,
}

impl WlClipboardPasteStream {
    /// init a paste steam, you can use WlListenType::ListenOnSelect to watch the select event
    /// It can just listen on text
    /// use ListenOnCopy will receive the mimetype, can copy many types
    pub fn init(listentype: WlListenType) -> Result<Self, WlClipboardListenerError> {
        Ok(Self {
            inner: WlClipboardListenerStream::init(listentype)?,
        })
    }

    /// return a steam, to iter
    /// ```rust, no_run
    /// use wayland_clipboard_listener::WlClipboardPasteStream;
    /// use wayland_clipboard_listener::WlListenType;
    ///
    /// let mut stream = WlClipboardPasteStream::init(WlListenType::ListenOnCopy).unwrap();
    ///
    /// for context in stream.paste_stream().flatten() {
    ///     println!("{context:?}")
    /// }
    /// ```
    pub fn paste_stream(&mut self) -> &mut WlClipboardListenerStream {
        &mut self.inner
    }

    ///  just get the clipboard once
    pub fn get_clipboard(
        &mut self,
    ) -> Result<Option<ClipBoardListenMessage>, WlClipboardListenerError> {
        self.inner.get_clipboard()
    }

    ///  set MIME type priority
    pub fn set_priority(&mut self, val: Vec<String>) {
        self.inner.set_priority = Some(val);
    }
}

/// copy stream,
/// it can used to make a wl-copy
pub struct WlClipboardCopyStream {
    inner: WlClipboardListenerStream,
}

impl WlClipboardCopyStream {
    /// init a copy steam, you can use it to copy some files
    pub fn init() -> Result<Self, WlClipboardListenerError> {
        Ok(Self {
            inner: WlClipboardListenerStream::init(WlListenType::ListenOnCopy)?,
        })
    }

    /// it will run a never end loop, to handle the paste event, like what wl-copy do
    /// it will live until next copy event happened
    /// you need to pass data and if use useprimary to it,
    /// if is useprimary, you can use the middle button of mouse to paste
    /// Take [primary-selection](https://patchwork.freedesktop.org/patch/257267/) as reference
    /// ``` rust, no_run
    /// use wayland_clipboard_listener::{WlClipboardCopyStream, WlClipboardListenerError};
    /// let args = std::env::args();
    /// if args.len() != 2 {
    ///     println!("You need to pass a string to it");
    /// } else {
    ///     let context: &str = &args.last().unwrap();
    ///     let mut stream = WlClipboardCopyStream::init().unwrap();
    ///     stream.copy_to_clipboard(context.as_bytes().to_vec(), vec!["STRING"], false).unwrap();
    /// }
    ///```
    pub fn copy_to_clipboard(
        &mut self,
        data: Vec<u8>,
        mimetypes: Vec<&str>,
        useprimary: bool,
    ) -> Result<(), WlClipboardListenerError> {
        self.inner.copy_to_clipboard(data, mimetypes, useprimary)
    }
}
/// Stream, provide a iter to listen to clipboard
/// Note, the iter will loop very fast, you would better to use thread sleep
/// or iter you self
pub struct WlClipboardListenerStream {
    listentype: WlListenType,
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<String>,
    data_manager: Option<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1>,
    data_device: Option<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1>,
    mime_types: Vec<String>,
    set_priority: Option<Vec<String>>,
    pipereader: Option<os_pipe::PipeReader>,
    queue: Option<Arc<Mutex<EventQueue<Self>>>>,
    copy_data: Option<Vec<u8>>,
    copy_cancelled: bool,
}

impl Iterator for WlClipboardListenerStream {
    type Item = Result<Option<ClipBoardListenMessage>, WlClipboardListenerError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.get_clipboard())
    }
}

impl WlClipboardListenerStream {
    /// private init
    /// to init a stream
    fn init(listentype: WlListenType) -> Result<Self, WlClipboardListenerError> {
        let conn = Connection::connect_to_env().map_err(|_| {
            WlClipboardListenerError::InitFailed("Cannot connect to wayland".to_string())
        })?;

        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let display = conn.display();

        display.get_registry(&qhandle, ());
        let mut state = WlClipboardListenerStream {
            listentype,
            seat: None,
            seat_name: None,
            data_manager: None,
            data_device: None,
            mime_types: Vec::new(),
            set_priority: None,
            pipereader: None,
            queue: None,
            copy_data: None,
            copy_cancelled: false,
        };

        event_queue.blocking_dispatch(&mut state).map_err(|e| {
            WlClipboardListenerError::InitFailed(format!("Inital dispatch failed: {e}"))
        })?;

        if !state.device_ready() {
            return Err(WlClipboardListenerError::InitFailed(
                "Cannot get seat and data manager".to_string(),
            ));
        }

        while state.seat_name.is_none() {
            event_queue.roundtrip(&mut state).map_err(|_| {
                WlClipboardListenerError::InitFailed("Cannot roundtrip during init".to_string())
            })?;
        }

        state.set_data_device(&qhandle);
        state.queue = Some(Arc::new(Mutex::new(event_queue)));
        Ok(state)
    }

    /// copy data to stream
    /// pass [Vec<u8>] as data
    /// now it can just copy text
    /// It will always live in the background, so you need to handle it yourself
    fn copy_to_clipboard(
        &mut self,
        data: Vec<u8>,
        mimetypes: Vec<&str>,
        useprimary: bool,
    ) -> Result<(), WlClipboardListenerError> {
        let eventqh = self.queue.clone().unwrap();
        let mut event_queue = eventqh.lock().unwrap();
        let qh = event_queue.handle();
        let manager = self.data_manager.as_ref().unwrap();
        let source = manager.create_data_source(&qh, ());
        let device = self.data_device.as_ref().unwrap();

        for mimetype in mimetypes {
            source.offer(mimetype.to_string());
        }

        if useprimary {
            device.set_primary_selection(Some(&source));
        } else {
            device.set_selection(Some(&source));
        }

        self.copy_data = Some(data);
        while !self.copy_cancelled {
            event_queue
                .blocking_dispatch(self)
                .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        }
        self.copy_data = None;
        self.copy_cancelled = false;
        Ok(())
    }

    /// get data from clipboard for once
    /// it is also used in iter
    fn get_clipboard(
        &mut self,
    ) -> Result<Option<ClipBoardListenMessage>, WlClipboardListenerError> {
        // get queue, start blocking_dispatch for first loop
        let queue = self.queue.clone().unwrap();
        let mut queue = queue
            .lock()
            .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        queue
            .blocking_dispatch(self)
            .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        if self.pipereader.is_some() {
            // roundtrip to init pipereader
            queue
                .roundtrip(self)
                .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
            let mut read = self.pipereader.as_ref().unwrap();
            if self.is_text() {
                let mut context = String::new();
                read.read_to_string(&mut context)
                    .map_err(|_| WlClipboardListenerError::PipeError)?;
                self.pipereader = None;
                let mime_types = self.mime_types.clone();
                self.mime_types.clear();
                Ok(Some(ClipBoardListenMessage {
                    mime_types,
                    context: ClipBoardListenContext::Text(context),
                }))
            } else {
                let mut context = vec![];
                read.read_to_end(&mut context)
                    .map_err(|_| WlClipboardListenerError::PipeError)?;
                self.pipereader = None;
                let mime_types = self.mime_types.clone();
                self.mime_types.clear();
                // it is hover type, it will not receive the context
                if let WlListenType::ListenOnSelect = self.listentype {
                    Ok(None)
                } else {
                    Ok(Some(ClipBoardListenMessage {
                        mime_types,
                        context: ClipBoardListenContext::File(context),
                    }))
                }
            }
        } else {
            Ok(None)
        }
    }

    fn device_ready(&self) -> bool {
        self.seat.is_some() && self.data_manager.is_some()
    }

    fn set_data_device(&mut self, qh: &wayland_client::QueueHandle<Self>) {
        let seat = self.seat.as_ref().unwrap();
        let manager = self.data_manager.as_ref().unwrap();
        let device = manager.get_data_device(seat, qh, ());

        self.data_device = Some(device);
    }

    fn is_text(&self) -> bool {
        !self.mime_types.is_empty()
            && self.mime_types.contains(&TEXT.to_string())
            && !self.mime_types.contains(&IMAGE.to_string())
    }
}
