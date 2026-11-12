//! What a client of an agent needs: paired-device files, connecting, the handshake and pairing.
//! Used by `ywza` and the desktop app.

mod connect;
mod devices;
mod pair;

pub use connect::{ask, bind_devices, close, dial, open_device, open_local, resolve, shut};
pub use devices::{
    device_file, devices_dir, list_devices, load_device, relay_url, remove_device,
    valid_device_name,
};
pub use pair::pair;
