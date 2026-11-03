use crate::{Image, Recognizer, TextLine};
use anyhow::Result;

type Make = fn() -> Result<Box<dyn Recognizer>>;

/// Runs `first`; the fallback is only created when escalation is needed.
pub(crate) struct Auto {
    pub first: Box<dyn Recognizer>,
    pub fallback: Option<Make>,
}

impl Recognizer for Auto {
    fn recognize(&self, img: &Image) -> Result<Vec<TextLine>> {
        run(
            || self.first.recognize(img),
            self.fallback.map(|make| move || make()?.recognize(img)),
            !img.is_blank(),
        )
    }
}

/// Escalation rule: the first engine failed (e.g. language unavailable), or
/// returned nothing for an image that is not blank.
fn needs_escalation(first: &Result<Vec<TextLine>>, image_nonblank: bool) -> bool {
    match first {
        Err(_) => true,
        Ok(lines) => lines.is_empty() && image_nonblank,
    }
}

/// Run `first`; if the rule fires and a fallback exists, run it. A failing
/// fallback never hides the first engine's outcome.
fn run(
    first: impl FnOnce() -> Result<Vec<TextLine>>,
    fallback: Option<impl FnOnce() -> Result<Vec<TextLine>>>,
    image_nonblank: bool,
) -> Result<Vec<TextLine>> {
    let r = first();
    match fallback {
        Some(fb) if needs_escalation(&r, image_nonblank) => match fb() {
            Ok(v) if !v.is_empty() || r.is_err() => Ok(v),
            Ok(_) => r,
            Err(e) => {
                r.map_err(|first| first.context(format!("paddle fallback also failed: {e:#}")))
            }
        },
        _ => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Rect;
    use anyhow::bail;

    fn line(t: &str) -> TextLine {
        TextLine {
            text: t.into(),
            rect: Rect::default(),
            words: vec![],
        }
    }

    #[test]
    fn escalation_rule() {
        assert!(needs_escalation(&Ok(vec![]), true));
        assert!(!needs_escalation(&Ok(vec![]), false)); // blank image: empty is correct
        assert!(!needs_escalation(&Ok(vec![line("x")]), true));
        assert!(needs_escalation(
            &Err(anyhow::anyhow!("language unavailable")),
            false
        ));

        let fb = || Ok(vec![line("paddle")]);
        let none = || -> Result<Vec<TextLine>> { Ok(vec![]) };
        assert_eq!(run(none, Some(fb), true).unwrap()[0].text, "paddle");
        assert!(run(none, Some(fb), false).unwrap().is_empty());
        assert_eq!(
            run(|| Ok(vec![line("win")]), Some(fb), true).unwrap()[0].text,
            "win"
        );
        // no fallback compiled in: Windows failure surfaces
        let fail = || -> Result<Vec<TextLine>> { bail!("no language") };
        assert!(run(fail, None::<fn() -> Result<Vec<TextLine>>>, true).is_err());
        // failed fallback on empty result keeps the empty result
        let bad = || -> Result<Vec<TextLine>> { bail!("no models") };
        assert!(run(none, Some(bad), true).unwrap().is_empty());
        assert_eq!(run(fail, Some(fb), true).unwrap()[0].text, "paddle");
    }
}
