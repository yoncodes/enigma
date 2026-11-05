use crate::excel_confgen::rust_ident::{camel_to_snake, snake_to_pascal};

pub fn emit_root_module(table_names: &[String]) -> String {
    let exploration = table_names.iter().any(|name| name == "explore_scene");
    let mut lines = vec![
        "// Auto-generated module declarations".to_string(),
        "".to_string(),
    ];

    for table_name in table_names {
        lines.push(format!("pub mod {};", camel_to_snake(table_name)));
    }

    lines.push("".to_string());
    lines.push("use std::sync::{Mutex, OnceLock};".to_string());
    lines.push("".to_string());
    lines.push("pub struct GameDB {".to_string());

    for table_name in table_names {
        let struct_name = snake_to_pascal(table_name);
        let ident = camel_to_snake(table_name);
        lines.push(format!(
            "    pub {}: {}::{}Table,",
            ident, ident, struct_name
        ));
    }

    if exploration {
        lines.push("    pub explore_map: crate::exploration::MapTable,".to_string());
    }
    lines.push("}".to_string());
    lines.push("".to_string());
    lines.push("impl GameDB {".to_string());
    lines.push("    pub fn load(data_dir: &str) -> anyhow::Result<Self> {".to_string());

    for table_name in table_names {
        let struct_name = snake_to_pascal(table_name);
        let ident = camel_to_snake(table_name);
        lines.push(format!(
            "        let {} = {}::{}Table::load(",
            ident, ident, struct_name
        ));
        lines.push(format!(
            "            &format!(\"{{}}/{}.json\", data_dir)",
            table_name
        ));
        lines.push(format!(
            "        ).map_err(|e| anyhow::anyhow!(\"Failed to load {}.json: {{}}\", e))?;",
            table_name
        ));
    }

    lines.push("".to_string());
    if exploration {
        lines.push("        let explore_map = crate::exploration::MapTable::load(data_dir, &explore_scene)?;".to_string());
    }
    lines.push("        Ok(Self {".to_string());
    for table_name in table_names {
        lines.push(format!("            {},", camel_to_snake(table_name)));
    }
    if exploration {
        lines.push("            explore_map,".to_string());
    }
    lines.push("        })".to_string());
    lines.push("    }".to_string());
    lines.push("".to_string());
    lines.push("    pub fn global() -> &'static GameDB {".to_string());
    lines.push("        static DB: OnceLock<GameDB> = OnceLock::new();".to_string());
    lines.push("        DB.get_or_init(|| {".to_string());
    lines.push(
        "            Self::load(\"data\").expect(\"Failed to load game database\")".to_string(),
    );
    lines.push("        })".to_string());
    lines.push("    }".to_string());
    lines.push("}".to_string());
    lines.push("".to_string());
    lines.push("static GAME_DATA: OnceLock<GameDB> = OnceLock::new();".to_string());
    lines.push("static GAME_DATA_INIT: Mutex<()> = Mutex::new(());".to_string());
    lines.push("".to_string());
    lines.push("pub fn init(data_dir: &str) -> anyhow::Result<()> {".to_string());
    lines.push("    if GAME_DATA.get().is_some() {".to_string());
    lines.push("        return Ok(());".to_string());
    lines.push("    }".to_string());
    lines.push("".to_string());
    lines.push(
        "    let _guard = GAME_DATA_INIT.lock().unwrap_or_else(|error| error.into_inner());"
            .to_string(),
    );
    lines.push("    if GAME_DATA.get().is_some() {".to_string());
    lines.push("        return Ok(());".to_string());
    lines.push("    }".to_string());
    lines.push("".to_string());
    lines.push("    let db = GameDB::load(data_dir)?;".to_string());
    lines.push("    GAME_DATA.set(db)".to_string());
    lines.push(
        "        .map_err(|_| anyhow::anyhow!(\"Game data already initialized\"))".to_string(),
    );
    lines.push("}".to_string());
    lines.push("".to_string());
    lines.push("#[inline]".to_string());
    lines.push("pub fn get() -> &'static GameDB {".to_string());
    lines.push(
        "    GAME_DATA.get().expect(\"Game data not initialized. Call init() first.\")".to_string(),
    );
    lines.push("}".to_string());
    lines.push("".to_string());
    lines.push("#[inline]".to_string());
    lines.push("pub fn try_get() -> Option<&'static GameDB> {".to_string());
    lines.push("    GAME_DATA.get()".to_string());
    lines.push("}".to_string());

    lines.join("\n")
}
