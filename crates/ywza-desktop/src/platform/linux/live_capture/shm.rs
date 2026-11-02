use crate::platform::linux::x::X;
use x11rb::connection::Connection;
use x11rb::protocol::shm;

pub(super) struct Shm {
    pub(super) seg: shm::Seg,
    pub(super) ptr: *mut libc::c_void,
    pub(super) len: usize,
}

/// A shared segment the server can write images into; None for remote servers etc.
pub(super) fn shm_attach(x: &X, len: usize) -> Option<Shm> {
    shm::query_version(&x.conn).ok()?.reply().ok()?;
    let seg = x.conn.generate_id().ok()?;
    unsafe {
        let id = libc::shmget(libc::IPC_PRIVATE, len, libc::IPC_CREAT | 0o600);
        if id < 0 {
            return None;
        }
        let ptr = libc::shmat(id, std::ptr::null(), 0);
        let mapped = ptr as isize != -1;
        let attached = mapped
            && shm::attach(&x.conn, seg, id as u32, false)
                .ok()
                .and_then(|c| c.check().ok())
                .is_some();
        // Marked for removal now so it is freed once both sides detach, even on a crash.
        libc::shmctl(id, libc::IPC_RMID, std::ptr::null_mut());
        if !attached {
            if mapped {
                libc::shmdt(ptr);
            }
            return None;
        }
        Some(Shm { seg, ptr, len })
    }
}
