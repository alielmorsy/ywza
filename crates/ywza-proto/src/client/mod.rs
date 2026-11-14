//! What a client of an agent needs: paired-device files, connecting, the handshake and pairing.
//! Used by `ywza` and the desktop app.

mod call;
mod connect;
pub mod copy;
mod devices;
mod pair;

pub use call::{call_tool, fanout_section, parse_targets, ToolReply};
pub use connect::{ask, bind_devices, close, dial, open_device, open_local, resolve, shut};
pub use devices::{
    device_file, devices_dir, list_devices, load_device, relay_url, remove_device,
    valid_device_name,
};
pub use pair::pair;
