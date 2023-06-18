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
/// 2. file , with [Vec<u8>]
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
    /// ``` rust, no_run
    /// use wayland_clipboard_listener::{WlClipboardCopyStream, WlClipboardListenerError};
    /// let args = std::env::args();
    /// if args.len() != 2 {
    ///     println!("You need to pass a string to it");
    ///     return Ok(());
    /// }
    /// let context: &str = &args.last().unwrap();
    /// let mut stream = WlClipboardCopyStream::init()?;
    /// stream.copy_to_clipboard(context.as_bytes().to_vec())?;
    /// Ok(())
    ///```
    pub fn copy_to_clipboard(&mut self, data: Vec<u8>) -> Result<(), WlClipboardListenerError> {
        self.inner.copy_to_clipboard(data)
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
    fn copy_to_clipboard(&mut self, data: Vec<u8>) -> Result<(), WlClipboardListenerError> {
        let eventqh = self.queue.clone().unwrap();
        let mut event_queue = eventqh.lock().unwrap();
        let qh = event_queue.handle();
        let manager = self.data_manager.as_ref().unwrap();
        let source = manager.create_data_source(&qh, ());
        let device = self.data_device.as_ref().unwrap();
        source.offer(TEXT.to_string());
        source.offer("text/plain".to_string());
        source.offer("TEXT".to_string());
        source.offer("UTF8_STRING".to_string());
        device.set_selection(Some(&source));
        self.copy_data = Some(data);
        while !self.copy_cancelled {
            event_queue
                .blocking_dispatch(self)
                .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        }
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
