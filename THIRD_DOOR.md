# The Three Doors

This branch starts from official Enigma `main` at `d3b1eaadcc9d83a06450f978635cf42d1b206845`. It ports the exploration implementation from `stokes1235/enigma` commit `6b8246109c139795d0192483b8e2fc4fa99bdc9c` without importing the unrelated activity, tower, or battle changes in that commit.

The server stores map state, items and encounter state per player. It handles map entry and resume, movement, chests, archives, runes, pressure plates, rocks, spikes, pipes, light puzzles, bonus scenes, map exits and exploration battles where supported by the ported implementation. Embedded layout data covers maps 101–103, 201–203 and 301–303. The presence of a layout does not imply that every object or puzzle on the map is implemented: unsupported effects and mechanics are rejected rather than treated as completed. Pipe and light interactions, in particular, are currently limited to maps 201 and 301 respectively.

The normal server setup and matching game-data requirements in the main README still apply. Migrations 097–099 add exploration state; use a database compatible with the official migration history, and back it up before changing server versions. The existing expanded `stokes1235/enigma` main branch includes additional migrations and functionality and is not interchangeable with this branch merely because both use the same game client.

A legacy profile containing only the initial map unlock can be initialized; older summary-only progress without detailed map state is preserved but cannot be resumed automatically.

Tests include map routes, router interactions and per-player persistence. They do not replace a complete in-client playthrough of all maps.
