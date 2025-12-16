@echo off
cargo build --release
copy ".\target\release\sorceries_incantations_keybinds.dll" "C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\mod_dll\sorceries-incantations-keybinds\"