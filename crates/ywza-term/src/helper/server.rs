use super::protocol::{Request, Response, HELPER_ARG};
use crate::platform;
use std::io::{Read, Write};

/// Call first in `main()` with `std::env::args().collect()`. Returns `Some(exit_code)` when
/// this process was started as a helper (the caller should exit with it), else `None`.
pub fn helper_main(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some(HELPER_ARG) {
        return None;
    }
    let mut input = String::new();
    let _ = std::io::stdin().read_to_string(&mut input);
    let resp = match serde_json::from_str::<Request>(&input) {
        Ok(req) => platform::run_request(req),
        Err(e) => Response {
            error: Some(format!("bad request: {e}")),
            ..Default::default()
        },
    };
    let ok = resp.error.is_none();
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{}", serde_json::to_string(&resp).unwrap_or_default());
    let _ = out.flush();
    Some(if ok { 0 } else { 1 })
}
