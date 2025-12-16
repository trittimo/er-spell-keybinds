@echo off
cargo build --release
copy ".\target\release\spell_keybinds.dll" "C:\Program Files (x86)\Steam\steamapps\common\ELDEN RING\Game\mod_dll\spell-keybinds\"