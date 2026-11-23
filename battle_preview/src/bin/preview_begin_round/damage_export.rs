use std::{
    fs::File,
    io::{BufWriter, Write},
    path::{Path, absolute},
};

use anyhow::{Context, bail};
use serde_json::{Value, json};
use sonettobuf::{Fight, FightRound};

pub(super) struct DamageExport(BufWriter<File>);

impl DamageExport {
    pub fn from_args(args: &mut Vec<String>) -> anyhow::Result<Option<Self>> {
        let Some(index) = args.iter().position(|arg| arg == "--damage-export") else {
            return Ok(None);
        };
        args.remove(index);
        if args.get(index).is_none_or(|arg| arg.starts_with("--")) {
            bail!("--damage-export needs a new output file");
        }
        let path = args.remove(index);
        if args.iter().any(|arg| arg == "--damage-export") {
            bail!("--damage-export can only be provided once");
        }
        let file =
            File::create_new(&path).with_context(|| format!("create damage export {path}"))?;
        let mut export = Self(BufWriter::new(file));
        export.event(json!({"kind": "header", "format": "enigma-damage", "version": 2}))?;
        Ok(Some(export))
    }

    pub fn battle(
        &mut self,
        setup: &Path,
        fight: &Fight,
        through: i32,
        resync: bool,
    ) -> anyhow::Result<()> {
        self.event(json!({
            "kind": "battle",
            "setup": absolute(setup)?,
            "battle_id": fight.battle_id,
            "episode_id": fight.episode_id,
            "fight_version": fight.version,
            "through_round": through,
            "resync": resync,
        }))
    }

    pub fn phase<T, E>(
        export: &mut Option<&mut Self>,
        phase: &str,
        round: i32,
        input: &Path,
        generated_round: impl FnOnce(&T) -> Option<&FightRound>,
        run: impl FnOnce() -> Result<T, E>,
    ) -> anyhow::Result<Result<T, E>> {
        let Some(export) = export.as_deref_mut() else {
            return Ok(run());
        };
        let (result, records) = battle::tooling::damage::collect(run)?;
        if let Ok(reply) = &result {
            let generated_round = generated_round(reply)
                .with_context(|| format!("{phase} {round} produced no generated round"))?;
            export.event(json!({
                "kind": "phase",
                "phase": phase,
                "round": round,
                "input": absolute(input)?,
                "generated_round": generated_round,
            }))?;
            for record in records {
                writeln!(export.0, "{record}")?;
            }
        }
        Ok(result)
    }

    pub fn warning(&mut self, message: &str) -> anyhow::Result<()> {
        self.event(json!({"kind": "warning", "message": message}))
    }

    pub fn finish(mut self) -> anyhow::Result<()> {
        self.event(json!({"kind": "end"}))?;
        self.0.flush()?;
        Ok(())
    }

    fn event(&mut self, value: Value) -> anyhow::Result<()> {
        serde_json::to_writer(&mut self.0, &value)?;
        writeln!(self.0)?;
        Ok(())
    }
}
