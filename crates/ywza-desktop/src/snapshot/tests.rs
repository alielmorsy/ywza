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
        focused: false,
        maximized: false,
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
        render_diff(&diff(&old, &new)),
        "-[2]\n~[1] button \"Save\" disabled\n~[3] = \"new.txt\"\n+[5] dialog \"Save as\"\n"
    );
    assert_eq!(render_diff(&diff(&new, &new)), "(no changes)\n");
}

fn el(id: u32, kind: &'static str, name: &str, key: &[i32]) -> Element {
    Element {
        id,
        kind,
        name: name.into(),
        automation_id: String::new(),
        value: None,
        enabled: true,
        selected: false,
        busy: false,
        group: String::new(),
        labeled: false,
        learned: false,
        rect: Rect::default(),
        patterns: Patterns::default(),
        key: key.to_vec(),
        popup: false,
        parent: None,
        anchor: Anchor::default(),
        handle: None,
    }
}

fn snap_of(elements: Vec<Element>) -> Snapshot {
    Snapshot {
        window: window(),
        seq: 1,
        elements,
    }
}

/// The original string-based diff, kept as the reference for `render_diff(diff(..))`.
fn old_diff(old: &Snapshot, new: &Snapshot) -> String {
    use std::fmt::Write;
    const COLLAPSE: usize = 20;
    const SAMPLE_REMOVED: usize = 8;
    const SAMPLE_ADDED: usize = 12;
    let ocr = |a: &Element, b: &Element| a.kind == "ocr" && b.kind == "ocr";
    let same = |a: &Element, b: &Element| {
        if ocr(a, b) {
            a.id == b.id
        } else if a.key.is_empty() || b.key.is_empty() {
            a.key.is_empty() && b.key.is_empty() && a.kind == b.kind && a.name == b.name
        } else {
            a.key == b.key
        }
    };
    let same_name = |a: &Element, b: &Element| {
        a.name == b.name || (ocr(a, b) && super::ocr::ocr_similar(&a.name, &b.name))
    };
    let removed: Vec<String> = old
        .elements
        .iter()
        .filter(|o| !new.elements.iter().any(|n| same(o, n)))
        .map(|o| format!("-[{}]", o.id))
        .collect();
    // In document order; true = added.
    let mut rest: Vec<(bool, String)> = Vec::new();
    for n in &new.elements {
        match old.elements.iter().find(|o| same(o, n)) {
            None => rest.push((true, format!("+{}", n.line()))),
            Some(o)
                if o.kind == n.kind
                    && same_name(o, n)
                    && o.enabled == n.enabled
                    && o.selected == n.selected
                    && o.busy == n.busy =>
            {
                if o.value != n.value {
                    rest.push((false, format!("~[{}]{}", n.id, n.value_text())));
                }
            }
            Some(_) => rest.push((false, format!("~{}", n.line()))),
        }
    }
    let r = removed.len();
    let a = rest.iter().filter(|(added, _)| *added).count();
    let mut out = String::new();
    if r > COLLAPSE || a > COLLAPSE {
        let _ = writeln!(
            out,
            "{r} removed, {a} added (sample; get_elements lists all)"
        );
    }
    if r > COLLAPSE {
        let _ = writeln!(out, "{} …", removed[..SAMPLE_REMOVED].join(" "));
    } else {
        for l in &removed {
            let _ = writeln!(out, "{l}");
        }
    }
    let mut shown = 0;
    for (added, l) in &rest {
        if *added && a > COLLAPSE {
            shown += 1;
            if shown > SAMPLE_ADDED {
                continue;
            }
        }
        let _ = writeln!(out, "{l}");
    }
    if a > COLLAPSE {
        let _ = writeln!(out, "+… {} more added", a - SAMPLE_ADDED);
    }
    if out.is_empty() {
        out.push_str("(no changes)\n");
    }
    out
}

#[test]
fn render_diff_matches_reference() {
    let base = || {
        let mut check = el(4, "checkbox", "Auto", &[1, 4]);
        check.patterns.toggle = true;
        check.value = Some("off".into());
        let mut edit = el(3, "edit", "Name", &[1, 3]);
        edit.value = Some("a.txt".into());
        vec![el(1, "button", "Save", &[1, 1]), edit, check]
    };
    let ocr = |names: &[&str]| {
        snap_of(
            names
                .iter()
                .enumerate()
                .map(|(i, n)| el(i as u32 + 1, "ocr", n, &[]))
                .collect(),
        )
    };
    let mut cases: Vec<(Snapshot, Snapshot)> = Vec::new();
    let old = snap(sample(), None);
    let mut raws = sample();
    raws.remove(1);
    raws[1].value = Some("new.txt".into());
    raws[0].enabled = false;
    cases.push((old.clone(), snap(raws, Some(&old))));
    cases.push((snap_of(vec![]), snap_of(vec![])));
    cases.push((snap_of(vec![]), snap_of(base())));
    cases.push((snap_of(base()), snap_of(vec![])));
    cases.push((ocr(&["A", "A", "B"]), ocr(&["A", "C", "B", "B"])));
    cases.push((ocr(&["A", "B"]), ocr(&["B", "A", "A"])));
    let mut toggled = base();
    toggled[2].value = Some("on".into());
    cases.push((snap_of(base()), snap_of(toggled)));
    let mut valued = base();
    valued[1].value = Some("b.txt".into());
    cases.push((snap_of(base()), snap_of(valued)));
    let mut renamed = base();
    renamed[0].name = "Store".into();
    cases.push((snap_of(base()), snap_of(renamed)));
    let mut disabled = base();
    disabled[0].enabled = false;
    disabled[1].value = None;
    cases.push((snap_of(base()), snap_of(disabled)));
    let mut dup_old = base();
    dup_old.push(el(9, "button", "Save copy", &[1, 1]));
    cases.push((snap_of(dup_old.clone()), snap_of(base())));
    cases.push((snap_of(base()), snap_of(dup_old)));
    let mut picked = base();
    picked[0].selected = true;
    picked[1].busy = true;
    picked[1].value = Some("b.txt".into());
    cases.push((snap_of(base()), snap_of(picked)));
    // OCR lines match on id; a noisy or cut-off re-read is no change, other text is.
    let (jitter, cut, other) = (
        ocr(&["File eView", "Use the current in", "Go"]),
        ocr(&["File View", "Use the current implied", "Go"]),
        ocr(&["Save", "Delete", "Stop"]),
    );
    cases.push((cut.clone(), jitter.clone()));
    cases.push((jitter.clone(), cut));
    cases.push((jitter, other));
    let mut shifted = ocr(&["A", "B"]);
    shifted.elements[0].id = 7;
    cases.push((ocr(&["A", "B"]), shifted));
    let mut valued_ocr = ocr(&["A"]);
    valued_ocr.elements[0].value = Some("v".into());
    cases.push((ocr(&["A"]), valued_ocr));
    // Page changes collapse, on either side or both.
    let many = |n: u32, key: i32| {
        snap_of(
            (0..n)
                .map(|i| el(i + 1, "button", "B", &[key, i as i32]))
                .collect(),
        )
    };
    cases.push((many(30, 1), many(25, 2)));
    cases.push((many(30, 1), many(3, 2)));
    cases.push((many(3, 1), many(25, 2)));
    cases.push((many(21, 1), many(20, 2)));
    cases.push((many(20, 1), many(21, 2)));
    cases.push((many(30, 1), many(30, 1)));
    for (a, b) in &cases {
        assert_eq!(render_diff(&diff(a, b)), old_diff(a, b));
    }
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
    let d = render_diff(&diff(&old, &new));
    let lines: Vec<&str> = d.lines().collect();
    assert_eq!(
        lines[0],
        "30 removed, 25 added (sample; get_elements lists all)"
    );
    assert_eq!(lines[1], "-[1] -[2] -[3] -[4] -[5] -[6] -[7] -[8] …");
    assert_eq!(lines[2], "+[31] button \"B\"");
    assert_eq!(lines.len(), 2 + 12 + 1);
    assert_eq!(lines[14], "+… 13 more added");
    // Small changes stay listed in full.
    let few = snap(many(100, 28).collect(), Some(&old));
    assert_eq!(render_diff(&diff(&old, &few)), "-[29]\n-[30]\n");
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
        render_diff(&diff(&old, &snap(raws, Some(&old)))),
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

#[test]
fn ocr_jitter_is_no_change() {
    let mut old = snap(vec![], None);
    merge_ocr(
        &mut old,
        vec![(
            "Use the current implied".into(),
            Rect::new(10, 700, 300, 20),
        )],
    );
    let mut new = old.clone();
    new.elements[0].name = "Use the current in".into();
    assert_eq!(render_diff(&diff(&old, &new)), "(no changes)\n");
    new.elements[0].name = "Delete".into();
    assert_eq!(
        render_diff(&diff(&old, &new)),
        format!("~{}\n", new.elements[0].line())
    );
}

fn node(role: &'static str, name: &str, aid: &str, parent: Option<usize>, y: i32, key: i32) -> Raw {
    Raw {
        role,
        name: name.into(),
        automation_id: aid.into(),
        rect: Rect::new(10, y, 100, 20),
        enabled: true,
        focusable: true,
        key: vec![key],
        parent,
        ..Raw::default()
    }
}

fn snap_in(process: &str, raws: Vec<Raw>) -> Snapshot {
    let w = WindowInfo {
        process: process.into(),
        ..window()
    };
    Snapshot::build(w, raws.into_iter().map(|r| (r, None)).collect(), None)
}

fn named<'a>(s: &'a Snapshot, name: &str) -> &'a Element {
    s.elements.iter().find(|e| e.name == name).unwrap()
}

fn top(r: &Resolve) -> u32 {
    match r {
        Resolve::Found(c) => c[0].id,
        other => panic!("not resolved: {other:?}"),
    }
}

#[test]
fn parent_skips_unlisted_ancestors() {
    let dialog = raw("dialog", "Save as", 1);
    let mut pane = raw("pane", "", 2);
    pane.parent = Some(0);
    let mut ok = raw("button", "OK", 3);
    ok.parent = Some(1);
    let outer = raw("button", "Top", 4);
    let s = snap(vec![dialog, pane, ok, outer], None);
    assert_eq!(s.elements.len(), 3);
    assert_eq!(named(&s, "Save as").parent, None);
    assert_eq!(named(&s, "OK").parent, Some(named(&s, "Save as").id));
    assert_eq!(named(&s, "Top").parent, None);
}

#[test]
fn anchor_path_and_name() {
    let group = node("group", "  Main   Toolbar ", "tb", None, 10, 1);
    let mut button = node("button", "  Save  As ", "", Some(0), 50, 2);
    button.bounds = Some(Rect::new(0, 0, 500, 500));
    let s = snap(vec![group, button], None);
    let a = &named(&s, "Save  As").anchor;
    assert_eq!(a.process, "notepad.exe");
    assert_eq!(a.role, "button");
    assert_eq!(a.name, "save as");
    assert_eq!(a.rel, Rect::new(10, 50, 100, 20));
    assert_eq!(
        a.path,
        [Step {
            role: "group",
            automation_id: "tb".into(),
            name: "main toolbar".into()
        }]
    );
}

fn form(first_y: i32, key0: i32, extra: bool) -> Vec<Raw> {
    let mut raws = Vec::new();
    if extra {
        raws.push(node("button", "New", "btnNew", None, 5, key0));
    }
    let g = raws.len();
    raws.push(node("group", "Form", "form", None, 40, key0 + 1));
    raws.push(node("button", "Save", "btnSave", Some(g), first_y, key0 + 2));
    raws.push(node("button", "Cancel", "btnCancel", Some(g), first_y + 40, key0 + 3));
    raws
}

#[test]
fn resolves_after_restart() {
    let before = snap(form(100, 10, false), None);
    let save = named(&before, "Save");
    let after = snap(form(100, 500, true), None);
    let want = named(&after, "Save").id;
    assert_ne!(save.id, want);
    let r = resolve(&save.anchor, &after);
    assert_eq!(top(&r), want);
    let Resolve::Found(c) = r else { unreachable!() };
    assert!(c[0].score > 0.99 && c.iter().all(|c| (0.0..=1.0).contains(&c.score)));
    // A small move stays within the position tolerance.
    let moved = snap(form(104, 900, true), None);
    assert_eq!(top(&resolve(&save.anchor, &moved)), named(&moved, "Save").id);
}

fn closes(aids: [&str; 2], groups: [&str; 2], y: i32) -> Vec<Raw> {
    vec![
        node("group", groups[0], "", None, 10, 1),
        node("button", "Close", aids[0], Some(0), y, 2),
        node("group", groups[1], "", None, 200, 3),
        node("button", "Close", aids[1], Some(2), y + 40, 4),
    ]
}

#[test]
fn identical_buttons_are_ambiguous_until_distinguished() {
    let anchor_of = |aids, groups| named(&snap(closes(aids, groups, 100), None), "Close").anchor.clone();
    // The layout moved, so position cannot help either.
    let b = snap(closes(["", ""], ["Tabs", "Tabs"], 300), None);
    let anchor = anchor_of(["", ""], ["Tabs", "Tabs"]);
    let Resolve::Ambiguous(c) = resolve(&anchor, &b) else {
        panic!("expected ambiguous");
    };
    assert_eq!(c.len(), 2);

    let b = snap(closes(["x1", "x2"], ["Tabs", "Tabs"], 300), None);
    let anchor = anchor_of(["x1", "x2"], ["Tabs", "Tabs"]);
    assert_eq!(top(&resolve(&anchor, &b)), named(&b, "Close").id);

    let b = snap(closes(["", ""], ["Left", "Right"], 300), None);
    let anchor = anchor_of(["", ""], ["Left", "Right"]);
    assert_eq!(top(&resolve(&anchor, &b)), named(&b, "Close").id);
}

#[test]
fn other_process_or_role_is_not_found() {
    let a = snap(form(100, 10, false), None);
    let anchor = named(&a, "Save").anchor.clone();
    let other = snap_in("calc.exe", form(100, 10, false));
    assert_eq!(resolve(&anchor, &other), Resolve::NotFound);
    let mut edit = anchor;
    edit.role = "edit";
    assert_eq!(resolve(&edit, &a), Resolve::NotFound);
}

#[test]
fn learned_label_names_unnamed_only() {
    let proc = "learned-test.exe";
    let raws = || {
        vec![
            node("button", "", "", None, 40, 1),
            node("button", "Save", "", None, 200, 2),
        ]
    };
    let anchor = snap_in(proc, raws()).elements[0].anchor.clone();
    set_learned(vec![(anchor, "Send".into())]);
    let s = snap_in(proc, raws());
    set_learned(vec![]);
    assert_eq!(s.elements[0].line(), "[1] button \"Send\" (learned)");
    assert_eq!(s.elements[1].line(), "[2] button \"Save\"");
    assert!(snap_in(proc, raws()).elements[0].name.is_empty());
}
