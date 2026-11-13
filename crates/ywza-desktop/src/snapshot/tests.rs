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
fn big_diff_collapses() {
    let many = |base: i32, n: i32| {
        (0..n).map(move |i| Raw {
            rect: Rect::new(10, 10, 100, 20),
            ..raw("button", "B", base + i)
        })
    };
    let old = snap(many(100, 30).collect(), None);
    let new = snap(many(200, 25).collect(), Some(&old));
    let d = diff(&old, &new);
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(lines[0], "30 removed, 25 added (sample; get_elements lists all)");
    assert_eq!(lines[1], "-[1] -[2] -[3] -[4] -[5] -[6] -[7] -[8] …");
    assert_eq!(lines[2], "+[31] button \"B\"");
    assert_eq!(lines.len(), 2 + 12 + 1);
    assert_eq!(lines[14], "+… 13 more added");
    // Small changes stay listed in full.
    let few = snap(many(100, 28).collect(), Some(&old));
    assert_eq!(diff(&old, &few), "-[29]\n-[30]\n");
}

#[test]
fn unnamed_clickables_borrow_labels() {
    let click = |key: i32, rect: Rect| {
        let mut r = raw("pane", "", key);
        r.patterns.invoke = true;
        r.rect = rect;
        r
    };
    let mut item = raw("listitem", "Job Board", 1);
    item.rect = Rect::new(10, 40, 300, 24);
    let hit = click(2, Rect::new(12, 42, 20, 20)); // follows the named row it overlaps
    let row = click(3, Rect::new(10, 100, 300, 24)); // wraps the option text
    let circle = click(4, Rect::new(12, 102, 20, 20)); // left of the option text
    let mut text = raw("text", "Immediately", 5);
    text.rect = Rect::new(40, 104, 100, 16);
    let lone = click(6, Rect::new(10, 300, 20, 20));
    let s = snap(vec![item, hit, row, circle, text, lone], None);
    let lines: Vec<String> = s.elements.iter().map(|e| e.line()).collect();
    assert_eq!(
        lines,
        [
            "[1] listitem \"Job Board\"",
            "[2] clickable \"Job Board\"",
            "[3] clickable \"Immediately\"",
            "[4] clickable \"Immediately\"",
            "[5] clickable",
        ]
    );
    // The live node is still unnamed: not a reason to call it stale.
    let mut live = click(2, Rect::new(12, 42, 20, 20));
    assert_eq!(recheck(&s.elements[1], &live, &WIN, &WIN), Ok(()));
    live.name = "Other".into();
    assert!(recheck(&s.elements[1], &live, &WIN, &WIN).is_err());
}

#[test]
fn state_flags_and_groups() {
    let mut yes = raw("radio", "Yes", 1);
    yes.patterns.select = true;
    yes.selected = true;
    yes.group = "Are you 18 or older?".into();
    let mut spin = raw("spinner", "Day", 2);
    spin.value = Some("1".into());
    let mut list = raw("listitem", "Loading", 3);
    list.busy = true;
    let s = snap(vec![yes, spin, list], None);
    let lines: Vec<String> = s.elements.iter().map(|e| e.line()).collect();
    assert_eq!(
        lines,
        [
            "[1] radio \"Yes\" in \"Are you 18 or older?\" selected",
            "[2] spinner \"Day\" = \"1\"",
            "[3] listitem \"Loading\" busy",
        ]
    );
    let mut raws = sample();
    raws[3].selected = true;
    let old = snap(sample(), None);
    assert_eq!(
        diff(&old, &snap(raws, Some(&old))),
        "~[4] checkbox \"Auto-update\" = off selected\n"
    );
}

#[test]
fn page_only_skips_browser_chrome() {
    let mut tab = raw("tabitem", "Inbox", 1);
    tab.rect = Rect::new(0, 0, 200, 30);
    let mut doc = raw("document", "Inbox", 2);
    doc.rect = Rect::new(0, 100, 1200, 700);
    let mut next = raw("button", "Next", 3);
    next.rect = Rect::new(500, 600, 80, 30);
    let mut menu = raw("menuitem", "Copy", 4);
    menu.rect = Rect::new(0, 0, 100, 20);
    menu.bounds = Some(Rect::new(0, 0, 100, 100));
    let s = snap(vec![tab, doc, next, menu], None);
    let page = s.render_page();
    assert!(!page.contains("tabitem") && page.contains("Next") && page.contains("Copy"));
    assert!(s.render().contains("tabitem"));
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
