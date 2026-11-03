use super::console::Console;
use crate::helper::{Info, Request, Response};
use crate::keys::parse_keys;

pub(crate) fn run_request(req: Request) -> Response {
    let mut r = Response::default();
    let res: Result<(), String> = (|| {
        match req {
            Request::Read { pid, lines } => r.text = Some(Console::attach(pid)?.read(lines)?),
            Request::Write { pid, text, enter } => {
                Console::attach(pid)?.write_text(&text, enter)?
            }
            Request::Keys { pid, keys } => {
                let keys = parse_keys(&keys).map_err(|e| e.0)?;
                Console::attach(pid)?.send_keys(&keys)?
            }
            Request::Info { pids } => {
                for pid in pids {
                    let mut i = Info {
                        pid,
                        ..Default::default()
                    };
                    match Console::attach(pid) {
                        Ok(c) => {
                            i.hwnd = c.hwnd();
                            i.title = c.title();
                            i.procs = c.process_list();
                            i.procs.retain(|&p| p != std::process::id());
                        }
                        Err(e) => i.error = Some(e),
                    }
                    r.infos.push(i);
                }
            }
        }
        Ok(())
    })();
    r.error = res.err();
    r
}
