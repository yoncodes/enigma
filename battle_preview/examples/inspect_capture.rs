use std::{env, fs, path::PathBuf};

use anyhow::{Context, bail};

fn main() -> anyhow::Result<()> {
    let mut args = env::args_os().skip(1);
    let path = PathBuf::from(args.next().context("usage: inspect_capture PATH BUFF_ID")?);
    let buff_id = args
        .next()
        .context("usage: inspect_capture PATH BUFF_ID")?
        .to_string_lossy()
        .parse::<i64>()
        .context("BUFF_ID must be an integer")?;
    if args.next().is_some() {
        bail!("usage: inspect_capture PATH BUFF_ID");
    }

    let mut value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
    battle_preview::expand_compressed_fight_steps(&mut value)?;
    inspect(&value, buff_id);
    Ok(())
}

fn inspect(value: &serde_json::Value, buff_id: i64) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Array(effects)) = object.get("actEffect") {
                for (index, effect) in effects.iter().enumerate() {
                    if effect
                        .get("buff")
                        .and_then(|buff| buff.get("buffId"))
                        .and_then(serde_json::Value::as_i64)
                        == Some(buff_id)
                    {
                        let start = index.saturating_sub(1);
                        let end = (index + 4).min(effects.len());
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&effects[start..end]).unwrap()
                        );
                    }
                }
            }
            object.values().for_each(|child| inspect(child, buff_id));
        }
        serde_json::Value::Array(values) => {
            values.iter().for_each(|child| inspect(child, buff_id));
        }
        _ => {}
    }
}
