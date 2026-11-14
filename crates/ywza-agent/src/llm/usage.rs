use serde_json::Value;
use ywza_proto::{Llm, SessionEvent};

use super::Msg;

/// Tokens one 1280x800 screenshot costs a vision model.
const SCREENSHOT_TOKENS: u64 = 1500;

/// The `usage` object of one response; every field is optional because some servers omit it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct StepUsage {
    pub prompt: Option<u64>,
    pub completion: Option<u64>,
    pub cached: Option<u64>,
}

impl StepUsage {
    pub fn parse(response: &Value) -> Self {
        let u = &response["usage"];
        Self {
            prompt: u["prompt_tokens"].as_u64(),
            completion: u["completion_tokens"].as_u64(),
            cached: u["prompt_tokens_details"]["cached_tokens"].as_u64(),
        }
    }
}

/// Characters of text in the history, a stand-in for the prompt size when the server reports no usage.
pub fn history_chars(history: &[Msg]) -> usize {
    history
        .iter()
        .map(|m| {
            let v = &m.value;
            v["content"].as_str().map_or(0, str::len)
                + v.get("tool_calls").map_or(0, |c| c.to_string().len())
        })
        .sum()
}

/// Token totals of one run plus what a screenshot-based agent doing the same steps would have used:
/// it sends one screenshot per tool-call step and keeps every earlier one in its history, so for
/// request k the estimate is the actual prompt plus (tool calls before k) * SCREENSHOT_TOKENS;
/// output tokens are the same. Missing usage falls back to chars / 4.
#[derive(Debug, Default)]
pub struct RunUsage {
    price_in: Option<f64>,
    price_out: Option<f64>,
    input: u64,
    output: u64,
    cached: u64,
    requests: u32,
    tool_calls: u32,
    estimate_in: u64,
}

impl RunUsage {
    pub fn new(llm: &Llm) -> Self {
        Self {
            price_in: llm.price_in,
            price_out: llm.price_out,
            ..Self::default()
        }
    }

    pub fn add_step(
        &mut self,
        u: StepUsage,
        prompt_chars: impl FnOnce() -> usize,
        completion_chars: usize,
        calls: u32,
    ) {
        let prompt = u.prompt.unwrap_or_else(|| prompt_chars() as u64 / 4);
        let completion = u.completion.unwrap_or(completion_chars as u64 / 4);
        self.input += prompt;
        self.output += completion;
        self.cached += u.cached.unwrap_or(0);
        self.estimate_in += prompt + u64::from(self.tool_calls) * SCREENSHOT_TOKENS;
        self.requests += 1;
        self.tool_calls += calls;
    }

    fn cost(&self, input: u64) -> Option<f64> {
        let (pi, po) = (self.price_in?, self.price_out?);
        Some((input as f64 * pi + self.output as f64 * po) / 1e6)
    }

    pub fn event(&self) -> SessionEvent {
        SessionEvent::Usage {
            input: self.input,
            output: self.output,
            cached: self.cached,
            requests: self.requests,
            tool_calls: self.tool_calls,
            screenshot_estimate: self.estimate_in + self.output,
            cost: self.cost(self.input),
            screenshot_cost: self.cost(self.estimate_in),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn usage(price_in: Option<f64>, price_out: Option<f64>) -> RunUsage {
        RunUsage {
            price_in,
            price_out,
            ..RunUsage::default()
        }
    }

    fn step(p: u64, c: u64) -> StepUsage {
        StepUsage {
            prompt: Some(p),
            completion: Some(c),
            cached: Some(10),
        }
    }

    #[test]
    fn parses_usage() {
        let v = json!({"usage":{"prompt_tokens":5,"completion_tokens":2,
            "prompt_tokens_details":{"cached_tokens":3}}});
        assert_eq!(
            StepUsage::parse(&v),
            StepUsage {
                prompt: Some(5),
                completion: Some(2),
                cached: Some(3)
            }
        );
        assert_eq!(StepUsage::parse(&json!({})), StepUsage::default());
    }

    #[test]
    fn screenshot_estimate_grows_with_earlier_tool_calls() {
        let mut u = usage(Some(2.0), Some(10.0));
        u.add_step(step(1000, 100), || 0, 0, 2);
        u.add_step(step(1200, 50), || 0, 0, 1);
        u.add_step(step(1300, 20), || 0, 0, 0);
        let SessionEvent::Usage {
            input,
            output,
            cached,
            requests,
            tool_calls,
            screenshot_estimate,
            cost,
            screenshot_cost,
        } = u.event()
        else {
            panic!()
        };
        assert_eq!(
            (input, output, cached, requests, tool_calls),
            (3500, 170, 30, 3, 3)
        );
        // 1000 + (1200 + 2*1500) + (1300 + 3*1500) + output
        assert_eq!(screenshot_estimate, 1000 + 4200 + 5800 + 170);
        assert!((cost.unwrap() - (3500.0 * 2.0 + 170.0 * 10.0) / 1e6).abs() < 1e-12);
        assert!((screenshot_cost.unwrap() - (11000.0 * 2.0 + 170.0 * 10.0) / 1e6).abs() < 1e-12);
    }

    #[test]
    fn missing_usage_falls_back_to_text_length() {
        let mut u = usage(None, None);
        u.add_step(StepUsage::default(), || 4000, 80, 1);
        let SessionEvent::Usage {
            input,
            output,
            cost,
            screenshot_cost,
            ..
        } = u.event()
        else {
            panic!()
        };
        assert_eq!((input, output, cost, screenshot_cost), (1000, 20, None, None));
    }

    #[test]
    fn cost_needs_both_prices() {
        let mut u = usage(Some(1.0), None);
        u.add_step(step(10, 1), || 0, 0, 0);
        assert!(matches!(u.event(), SessionEvent::Usage { cost: None, .. }));
    }
}
