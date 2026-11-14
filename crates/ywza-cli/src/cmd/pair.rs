use anyhow::Result;

/// The 6 digits, from bare digits or the `ywza://pair/<code>` form a QR carries.
pub fn parse_code(s: &str) -> Result<String, String> {
    let s = s.trim();
    let c = s.strip_prefix("ywza://pair/").unwrap_or(s);
    (c.len() == 6 && c.bytes().all(|b| b.is_ascii_digit()))
        .then(|| c.to_string())
        .ok_or_else(|| "the pairing code is 6 digits (or ywza://pair/<code>)".into())
}

pub async fn run(code: &str, name: &str) -> Result<()> {
    ywza_proto::client::pair(code, name).await?;
    println!("paired as `{name}`; use --device {name}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::parse_code;

    #[test]
    fn accepts_digits_and_uri() {
        assert_eq!(parse_code("012345").unwrap(), "012345");
        assert_eq!(parse_code("ywza://pair/012345").unwrap(), "012345");
        assert!(parse_code("ywza://pair/12345").is_err());
        assert!(parse_code("ywza://other/012345").is_err());
        assert!(parse_code("12345a").is_err());
    }
}
