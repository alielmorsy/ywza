//! Element actions: validate against the stored snapshot, act, re-snapshot, report.

use ywza_desktop::{self as desk, Snapshot, WindowId};

use super::{
    ocr::{check_ocr, keep_ocr, snapshot_with_ocr},
    output::{desk_err, stale, R},
    util::blocking,
    Ctx, ToolOutput,
};

/// What an element action answers on success.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Report {
    /// Settle, then the changes it caused.
    Diff,
    /// "ok: text written" (type_text verifies itself); no settle wait.
    Verified,
    /// "ok", no settle wait, no diff (inside a batch).
    Quiet,
}

/// `desk::diff` of two identical snapshots.
pub const NO_CHANGES: &str = "(no changes)\n";

/// Validates + runs an element action on the stored snapshot, re-snapshots (ids carried
/// over from the stored one, so they stay stable) and reports per `mode`. Stale: nothing
/// done, the fresh snapshot is returned instead.
pub async fn act(
    cx: &Ctx,
    w: WindowId,
    element: Option<u32>,
    mode: Report,
    f: impl FnOnce(&Snapshot) -> desk::Result<()> + Send + 'static,
) -> R {
    let ocr = cx.ocr;
    let snap = cx.snap(w).ok_or_else(|| {
        ToolOutput::err(format!("no element list for w{w}; call get_elements first"))
    })?;
    if let Some(e) = element
        .and_then(|id| snap.get(id))
        .filter(|e| e.kind == "ocr")
    {
        let (id, want, rect) = (e.id, e.name.clone(), e.rect);
        if let Err(reason) = check_ocr(w, rect, want, ocr).await {
            let fresh = refresh(cx, w, snap).await;
            return Err(stale(w, id, &reason, fresh));
        }
    }
    let had_ocr = snap.elements.iter().any(|e| e.kind == "ocr");
    let (res, old, fresh) = blocking(move || {
        let res = f(&snap);
        if res.is_ok() && mode == Report::Diff {
            // Waits until the UI stops changing; its snapshots do not carry ids over from
            // `snap`, so the one we keep is re-taken with `snap` as `prev`.
            let _ = desk::wait_settled(w, desk::SETTLE_TIMEOUT);
        }
        let fresh = desk::snapshot(w, Some(&snap));
        (res, snap, fresh)
    })
    .await;
    let Ok(mut fresh) = fresh else {
        cx.forget(w);
        return match res {
            Ok(()) => Ok(ToolOutput::ok(format!(
                "done; window w{w} is gone (closed)"
            ))),
            Err(e) => Err(desk_err(e)),
        };
    };
    if res.is_ok() && had_ocr {
        keep_ocr(w, ocr, &old, &mut fresh).await;
    }
    let out = match res {
        Ok(()) if mode == Report::Verified => Ok(ToolOutput::ok("ok: text written")),
        Ok(()) if mode == Report::Quiet => Ok(ToolOutput::ok("ok")),
        Ok(()) => {
            let d = desk::diff(&old, &fresh);
            Ok(ToolOutput::ok(if d == NO_CHANGES {
                "done; no visible change".to_string()
            } else {
                format!("done; changes:\n{d}")
            }))
        }
        Err(desk::Error::Stale { id, reason }) => Err(stale(w, id, &reason, Ok(fresh.clone()))),
        Err(e) => {
            let mut out = desk_err(e);
            // A batch reports the changes once, at its end.
            let d = desk::diff(&old, &fresh);
            if mode != Report::Quiet && d != NO_CHANGES {
                out.text.push_str(&format!("\nchanges:\n{d}"));
            }
            Err(out)
        }
    };
    cx.store(w, fresh);
    out
}

/// A fresh snapshot (with OCR when the trigger rule says so), stored as the window's latest.
async fn refresh(cx: &Ctx, w: WindowId, prev: Snapshot) -> desk::Result<Snapshot> {
    let (snap, _) = snapshot_with_ocr(w, Some(prev), false, cx.ocr).await?;
    cx.store(w, snap.clone());
    Ok(snap)
}

/// Waits for `w` to settle, stores the final snapshot and returns the diff from `base`
/// (None if the window has no stored snapshot).
pub async fn settle_diff(cx: &Ctx, w: WindowId, base: &Snapshot) -> Option<String> {
    let cur = cx.snap(w)?;
    let had_ocr = cur.elements.iter().any(|e| e.kind == "ocr");
    let r = blocking(move || {
        let _ = desk::wait_settled(w, desk::SETTLE_TIMEOUT);
        desk::snapshot(w, Some(&cur)).map(|s| (s, cur))
    })
    .await;
    let Ok((mut fresh, cur)) = r else {
        cx.forget(w);
        return Some("window closed\n".into());
    };
    if had_ocr {
        keep_ocr(w, cx.ocr, &cur, &mut fresh).await;
    }
    let d = desk::diff(base, &fresh);
    cx.store(w, fresh);
    Some(d)
}
