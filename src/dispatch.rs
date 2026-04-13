use super::WlClipboardListenerStream;

use std::fs::File;
use std::io::Write;
use std::os::fd::AsFd;

use wayland_client::{
    event_created_child,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy,
};

use os_pipe::pipe;
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1, ext_data_control_manager_v1, ext_data_control_offer_v1,
    ext_data_control_source_v1,
};

use crate::{
    constvar::{IMAGE, TEXT},
    WlListenType,
};

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
                == ext_data_control_manager_v1::ExtDataControlManagerV1::interface().name
            {
                state.data_manager = Some(
                    registry.bind::<ext_data_control_manager_v1::ExtDataControlManagerV1, _, _>(
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

impl Dispatch<ext_data_control_manager_v1::ExtDataControlManagerV1, ()>
    for WlClipboardListenerStream
{
    fn event(
        _state: &mut Self,
        _proxy: &ext_data_control_manager_v1::ExtDataControlManagerV1,
        _event: <ext_data_control_manager_v1::ExtDataControlManagerV1 as Proxy>::Event,
        _data: &(),
        _conn: &wayland_client::Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_data_control_device_v1::ExtDataControlDeviceV1, ()>
    for WlClipboardListenerStream
{
    fn event(
        state: &mut Self,
        _proxy: &ext_data_control_device_v1::ExtDataControlDeviceV1,
        event: <ext_data_control_device_v1::ExtDataControlDeviceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        qh: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_device_v1::Event::DataOffer { id } => {
                state
                    .offer_mime_types
                    .entry(id.id().protocol_id())
                    .or_default();
                if let WlListenType::ListenOnSelect = state.listentype {
                    if state.copy_data.is_some() {
                        return;
                    }
                    let (read, write) = pipe().unwrap();
                    state.current_type = Some(TEXT.to_string());
                    id.receive(TEXT.to_string(), write.as_fd());
                    drop(write);
                    state.pipereader = Some(read);
                }
            }
            ext_data_control_device_v1::Event::Finished => {
                state.clear_offers();
                if let Some(device) = state.data_device.take() {
                    device.destroy();
                }
                state.set_data_device(qh);
            }
            ext_data_control_device_v1::Event::PrimarySelection { id } => {
                state.replace_primary_selection_offer(id);
            }
            ext_data_control_device_v1::Event::Selection { id } => {
                state.replace_selection_offer(id.clone());
                // if is copying, not run this
                if state.copy_data.is_some() {
                    return;
                }
                let Some(offer) = id else {
                    return;
                };
                // TODO: how can I handle the mimetype?
                let select_mimetype = |state: &WlClipboardListenerStream| {
                    if state.is_text() || state.mime_types.is_empty() {
                        TEXT.to_string()
                    } else {
                        state.mime_types[0].clone()
                    }
                };
                if let WlListenType::ListenOnCopy = state.listentype {
                    // if priority is set
                    let mimetype = if let Some(val) = &state.set_priority {
                        val.iter()
                            .find(|i| state.mime_types.contains(i))
                            .cloned()
                            .unwrap_or_else(|| select_mimetype(state))
                    } else {
                        select_mimetype(state)
                    };
                    state.current_type = Some(mimetype.clone());
                    let (read, write) = pipe().unwrap();
                    offer.receive(mimetype, write.as_fd());
                    drop(write);
                    state.pipereader = Some(read);
                }
            }
            _ => {
                log::info!("unhandled event: {event:?}");
            }
        }
    }
    event_created_child!(WlClipboardListenerStream, ext_data_control_device_v1::ExtDataControlDeviceV1, [
        ext_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (ext_data_control_offer_v1::ExtDataControlOfferV1, ())
    ]);
}

impl Dispatch<ext_data_control_source_v1::ExtDataControlSourceV1, ()>
    for WlClipboardListenerStream
{
    fn event(
        state: &mut Self,
        _proxy: &ext_data_control_source_v1::ExtDataControlSourceV1,
        event: <ext_data_control_source_v1::ExtDataControlSourceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        match event {
            ext_data_control_source_v1::Event::Send { fd, mime_type } => {
                let Some(data) = state.copy_data.as_ref() else {
                    return;
                };
                // FIXME: how to handle the mime_type?
                if mime_type == TEXT || mime_type == IMAGE {
                    let mut f = File::from(fd);
                    f.write_all(&data.to_vec()).unwrap();
                }
            }
            ext_data_control_source_v1::Event::Cancelled => state.copy_cancelled = true,
            _ => {
                eprintln!("unhandled event: {event:?}");
            }
        }
    }
}

impl Dispatch<ext_data_control_offer_v1::ExtDataControlOfferV1, ()> for WlClipboardListenerStream {
    fn event(
        state: &mut Self,
        _proxy: &ext_data_control_offer_v1::ExtDataControlOfferV1,
        event: <ext_data_control_offer_v1::ExtDataControlOfferV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        if let ext_data_control_offer_v1::Event::Offer { mime_type } = event {
            state
                .offer_mime_types
                .entry(_proxy.id().protocol_id())
                .or_default()
                .push(mime_type);
        }
    }
}
