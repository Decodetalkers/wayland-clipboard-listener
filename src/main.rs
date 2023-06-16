use std::{io::Read, os::fd::AsRawFd};

use wayland_client::{
    event_created_child,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, EventQueue, Proxy,
};

use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1, zwlr_data_control_offer_v1,
    zwlr_data_control_source_v1,
};

use std::sync::{Arc, Mutex};

use os_pipe::pipe;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum WaylandCopyError {
    #[error("Init Failed")]
    InitFailed(String),
    #[error("Error during queue")]
    QueueError(String),
    #[error("PipeError")]
    PipeError,
}
// TODO: just support text now
const TEXT: &str = "text/plain;charset=utf-8";

fn main() {
    let stream = WaylandCopyStream::init().unwrap();

    for context in stream.flatten().flatten() {
        println!("{context}");
    }
}

struct WaylandCopyStream {
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<String>,
    data_manager: Option<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1>,
    data_device: Option<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1>,
    mime_types: Vec<String>,
    pipereader: Option<os_pipe::PipeReader>,
    queue: Option<Arc<Mutex<EventQueue<Self>>>>,
}

impl Iterator for WaylandCopyStream {
    type Item = Result<Option<String>, WaylandCopyError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.get_clipboard())
    }
}

impl WaylandCopyStream {
    pub fn init() -> Result<Self, WaylandCopyError> {
        let conn = Connection::connect_to_env()
            .map_err(|_| WaylandCopyError::InitFailed("Cannot connect to wayland".to_string()))?;

        let mut event_queue = conn.new_event_queue();
        let qhandle = event_queue.handle();

        let display = conn.display();

        display.get_registry(&qhandle, ());
        let mut state = WaylandCopyStream {
            seat: None,
            seat_name: None,
            data_manager: None,
            data_device: None,
            mime_types: Vec::new(),
            pipereader: None,
            queue: None,
        };

        event_queue
            .blocking_dispatch(&mut state)
            .map_err(|e| WaylandCopyError::InitFailed(format!("Inital dispatch failed:{e}")))?;

        if !state.device_ready() {
            return Err(WaylandCopyError::InitFailed(
                "Cannot get seat and data manager".to_string(),
            ));
        }

        while state.seat_name.is_none() {
            event_queue.roundtrip(&mut state).map_err(|_| {
                WaylandCopyError::InitFailed("Cannot roundtrip during init".to_string())
            })?;
        }

        state.set_data_device(&qhandle);
        state.queue = Some(Arc::new(Mutex::new(event_queue)));
        Ok(state)
    }

    fn state_queue(&mut self) -> Result<(), WaylandCopyError> {
        let queue = self.queue.clone().unwrap();
        let mut queue = queue
            .lock()
            .map_err(|e| WaylandCopyError::QueueError(e.to_string()))?;
        queue
            .roundtrip(self)
            .map_err(|e| WaylandCopyError::QueueError(e.to_string()))?;
        Ok(())
    }

    fn get_clipboard(&mut self) -> Result<Option<String>, WaylandCopyError> {
        self.state_queue()?;
        if self.pipereader.is_some() {
            self.state_queue()?;
            let mut read = self.pipereader.as_ref().unwrap();
            let mut context = String::new();
            read.read_to_string(&mut context)
                .map_err(|_| WaylandCopyError::PipeError)?;
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

impl Dispatch<wl_registry::WlRegistry, ()> for WaylandCopyStream {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: <wl_registry::WlRegistry as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            if interface == wl_seat::WlSeat::interface().name {
                state.seat = Some(registry.bind::<wl_seat::WlSeat, _, _>(name, version, qh, ()));
            } else if interface
                == zwlr_data_control_manager_v1::ZwlrDataControlManagerV1::interface().name
            {
                state.data_manager = Some(
                    registry.bind::<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, _, _>(
                        name,
                        version,
                        qh,
                        (),
                    ),
                );
            }
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for WaylandCopyStream {
    fn event(
        state: &mut Self,
        _proxy: &wl_seat::WlSeat,
        event: <wl_seat::WlSeat as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Name { name } = event {
            state.seat_name = Some(name);
        }
    }
}

impl Dispatch<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, ()> for WaylandCopyStream {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_data_control_manager_v1::ZwlrDataControlManagerV1,
        _event: <zwlr_data_control_manager_v1::ZwlrDataControlManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, ()> for WaylandCopyStream {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
        event: <zwlr_data_control_device_v1::ZwlrDataControlDeviceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_data_control_device_v1::Event::DataOffer { id } = event {
            if state.is_text() {
                let (read, write) = pipe().unwrap();
                id.receive(TEXT.to_string(), write.as_raw_fd());
                drop(write);
                state.pipereader = Some(read);
                state.mime_types.clear();
            }
        } else if let zwlr_data_control_device_v1::Event::Finished = event {
            let source = state
                .data_manager
                .as_ref()
                .unwrap()
                .create_data_source(qh, ());
            state
                .data_device
                .as_ref()
                .unwrap()
                .set_selection(Some(&source));
        } else if let zwlr_data_control_device_v1::Event::PrimarySelection { id: Some(offer) } =
            event
        {
            offer.destroy();
        }
    }
    event_created_child!(WaylandCopyStream, zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, [
        zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ())
    ]);
}

impl Dispatch<zwlr_data_control_source_v1::ZwlrDataControlSourceV1, ()> for WaylandCopyStream {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
        _event: <zwlr_data_control_source_v1::ZwlrDataControlSourceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        //println!("source: {event:?}");
    }
}

impl Dispatch<zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ()> for WaylandCopyStream {
    fn event(
        state: &mut Self,
        _proxy: &zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
        event: <zwlr_data_control_offer_v1::ZwlrDataControlOfferV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_data_control_offer_v1::Event::Offer { mime_type } = event {
            state.mime_types.push(mime_type);
        }
    }
}
