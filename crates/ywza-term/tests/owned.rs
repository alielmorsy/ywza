use std::time::{Duration, Instant};
use ywza_term::TermManager;

fn wait_for(m: &TermManager, id: ywza_term::TermId, needle: &str) -> String {
    let end = Instant::now() + Duration::from_secs(15);
    loop {
        let text = m.read(id, 50).unwrap();
        if text.contains(needle) || Instant::now() > end {
            return text;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(windows)]
const ECHO: &str = "cmd /c echo hello";
#[cfg(not(windows))]
const ECHO: &str = "echo hello";
#[cfg(windows)]
const SHELL: &str = "cmd";
#[cfg(not(windows))]
const SHELL: &str = "sh";

#[test]
fn echo_hello() {
    let m = TermManager::new();
    let id = m.start(ECHO, None).unwrap();
    let text = wait_for(&m, id, "hello");
    assert!(text.contains("hello"), "got {text:?}");
    assert!(!text.contains('\x1b'));
    let end = Instant::now() + Duration::from_secs(10);
    while m.owned(id).unwrap().exit_status().is_none() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(m.owned(id).unwrap().exit_status(), Some(0));
    assert!(m.list_text().contains(&format!("[{id}]")));
}

#[test]
fn interactive_write_and_keys() {
    let m = TermManager::new();
    let id = m.start(SHELL, None).unwrap();
    m.write(id, "echo ywza-marker-one", true).unwrap();
    let text = wait_for(&m, id, "ywza-marker-one");
    assert!(text.contains("ywza-marker-one"), "got {text:?}");
    m.write(id, "echo ywza-marker-two", false).unwrap();
    m.keys(id, "enter").unwrap();
    assert!(
        wait_for(&m, id, "ywza-marker-two")
            .matches("ywza-marker-two")
            .count()
            >= 2
    );
    m.write(id, "exit", true).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while m.owned(id).unwrap().is_running() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!m.owned(id).unwrap().is_running());
}

#[test]
fn kill_and_scrollback() {
    let m = TermManager::new();
    let id = m.start(SHELL, None).unwrap();
    let t = m.owned(id).unwrap();
    // More lines than the 30-row screen: the early ones must come back from scrollback.
    #[cfg(windows)]
    m.write(id, "for /l %i in (1,1,60) do @echo line%i", true)
        .unwrap();
    #[cfg(not(windows))]
    m.write(
        id,
        "i=1; while [ $i -le 60 ]; do echo line$i; i=$((i+1)); done",
        true,
    )
    .unwrap();
    wait_for(&m, id, "line60");
    let text = m.read(id, 200).unwrap();
    assert!(
        text.contains("line1\n") && text.contains("line60"),
        "got {text:?}"
    );
    assert!(!t.raw_tail(1 << 20).is_empty());
    m.kill(id).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    while t.is_running() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!t.is_running());
}
