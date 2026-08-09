# Battle extraction boundary

This document defines the boundary for extracting `battle`, `battle_check`, and
`battle_preview` from Enigma. The extraction unit owns battle configuration
adaptation, battle state, execution, diagnostics, and replay tooling. It does
not own accounts, persistence, network sessions, or server orchestration.

## Dependency direction

The allowed direction is:

```text
Enigma composition (logic / server binaries)
                    |
                    v
       dungeon input + BattleCatalog
                    |
                    v
          BattleRuntime / managers
                    |
                    v
        semantic results and packets

battle_check ------> battle <------ battle_preview
```

The three extraction crates have exact direct-dependency allowlists enforced by
`battle/tests/boundary.rs`. In particular:

- `battle` does not depend on `logic`, `database`, any server crate, or an async
  runtime.
- `battle_check` and `battle_preview` may depend on `battle`; `battle` does not
  depend on either tool.
- `battle_check` may use `battle_preview` capture and replay helpers. The
  reverse dependency, including test-only source inclusion, is forbidden.
- `config` and `protocol` are explicit adapter contracts, described below.
  They are not hidden Enigma infrastructure dependencies.

Moving the extraction unit therefore requires either moving `config` and
`protocol` with it or supplying compatible crates. It does not require moving
`logic`, `database`, `common`, or any server binary.

## Configuration ownership

`config::GameDB` is loaded at a composition boundary. Dungeon and runtime
construction each receive a `BattleCatalog` derived from that immutable
database.

`BattleCatalog` and explicit battle compiler adapters own battle-facing
interpretation of configuration, including:

- fight version and battle rules;
- skill/effect metadata and exact registry-backed definitions;
- buff definitions, features, limits, and configured command origins;
- entity, equipment, destiny, monster, toughness, and resistance metadata;
- card, cloth, teaching, device, trigger, field, Magic Circle, Impromptu, and
  Lingering Glow configuration;
- dynamic skill-catalog extension for new rules, waves, summons, and devices.

Runtime and scheduling modules consume `BattleCatalog`; they do not acquire
`GameDB` or process-global config themselves. `BattleManagers` stores the same
catalog used to seed the fight and fails loudly in production if it is absent.
Manager-owned catalogs survive cloning and runtime views.

Some public APIs predate explicit catalog injection. Their signatures and
global/fail-soft behavior remain compatibility contracts. Those wrappers
acquire `BattleCatalog::global` or `BattleCatalog::try_global` only at the API
edge and delegate to the same explicit implementation used by production.
They are not a runtime fallback. Removing them is a separate public API change.

For builders, attach transient construction inputs to the existing builder:

```rust,ignore
EntityBuilder::new(hero, position, team_type, is_sub)
    .with_catalog(catalog)
    .with_stats(stats)
    .build()
```

Do not create parallel `*_with_catalog` families. Prefer a natural domain
entry point, a passed `BattleCatalog`, or the existing builder attachment.

## Battle state and mutation ownership

`BattleRuntime` owns execution order, the current fight snapshot, deterministic
choices, the compiled skill catalog, round state, and `BattleManagers`.

`BattleManagers` is the durable mutation boundary. Skill handlers and
mechanics emit typed commands; the owning manager validates and commits them,
then returns semantic changes for events and packet projection. In particular:

- HP, buffs, cards, attributes, resources, entities, fields, summons, waves,
  and other manager state have one mutation owner;
- exact condition, behavior, and buff-act registry keys remain the only
  execution gateways;
- packets never repair state, reorder actions, select targets, or invent
  unsupported behavior;
- configuration access is read-only and cannot become a second mutation path.

The detailed parity and registry rules remain in [workflow.md](workflow.md).

## Protobuf classification

The `protocol` dependency currently serves several distinct roles. Keep them
explicit when extracting or replacing it.

| Classification | Representative types | Owner |
| --- | --- | --- |
| Input and snapshot model | `Fight`, `FightTeam`, `FightEntityInfo`, `BuffInfo`, `CardInfo`, `FightRound` | Battle construction/runtime; these are currently the canonical persisted and replayable battle snapshots. |
| Request/response transport | `StartDungeonRequest`, `StartDungeonReply`, `BeginRoundRequest`, `BeginRoundReply` | Dungeon, server composition, and preview boundaries. |
| Packet projection | `ActEffect`, effect payloads, card pushes, markers, entity and resource updates | `battle::engine::packet`, after semantic state commits. |
| Protobuf JSON compatibility | Live-capture field, enum, and wrapper normalization | `protocol`; `battle_preview` re-exports the adapter for compatibility. |
| Internal battle model | `TargetEntity`, commands, events, manager state, registry definitions, normalized catalog values | `battle`; these must not leak back into transport shaping. |

Using protobuf snapshot types inside the runtime is an intentional current
boundary, not an unnoticed infrastructure dependency. Replacing them with a
separate wire-independent snapshot model would be a later migration with its
own serialization and parity plan. It is not required to move the battle
crates out of Enigma.

New config DTOs belong in `BattleCatalog` only when they normalize data for
battle rules. New protobuf construction belongs at dungeon, preview, or packet
projection boundaries. Do not put protobuf packet shaping into catalog accessors
and do not expose raw config rows to runtime handlers.

## `battle_check` and `battle_preview`

Both tools are consumers of the same explicit battle boundary:

- `battle_check` loads config once in `main`, then passes the same `GameDB`
  through root discovery, wire evidence, and optional opening simulation. A
  catalog derived from that database is passed to closure analysis and
  coverage, and opening constructs catalogs from the same database at its
  battle boundaries. Scanner code does not initialize config or reach into
  server state.
- `battle_preview` binaries load config once, then pass that database into
  replay helpers and construct `BattleCatalog` at the battle boundary. Capture
  normalization, deterministic replay, and output rendering do not own battle
  mutation. The binaries accept `ENIGMA_BATTLE_DATA_DIR` and otherwise use the
  repository-relative data path.
- Neither crate depends on `logic`, `database`, or server crates. Missing
  private capture fixtures may limit replay coverage, but do not change the
  compile-time boundary.

## Enforcement and validation

`battle/tests/boundary.rs` enforces:

1. exact normal and target-specific dependency allowlists for all three crates;
2. no direct `config::` or raw `.game_data()` access in non-test runtime source
   files;
3. no hidden source inclusion from `battle` into `battle_preview`.

Run the boundary and semantic gates before accepting a boundary change:

```text
cargo test -p battle --test boundary
cargo test -p battle --lib
cargo test -p battle_check
cargo test -p battle_preview
cargo test -p logic battle_setup
cargo check -p protocol --all-targets
cargo check -p battle --all-targets
cargo check -p battle_check --all-targets
cargo check -p battle_preview --all-targets
cargo check -p logic --all-targets
cargo clippy -p protocol --all-targets -- -D warnings
cargo clippy -p battle --all-targets -- -D warnings
cargo clippy -p battle_check --all-targets -- -D warnings
cargo clippy -p battle_preview --all-targets -- -D warnings
cargo clippy -p logic --all-targets -- -D warnings
```

For semantic battle changes, also run the focused registry/mechanic tests and
the relevant preview/parity gates required by `docs/workflow.md`. Compile the
private capture paths even when the captures are unavailable:

```text
cargo check -p battle --features private-fixtures --all-targets
cargo check -p battle_preview --features private-fixtures --all-targets
cargo clippy -p battle --features private-fixtures --all-targets -- -D warnings
cargo clippy -p battle_preview --features private-fixtures --all-targets -- -D warnings
```

Run private-fixture tests only when those ignored captures are available.

## Extraction checklist

The boundary is extraction-ready when all of the following remain true:

- composition constructs catalogs from one immutable `GameDB` and passes a
  catalog explicitly into each dungeon and runtime construction boundary;
- live runtime, scheduler, managers, checker scans, and preview replay do not
  acquire process-global battle data below their composition edge;
- config parsing and exact registry support are centralized in battle catalog
  or compiler adapters;
- all durable mutations are committed by the owning manager;
- transport projection consumes committed semantic results;
- `battle`, `battle_check`, and `battle_preview` have no Enigma
  infrastructure dependencies outside the documented `config` and `protocol`
  contracts;
- boundary, semantic, check, and strict-clippy gates are green.

Once these conditions hold, extraction is a repository/package move rather
than another behavioral refactor.
