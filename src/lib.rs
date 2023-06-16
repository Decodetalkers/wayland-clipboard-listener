mod constvar;
mod dispatch;
use std::io::Read;

use wayland_client::{protocol::wl_seat, Connection, EventQueue};

use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1,
};

use std::sync::{Arc, Mutex};

use thiserror::Error;

use constvar::TEXT;

#[derive(Error, Debug)]
pub enum WlClipboardListenerError {
    #[error("Init Failed")]
    InitFailed(String),
    #[error("Error during queue")]
    QueueError(String),
    #[error("PipeError")]
    PipeError,
}

pub struct WlClipboardListenerStream {
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<String>,
    data_manager: Option<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1>,
    data_device: Option<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1>,
    mime_types: Vec<String>,
    pipereader: Option<os_pipe::PipeReader>,
    queue: Option<Arc<Mutex<EventQueue<Self>>>>,
}

impl Iterator for WlClipboardListenerStream {
    type Item = Result<Option<String>, WlClipboardListenerError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.get_clipboard())
    }
}

impl WlClipboardListenerStream {
    pub fn init() -> Result<Self, WlClipboardListenerError> {
        let conn = Connection::connect_to_env().map_err(|_| {
            WlClipboardListenerError::InitFailed("Cannot connect to wayland".to_string())
        })?;

        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let display = conn.display();

        display.get_registry(&qhandle, ());
        let mut state = WlClipboardListenerStream {
            seat: None,
            seat_name: None,
            data_manager: None,
            data_device: None,
            mime_types: Vec::new(),
            pipereader: None,
            queue: None,
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

    fn state_queue(&mut self) -> Result<(), WlClipboardListenerError> {
        let queue = self.queue.clone().unwrap();
        let mut queue = queue
            .lock()
            .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        queue
            .roundtrip(self)
            .map_err(|e| WlClipboardListenerError::QueueError(e.to_string()))?;
        Ok(())
    }

    fn get_clipboard(&mut self) -> Result<Option<String>, WlClipboardListenerError> {
        self.state_queue()?;
        if self.pipereader.is_some() {
            self.state_queue()?;
            let mut read = self.pipereader.as_ref().unwrap();
            let mut context = String::new();
            read.read_to_string(&mut context)
                .map_err(|_| WlClipboardListenerError::PipeError)?;
            self.pipereader = None;
            Ok(Some(context))
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
        let source = manager.create_data_source(qh, ());
        let device = manager.get_data_device(seat, qh, ());
        device.set_selection(Some(&source));

        self.data_device = Some(device);
    }

    fn is_text(&self) -> bool {
        !self.mime_types.is_empty() && self.mime_types.contains(&TEXT.to_string())
    }
}
