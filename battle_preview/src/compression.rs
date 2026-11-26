use std::io::{Cursor, Read};

use anyhow::{Context, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::read::MultiGzDecoder;
use prost::Message;
use serde_json::{Map, Value};
use sonettobuf::FightStep;

pub fn expand_compressed_fight_steps(value: &mut Value) -> anyhow::Result<()> {
    match value {
        Value::Object(map) => {
            expand_round(map)?;
            for value in map.values_mut() {
                expand_compressed_fight_steps(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                expand_compressed_fight_steps(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expand_round(map: &mut Map<String, Value>) -> anyhow::Result<()> {
    let Some(encoded) = map.get("fightStepBytes").filter(|value| !value.is_null()) else {
        return Ok(());
    };
    let compressed = match encoded {
        Value::String(encoded) => STANDARD
            .decode(encoded)
            .context("fightStepBytes is not valid base64")?,
        Value::Array(_) => serde_json::from_value(encoded.clone())
            .context("fightStepBytes is not a valid byte list")?,
        _ => bail!("fightStepBytes must be base64 or a byte list"),
    };
    if compressed.is_empty() {
        map.remove("fightStepBytes");
        return Ok(());
    }
    let mut framed = Vec::new();
    MultiGzDecoder::new(compressed.as_slice())
        .read_to_end(&mut framed)
        .context("fightStepBytes is not valid gzip")?;
    let steps = decode_fight_steps(&framed)?;
    if let Some(total) = map.get("totalStep").filter(|value| !value.is_null())
        && total.as_u64() != Some(steps.len() as u64)
    {
        bail!(
            "totalStep {total} disagrees with the {} compressed steps",
            steps.len()
        );
    }
    let serialized = serde_json::to_value(steps)?;
    if let Some(mut expanded) = map
        .get("fightStep")
        .filter(|value| !value.is_null())
        .cloned()
    {
        crate::normalize_live_json(&mut expanded);
        let expanded: Vec<FightStep> =
            serde_json::from_value(expanded).context("fightStep is not a valid step list")?;
        if !expanded.is_empty() {
            let mut decoded = serialized.clone();
            crate::normalize_live_json(&mut decoded);
            if expanded != serde_json::from_value::<Vec<FightStep>>(decoded)? {
                bail!("fightStep disagrees with fightStepBytes");
            }
        }
    }

    map.insert("fightStep".to_owned(), serialized);
    map.remove("fightStepBytes");
    map.remove("totalStep");
    Ok(())
}

fn decode_fight_steps(framed: &[u8]) -> anyhow::Result<Vec<FightStep>> {
    let mut bytes = Cursor::new(framed);
    let count = read_u32(&mut bytes).context("fightStepBytes has no step count")?;
    if count as usize > (framed.len() - 4) / 4 {
        bail!("fightStepBytes step count exceeds its frame");
    }
    let mut steps = Vec::with_capacity(count as usize);
    for index in 0..count {
        let len = read_u32(&mut bytes)
            .with_context(|| format!("fightStepBytes has no length for step {index}"))?
            as usize;
        let start = bytes.position() as usize;
        let end = start
            .checked_add(len)
            .filter(|end| *end <= framed.len())
            .with_context(|| format!("fightStepBytes step {index} exceeds its frame"))?;
        steps.push(
            FightStep::decode(&framed[start..end])
                .with_context(|| format!("fightStepBytes step {index} is invalid protobuf"))?,
        );
        bytes.set_position(end as u64);
    }
    if bytes.position() as usize != framed.len() {
        bail!("fightStepBytes contains trailing data");
    }
    Ok(steps)
}

#[cfg(test)]
mod tests;

fn read_u32(bytes: &mut Cursor<&[u8]>) -> Option<u32> {
    let mut value = [0; 4];
    bytes.read_exact(&mut value).ok()?;
    Some(u32::from_be_bytes(value))
}
