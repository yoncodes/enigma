//! Data-only reader for the client's constant map tables; never evaluates Lua.

use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;

mod tokens;

use tokens::Token;

pub(super) fn read(input: &str) -> Result<Vec<Value>> {
    let tokens = tokens::read(input.trim_start_matches('\u{feff}'))?;
    let mut parser = Parser {
        tokens: &tokens,
        offset: 0,
    };
    parser.expect(Token::Name("local"))?;
    let Some(Token::Name(name)) = parser.take() else {
        bail!("map constant name is missing");
    };
    parser.expect(Token::Symbol(b'='))?;
    let constants = parser.value(None)?;
    ensure!(constants.is_array(), "map constant must be an array");
    parser.expect(Token::Name("return"))?;
    let map = parser.value(Some((name, &constants)))?;
    ensure!(parser.take().is_none(), "unexpected data after map table");
    match map {
        Value::Array(values) => Ok(values),
        _ => bail!("map must be a table"),
    }
}

struct Parser<'a, 'b> {
    tokens: &'a [Token<'b>],
    offset: usize,
}

impl<'a, 'b> Parser<'a, 'b> {
    fn take(&mut self) -> Option<&'a Token<'b>> {
        let value = self.tokens.get(self.offset)?;
        self.offset += 1;
        Some(value)
    }

    fn expect(&mut self, expected: Token<'_>) -> Result<()> {
        ensure!(
            self.take() == Some(&expected),
            "expected {expected:?} in map data"
        );
        Ok(())
    }

    fn value(&mut self, constant: Option<(&str, &Value)>) -> Result<Value> {
        match self.take().context("incomplete map table")? {
            Token::Literal(value) => Ok(value.clone()),
            Token::Name("true") => Ok(Value::Bool(true)),
            Token::Name("false") => Ok(Value::Bool(false)),
            Token::Name("nil") => Ok(Value::Null),
            Token::Name(name) => {
                let (declared, values) = constant.context("unexpected map identifier")?;
                ensure!(*name == declared, "unknown map constant {name}");
                self.expect(Token::Symbol(b'['))?;
                let Some(Token::Literal(index)) = self.take() else {
                    bail!("map constant index must be an integer");
                };
                let index = index
                    .as_u64()
                    .and_then(|value| usize::try_from(value).ok())
                    .and_then(|value| value.checked_sub(1))
                    .context("invalid map constant index")?;
                self.expect(Token::Symbol(b']'))?;
                values
                    .as_array()
                    .and_then(|values| values.get(index))
                    .cloned()
                    .context("map constant index is out of bounds")
            }
            Token::Symbol(b'{') => {
                let mut values = Vec::new();
                while self.tokens.get(self.offset) != Some(&Token::Symbol(b'}')) {
                    values.push(self.value(constant)?);
                    if self.tokens.get(self.offset) == Some(&Token::Symbol(b',')) {
                        self.offset += 1;
                    } else {
                        break;
                    }
                }
                self.expect(Token::Symbol(b'}'))?;
                Ok(Value::Array(values))
            }
            other => bail!("unsupported map expression {other:?}"),
        }
    }
}

#[cfg(test)]
mod tests;
