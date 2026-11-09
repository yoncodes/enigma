use anyhow::Result;
use muipserver::GmResponse;

use crate::net::app::AppState;

use super::{catalog::MaterialKind, editor, list_players, status};
use grant::grant;
use workflow::{complete_guides, grant_hero_upgrade_materials, unlock_bgms, unlock_dungeon};

pub(super) mod grant;
pub(super) mod push;
pub(super) mod workflow;

pub(super) async fn run_command(
    state: &'static AppState,
    player_id: i64,
    command: &str,
) -> Result<GmResponse> {
    let parts = command.split_whitespace().collect::<Vec<_>>();
    let Some(first) = parts.first() else {
        anyhow::bail!("command is required");
    };

    let args = match first.to_ascii_lowercase().as_str() {
        "achievement" | "achievements" => {
            return editor::achievements(state, player_id, &parts[1..]).await;
        }
        "background" | "backgrounds" => {
            return editor::backgrounds(state, player_id, &parts[1..]).await;
        }
        "bgm" => return unlock_bgms(state, player_id, &parts[1..]).await,
        "dungeon" => return unlock_dungeon(state, player_id, &parts[1..]).await,
        "garment" | "garments" => {
            return editor::garments(state, player_id, &parts[1..]).await;
        }
        "guide" | "guides" => return complete_guides(state, player_id, &parts[1..]).await,
        "hero"
            if parts
                .get(1)
                .is_some_and(|arg| arg.eq_ignore_ascii_case("upgrade")) =>
        {
            return grant_hero_upgrade_materials(state, player_id, &parts[1..]).await;
        }
        "hero" => return editor::hero(state, player_id, &parts[1..]).await,
        "psychube" | "psychubes" => {
            return editor::psychube(state, player_id, &parts[1..]).await;
        }
        "material" | "reward" | "give" | "add" => &parts[1..],
        kind if MaterialKind::parse(kind).is_some() => &parts[..],
        "status" => return Ok(status(state).await),
        "players" | "list" | "listplayers" | "list_players" => return Ok(list_players(state)),
        "help" | "?" => {
            return Ok(GmResponse::ok(
                "commands: help, status, players, achievement unlock all, background unlock all, bgm unlock all, garment unlock all, guide complete all, hero level <hero id> <level>, hero resonance <hero id> <level>, hero portrait <hero id> <1-5>, hero portrait all, hero upgrade materials <hero id> <1-180> <resonance 1-15> [destiny rank] [destiny stone id], psychube max <uid>, dungeon unlock <stage|chapter> <id>, material <type> <id> <amount>",
            ));
        }
        _ => anyhow::bail!("unknown command '{}'", first),
    };

    grant(state, player_id, args).await
}
