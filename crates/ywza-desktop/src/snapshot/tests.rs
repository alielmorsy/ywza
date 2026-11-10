use super::classify::classify;
use super::*;
use crate::{Rect, WindowInfo};

const WIN: Rect = Rect {
    x: 0,
    y: 0,
    w: 1200,
    h: 800,
};

fn raw(role: &'static str, name: &str, key: i32) -> Raw {
    Raw {
        role,
        name: name.into(),
        rect: Rect::new(10, 10 + key * 30, 100, 20),
        enabled: true,
        key: vec![42, key],
        ..Raw::default()
    }
}

fn window() -> WindowInfo {
    WindowInfo {
        id: 3,
        title: "Settings".into(),
        process: "notepad.exe".into(),
        pid: 1,
        rect: WIN,
        monitor: 1,
        minimized: false,
        hidden: false,
    }
}

fn snap(raws: Vec<Raw>, prev: Option<&Snapshot>) -> Snapshot {
    Snapshot::build(
        window(),
        raws.into_iter().map(|r| (r, None)).collect(),
        prev,
    )
}

fn sample() -> Vec<Raw> {
    let mut edit = raw("edit", "Name", 3);
    edit.patterns.value = true;
    edit.value = Some("report.txt".into());
    let mut check = raw("checkbox", "Auto-update", 4);
    check.patterns.toggle = true;
    check.toggle = Some(0);
    let mut cancel = raw("button", "Cancel", 2);
    cancel.enabled = false;
    vec![raw("button", "Save", 1), cancel, edit, check]
}

#[test]
fn render_format() {
    let s = snap(sample(), None);
    assert_eq!(
        s.render(),
        "window w3 \"Settings\" (notepad.exe) 1200x800 mon 1 snapshot 1\n\
         [1] button \"Save\"\n\
         [2] button \"Cancel\" disabled\n\
         [3] edit \"Name\" = \"report.txt\"\n\
         [4] checkbox \"Auto-update\" = off\n"
    );
}

#[test]
fn filter_rules() {
    let k = |r: &Raw| classify(r, &WIN).map(|e| e.kind);
    // Patterns upgrade generic control types.
    let mut pane = raw("pane", "", 1);
    assert_eq!(k(&pane), None);
    pane.patterns.invoke = true;
    assert_eq!(k(&pane), Some("clickable"));
    pane.patterns.toggle = true;
    assert_eq!(k(&pane), Some("toggle"));
    let mut custom = raw("custom", "x", 1);
    custom.patterns.value = true;
    assert_eq!(k(&custom), Some("editable"));
    custom.patterns.value_readonly = true;
    assert_eq!(k(&custom), None);
    custom.patterns.expand = true;
    assert_eq!(k(&custom), Some("expandable"));
    // Specific interactive types keep their name.
    let mut button = raw("button", "OK", 1);
    button.patterns.invoke = true;
    assert_eq!(k(&button), Some("button"));
    // Web content: focusable group/text.
    let mut group = raw("group", "Card", 1);
    assert_eq!(k(&group), None);
    group.focusable = true;
    assert_eq!(k(&group), Some("maybe-clickable"));
    // Informative only with a name.
    assert_eq!(k(&raw("text", "Hello", 1)), Some("text"));
    assert_eq!(k(&raw("text", "  ", 1)), None);
    // Visibility.
    let mut off = raw("button", "Hidden", 1);
    off.offscreen = true;
    assert_eq!(k(&off), None);
    let mut outside = raw("button", "Far", 1);
    outside.rect = Rect::new(5000, 5000, 10, 10);
    assert_eq!(k(&outside), None);
    let mut empty = raw("button", "Zero", 1);
    empty.rect.w = 0;
    assert_eq!(k(&empty), None);
}

#[test]
fn naming_fallback() {
    let mut b = raw("button", "", 7);
    b.automation_id = "btnSettings".into();
    let e = classify(&b, &WIN).unwrap();
    assert_eq!(e.line(), "[0] button (id=btnSettings)");
    b.help = "Open settings".into();
    assert_eq!(
        classify(&b, &WIN).unwrap().line(),
        "[0] button \"Open settings\""
    );
    let mut doc = raw("document", "Text editor", 1);
    doc.patterns.value = true;
    doc.value = Some("a\r\nb\r".into());
    assert_eq!(
        classify(&doc, &WIN).unwrap().line(),
        r#"[0] document "Text editor" = "a\nb""#
    );
}

#[test]
fn dropdown_marked() {
    let mut r = raw("radio", "Select", 1);
    r.patterns = Patterns {
        invoke: true,
        select: true,
        expand: true,
        ..Patterns::default()
    };
    assert_eq!(
        classify(&r, &WIN).unwrap().line(),
        "[0] radio \"Select\" +menu"
    );
    let mut tree = raw("pane", "Folder", 2);
    tree.patterns.expand = true; // kind already says it
    assert_eq!(
        classify(&tree, &WIN).unwrap().line(),
        "[0] expandable \"Folder\""
    );
}

#[test]
fn redundant_label_text_dropped() {
    let mut t = raw("text", "Save", 1);
    t.rect = Rect::new(20, 45, 50, 10); // inside the "Save" button
    t.key = vec![9];
    let mut tab = raw("tabitem", "a.txt. Unmodified.", 3);
    tab.rect.w = 300;
    let mut tab_text = raw("text", "a.txt", 4);
    tab_text.rect = Rect::new(20, 102, 60, 10);
    tab_text.key = vec![10];
    let s = snap(
        vec![
            raw("button", "Save", 1),
            t,
            raw("text", "Other", 2),
            tab,
            tab_text,
        ],
        None,
    );
    let names: Vec<_> = s.elements.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["Save", "Other", "a.txt. Unmodified."]);
}

#[test]
fn ids_carry_over_and_diff() {
    let old = snap(sample(), None);
    let mut raws = sample();
    raws.remove(1); // Cancel gone
    raws[1].value = Some("new.txt".into()); // edit value changed
    raws[0].enabled = false; // Save disabled
    let mut dialog = raw("dialog", "Save as", 9);
    dialog.rect = Rect::new(100, 100, 400, 300);
    raws.push(dialog);
    let new = snap(raws, Some(&old));
    assert_eq!(new.seq, 2);
    assert_eq!(new.get(3).unwrap().name, "Name"); // ID kept
    assert_eq!(
        diff(&old, &new),
        "-[2]\n~[1] button \"Save\" disabled\n~[3] = \"new.txt\"\n+[5] dialog \"Save as\"\n"
    );
    assert_eq!(diff(&new, &new), "(no changes)\n");
}

#[test]
fn needs_ocr_counts_named_interactive() {
    let mut s = snap(
        vec![
            raw("button", "A", 1),
            raw("button", "B", 2),
            raw("text", "Label", 3),
        ],
        None,
    );
    assert!(needs_ocr(&s)); // text is informative, not interactive
    s = snap(
        vec![
            raw("button", "A", 1),
            raw("button", "B", 2),
            raw("link", "C", 3),
        ],
        None,
    );
    assert!(!needs_ocr(&s));
    let mut unnamed = raw("button", "", 4);
    unnamed.automation_id = "x".into();
    s = snap(
        vec![raw("button", "A", 1), raw("button", "B", 2), unnamed],
        None,
    );
    assert!(needs_ocr(&s)); // unnamed does not count
    merge_ocr(
        &mut s,
        vec![("Page text".into(), Rect::new(10, 700, 100, 20))],
    );
    assert!(needs_ocr(&s)); // ocr lines do not count
    assert!(needs_ocr(&snap(vec![], None)));
}

#[test]
fn popup_elements_may_leave_the_window() {
    let mut item = raw("menuitem", "Open", 1);
    item.rect = Rect::new(1100, 700, 300, 20); // sticks out of the 1200x800 window
    assert_eq!(classify(&item, &WIN).map(|e| e.popup), Some(false));
    item.rect = Rect::new(1300, 700, 100, 20); // entirely outside
    assert!(classify(&item, &WIN).is_none());
    item.bounds = Some(Rect::new(1250, 650, 300, 200));
    assert_eq!(classify(&item, &WIN).map(|e| e.popup), Some(true));
}

#[test]
fn same_content_ignores_seq() {
    let a = snap(sample(), None);
    let b = snap(sample(), Some(&a));
    assert!(a.same_content(&b) && b.seq == 2);
    let mut r = sample();
    r[2].value = Some("x".into());
    assert!(!a.same_content(&snap(r, Some(&a))));
}

#[test]
fn ocr_merge_overlap() {
    let mut s = snap(sample(), None);
    let pane = Element {
        id: 5,
        kind: "clickable",
        name: String::new(),
        rect: WIN,
        ..s.elements[0].clone()
    };
    s.elements.push(pane);
    let save = s.elements[0].rect; // (10,40) 100x20
    merge_ocr(
        &mut s,
        vec![
            ("Save".into(), Rect::new(save.x + 5, save.y + 2, 60, 16)), // inside button: dropped
            ("Half".into(), Rect::new(save.x + 60, save.y, 100, 20)),   // 40% overlap: kept
            ("Last synced 2 min ago".into(), Rect::new(10, 700, 300, 20)), // only in unnamed pane: kept
            ("  ".into(), Rect::new(10, 750, 30, 20)),
        ],
    );
    let ocr: Vec<_> = s
        .elements
        .iter()
        .filter(|e| e.kind == "ocr")
        .map(|e| e.line())
        .collect();
    assert_eq!(
        ocr,
        ["[6] ocr \"Half\"", "[7] ocr \"Last synced 2 min ago\""]
    );
}

#[test]
fn recheck_rules() {
    let s = snap(sample(), None);
    let save = &s.elements[0];
    let mut r = raw("button", "Save", 1);
    assert_eq!(recheck(save, &r, &WIN, &WIN), Ok(()));
    // Window moved with the element: still fine.
    let moved_win = Rect { x: 300, ..WIN };
    let mut shifted = r.clone();
    shifted.rect.x += 300;
    assert_eq!(recheck(save, &shifted, &WIN, &moved_win), Ok(()));
    r.rect.y += 50;
    assert!(recheck(save, &r, &WIN, &WIN)
        .unwrap_err()
        .starts_with("moved"));
    let mut renamed = raw("button", "Save all", 1);
    assert!(recheck(save, &renamed, &WIN, &WIN)
        .unwrap_err()
        .starts_with("name now"));
    renamed.enabled = false;
    assert_eq!(recheck(save, &renamed, &WIN, &WIN), Err("disabled".into()));
}
