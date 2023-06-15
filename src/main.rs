use wayland_client::{
    event_created_child,
    protocol::{wl_registry, wl_seat},
    Connection, Dispatch, Proxy,
};

use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1, zwlr_data_control_manager_v1, zwlr_data_control_offer_v1,
    zwlr_data_control_source_v1,
};

fn main() {
    let conn = Connection::connect_to_env().unwrap();

    let mut event_queue = conn.new_event_queue();
    let qhandle = event_queue.handle();

    let display = conn.display();

    display.get_registry(&qhandle, ());
    let mut state = State {
        seat: None,
        seat_name: None,
        data_manager: None,
        data_device: None,
    };

    event_queue.blocking_dispatch(&mut state).unwrap();

    if !state.device_ready() {
        eprintln!("Cannot get seat and data maanger");
        return;
    }

    while state.seat_name.is_none() {
        event_queue.roundtrip(&mut state).unwrap();
    }

    println!("get seat name: {}", state.seat_name.as_ref().unwrap());

    state.set_data_device(&qhandle);

    loop {
        if let Err(e) = event_queue.roundtrip(&mut state) {
            println!("error: {e}");
            break;
        };
    }
}

struct State {
    seat: Option<wl_seat::WlSeat>,
    seat_name: Option<String>,
    data_manager: Option<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1>,
    data_device: Option<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1>,
}

impl State {
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
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
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

impl Dispatch<wl_seat::WlSeat, ()> for State {
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

impl Dispatch<zwlr_data_control_manager_v1::ZwlrDataControlManagerV1, ()> for State {
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

impl Dispatch<zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_data_control_device_v1::ZwlrDataControlDeviceV1,
        event: <zwlr_data_control_device_v1::ZwlrDataControlDeviceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
        println!("{:?}", event);
        if let zwlr_data_control_device_v1::Event::DataOffer { id } = event {
            println!("{:?}", id);
        }
    }
    event_created_child!(State, zwlr_data_control_device_v1::ZwlrDataControlDeviceV1, [
        zwlr_data_control_device_v1::EVT_DATA_OFFER_OPCODE => (zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ())
    ]);
}
impl Dispatch<zwlr_data_control_source_v1::ZwlrDataControlSourceV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_data_control_source_v1::ZwlrDataControlSourceV1,
        _event: <zwlr_data_control_source_v1::ZwlrDataControlSourceV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<zwlr_data_control_offer_v1::ZwlrDataControlOfferV1, ()> for State {
    fn event(
        _state: &mut Self,
        _proxy: &zwlr_data_control_offer_v1::ZwlrDataControlOfferV1,
        _event: <zwlr_data_control_offer_v1::ZwlrDataControlOfferV1 as Proxy>::Event,
        _data: &(),
        _conn: &Connection,
        _qhandle: &wayland_client::QueueHandle<Self>,
    ) {
    }
}
