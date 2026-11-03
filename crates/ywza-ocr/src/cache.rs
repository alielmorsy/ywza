use crate::{Engine, Image, TextLine};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

/// Small LRU keyed by image hash.
struct Cache {
    cap: usize,
    tick: u64,
    map: HashMap<u64, (u64, Vec<TextLine>)>,
}

impl Cache {
    fn new(cap: usize) -> Self {
        Cache {
            cap,
            tick: 0,
            map: HashMap::new(),
        }
    }

    fn get(&mut self, k: u64) -> Option<Vec<TextLine>> {
        self.tick += 1;
        let t = self.tick;
        self.map.get_mut(&k).map(|e| {
            e.0 = t;
            e.1.clone()
        })
    }

    fn put(&mut self, k: u64, v: Vec<TextLine>) {
        self.tick += 1;
        if !self.map.contains_key(&k) && self.map.len() >= self.cap {
            if let Some(old) = self.map.iter().min_by_key(|(_, e)| e.0).map(|(k, _)| *k) {
                self.map.remove(&old);
            }
        }
        self.map.insert(k, (self.tick, v));
    }
}

fn global() -> &'static Mutex<Cache> {
    static C: OnceLock<Mutex<Cache>> = OnceLock::new();
    C.get_or_init(|| Mutex::new(Cache::new(32)))
}

pub(crate) fn get(key: u64) -> Option<Vec<TextLine>> {
    global().lock().unwrap().get(key)
}

pub(crate) fn put(key: u64, v: Vec<TextLine>) {
    global().lock().unwrap().put(key, v)
}

pub(crate) fn key(img: &Image, engine: Engine) -> u64 {
    // DefaultHasher::new() has fixed keys: deterministic within a build.
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (img.width, img.height, engine).hash(&mut h);
    img.rgba.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;

    fn line(t: &str) -> TextLine {
        TextLine {
            text: t.into(),
            rect: Rect::default(),
            words: vec![],
        }
    }

    #[test]
    fn cache_is_lru() {
        let mut c = Cache::new(2);
        c.put(1, vec![line("a")]);
        c.put(2, vec![line("b")]);
        assert!(c.get(1).is_some()); // 2 is now oldest
        c.put(3, vec![line("c")]);
        assert!(c.get(2).is_none());
        assert_eq!(c.get(1).unwrap()[0].text, "a");
        assert!(c.get(3).is_some());
    }

    #[test]
    fn key_depends_on_pixels_and_engine() {
        let a = Image {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 255],
        };
        let b = Image {
            rgba: vec![1, 0, 0, 255],
            ..a.clone()
        };
        assert_eq!(key(&a, Engine::Auto), key(&a.clone(), Engine::Auto));
        assert_ne!(key(&a, Engine::Auto), key(&b, Engine::Auto));
        assert_ne!(key(&a, Engine::Auto), key(&a, Engine::Paddle));
    }
}
