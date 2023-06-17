use super::WlClipboardListenerStream;

use std::io::Write;
use std::{
    fs::File,
    os::fd::{AsRawFd, FromRawFd},
};

use wayland_client::{
    event_created_child,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy,
};

use os_pipe::pipe;
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1, zwlr_data_control_offer_v1,
    zwlr_data_control_source_v1,
};

use crate::{constvar::TEXT, WlListenType};

impl Dispatch<wl_registry::WlRegistry, ()> for WlClipboardListenerStream {
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

impl Dispatch<wl_seat::WlSeat, ()> for WlClipboardListenerStream {
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

impl Dispatch<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, ()>
    for WlClipboardListenerStream
{
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

impl Dispatch<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, ()>
    for WlClipboardListenerStream
{
    fn event(
        state: &mut Self,
        _proxy: &zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
        event: <zwlr_data_control_device_v1::ZwlrDataControlDeviceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            zwlr_data_control_device_v1::Event::DataOffer { id } => {
                if state.copy_data.is_some() {
                    return;
                }
                if let WlListenType::ListenOnSelect = state.listentype {
                    let (read, write) = pipe().unwrap();
                    id.receive(TEXT.to_string(), write.as_raw_fd());
                    drop(write);
                    state.pipereader = Some(read);
                }
            }
            zwlr_data_control_device_v1::Event::Finished => {
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
            }
            zwlr_data_control_device_v1::Event::PrimarySelection { id } => {
                if let Some(offer) = id {
                    offer.destroy();
                }
            }
            zwlr_data_control_device_v1::Event::Selection { id } => {
                let Some(offer) = id else {
                    return;
                };
                // if is copying, not run this
                if state.copy_data.is_some() {
                    return;
                }
                if let WlListenType::ListenOnCopy = state.listentype {
                    // TODO: how can I handle the mimetype?
                    let mimetype = if state.is_text() || state.mime_types.is_empty() {
                        TEXT.to_string()
                    } else {
                        state.mime_types[0].clone()
                    };
                    let (read, write) = pipe().unwrap();
                    offer.receive(mimetype, write.as_raw_fd());
                    drop(write);
                    state.pipereader = Some(read);
                }
            }
            _ => {
                println!("unhandled event: {:?}", event);
            }
        }
    }
    event_created_child!(WlClipboardListenerStream, zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, [
        zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ())
    ]);
}

impl Dispatch<zwlr_data_control_source_v1::ZwlrDataControlSourceV1, ()>
    for WlClipboardListenerStream
{
    fn event(
        state: &mut Self,
        _proxy: &zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
        event: <zwlr_data_control_source_v1::ZwlrDataControlSourceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let zwlr_data_control_source_v1::Event::Send { fd, mime_type } = event {
            let Some(data) = state.copy_data.as_ref() else {
                return;
            };
            // TODO: when need other type?
            if mime_type == TEXT {
                let mut f = unsafe { File::from_raw_fd(fd.as_raw_fd()) };
                f.write_all(&data.to_vec()).unwrap();
            }
        }
    }
}

impl Dispatch<zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ()>
    for WlClipboardListenerStream
{
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
