# Enigma

### Current supported version: **3.6.5 (non-Steam)**

[![Build and Release](https://github.com/yoncodes/enigma/actions/workflows/rust.yml/badge.svg)](https://github.com/yoncodes/enigma/actions/workflows/rust.yml)

---

## What is Enigma?

Enigma is a Rust server emulator for the PC version of *Reverse: 1999*.

The project is still under active development. Added support for certain game mechanics, but newer heroes, stages, and events may use things that have not been implemented yet.

---

## Quick start

1. Download the matching builds from the [Releases page](https://github.com/yoncodes/enigma/releases):

   - `gameserver`
   - `sdkserver`
   - `muipserver` (optional gm panel)

2. Extract the archives into the same folder.

3. Download [sonetto-data](https://gitlab.com/yoncodes/sonetto-data) then move excel2json to the same folder of gameserver.

```text
data/excel2json
```

4. Download [sonetto-patch](https://github.com/yoncodes/sonetto-patch), then place `launcher.exe` and `sonetto.dll` in the game folder.

5. Start the servers:

```text
sdkserver
gameserver
muipserver
```

6. Run the game through `launcher.exe`.

---

## Building from source

1. git clone https://github.com/yoncodes/enigma.git

2. Install Rust: [https://rust-lang.org/tools/install](https://rust-lang.org/tools/install)

3. create a data folder inside whatever folder you'll use to hold the gameserver.exe

4. git clone https://gitlab.com/yoncodes/sonetto-data.git

5. copy sonetto-data/excel2json to data/excel2json

6. cargo build --release -p gameserver -p sdkserver -p muipserver

The executables are written to `target/release/`. Copy `common/Config.toml` beside them as `config.toml`.

7. copy the files to the folder you created earlier

8. launch each file 

---

## How to login

Login using an email address in the game client (**DO NOT USE THE REGISTER BUTTON**, if the account doesn't exist it will be created automatically).

![login image](/images/r99-email.png)

---

## Features

- SQLite persistence with automatic migrations
- Tutorial works now
- Main story and resource-stage progression
- Server-authoritative, data-driven battle engine
- Battle cards, buffs, passives, damage, waves, summons, and replay support
- Heroes, psychubes, skins, destiny stones, talents, and teams
- Inventory, currency, rewards, shops, mail, sign-in, and gacha
- Room, buildings, manufacture, and character placement
- MUIP panel for granting resources and unlocking heroes, stages, or chapters

---

## Known limitations

- Battle parity is still being expanded for unsupported skills and mechanics.
- Battle reconnect does not restore the active fight yet.
- Assist heroes (trial heroes) dungeon selection is not implemented.
- Tower support (wip)
- Some event modes and Tower Compose settlement paths are incomplete.


---

## MUIP

Start `gameserver` before `muipserver`, then open:

```text
http://127.0.0.1:21100
```

The default token is `1999`. Change it in `config.toml` 

You can use this to get all the currency and items in the game

---

## Contributing

Open an issue first, then submit a pull request into `dev`. See
[CONTRIBUTING.md](CONTRIBUTING.md) for the contribution steps.

---

## Thanks

- Using [Luotianyi-0712](https://github.com/Luotianyi-0712) original script to auto build new updates

- [Yoshk4e](https://github.com/Yoshk4e) for helping on figuring the custom protocol needed to login on the original server

---

## Discord

Join the [Discord](https://discord.gg/CQCT2jVHZP).
