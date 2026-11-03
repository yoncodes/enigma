use anyhow::{Result, bail, ensure};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Token<'a> {
    Name(&'a str),
    Literal(Value),
    Symbol(u8),
}

pub(super) fn read(input: &str) -> Result<Vec<Token<'_>>> {
    let bytes = input.as_bytes();
    let mut offset = 0;
    let mut tokens = Vec::new();
    while offset < bytes.len() {
        let start = offset;
        match bytes[offset] {
            value if value.is_ascii_whitespace() => offset += 1,
            b'-' if bytes.get(offset + 1) == Some(&b'-') => {
                while offset < bytes.len() && bytes[offset] != b'\n' {
                    offset += 1;
                }
            }
            b'"' => {
                offset += 1;
                let mut closed = false;
                while offset < bytes.len() {
                    match bytes[offset] {
                        b'\\' => offset += 2,
                        b'"' => {
                            offset += 1;
                            closed = true;
                            break;
                        }
                        _ => offset += 1,
                    }
                }
                ensure!(closed, "unterminated map string at byte {start}");
                tokens.push(Token::Literal(serde_json::from_str(&input[start..offset])?));
            }
            value if value.is_ascii_digit() || value == b'-' => {
                offset += 1;
                while offset < bytes.len()
                    && matches!(
                        bytes[offset],
                        b'0'..=b'9' | b'.' | b'e' | b'E' | b'+' | b'-'
                    )
                {
                    offset += 1;
                }
                tokens.push(Token::Literal(serde_json::from_str(&input[start..offset])?));
            }
            value if value.is_ascii_alphabetic() || value == b'_' => {
                offset += 1;
                while offset < bytes.len()
                    && (bytes[offset].is_ascii_alphanumeric() || bytes[offset] == b'_')
                {
                    offset += 1;
                }
                tokens.push(Token::Name(&input[start..offset]));
            }
            value @ (b'{' | b'}' | b'[' | b']' | b'=' | b',') => {
                tokens.push(Token::Symbol(value));
                offset += 1;
            }
            _ => bail!("unsupported map syntax at byte {offset}"),
        }
    }
    Ok(tokens)
}
