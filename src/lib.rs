mod mapper;

use crate::mapper::{map_key, map_modifier};

use ini::ini;

use pelite::{
    pattern,
    pe32::headers::SectionHeader,
    pe64::{Pe, PeObject, PeView},
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    ptr::read_unaligned,
    time::{Duration, Instant},
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{HMODULE, MAX_PATH},
        System::LibraryLoader::{
            GetModuleFileNameW,
            GetModuleHandleExW,
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS,
            GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
        },
    },
};

use eldenring::{
    cs::{CSTaskGroupIndex, CSTaskImp, GameDataMan, HudType, WorldChrMan},
    fd4::FD4TaskData,
    util::system::wait_for_system_init,
};

use fromsoftware_shared::{program::Program, task::*, FromStatic};

use device_query::{DeviceQuery, DeviceState, Keycode};
use keyboard_codes::{parse_input, Modifier, Shortcut};

const GAME_DATA_MAN_PATTERN_STR: &str = "48 8B 05 ? ? ? ? 48 85 C0 74 05 48 8B 40 58 C3 C3";

const OFFSET: usize = 3;
const ADDITIONAL: usize = 7;
const DEFAULT_CYCLE_DEBOUNCE_MILLISECONDS: u64 = 200;

#[derive(Clone, Copy)]
enum Action {
    SetMemorySlot(u8),
    CycleBack,
    CycleForward,
    NoOp,
}

#[derive(Clone)]
pub struct OtherSettings {
    pub cycle_debouce_milliseconds: u64
}

fn get_pe_view() -> PeView<'static> {
    let pe_view = match Program::current() {
        Program::Mapping(mapping) => mapping,
        Program::File(file) => PeView::from_bytes(file.image()).unwrap()
    };

    pe_view
}

fn get_text_header(pe: PeView<'_>) -> &SectionHeader {
    let text_header = match pe
        .section_headers()
        .iter()
        .find(|h| h.name() == Ok(".text"))
    {
        Some(h) => { h }
        None => { panic!() }
    };
    text_header
}

fn get_game_data_man() -> &'static mut GameDataMan {
    let pattern = match pattern::parse(GAME_DATA_MAN_PATTERN_STR) {
        Ok(p) => p,
        Err(_) => { panic!() }
    };

    let pe = get_pe_view();
    let text_header = get_text_header(pe);

    let scanner = pe.scanner();

    let mut rva = [0; 8];
    let mut matches = scanner.matches(&*pattern, text_header.file_range());

    let game_data_man = loop {
        if !matches.next(&mut rva) {
            panic!()
        }

        let rva = rva[0] as usize;

        let resolved_va = unsafe {
            let aob_va = pe.image().as_ptr().add(rva);

            let offset_value = read_unaligned(aob_va.add(OFFSET) as *const i32);

            let resolved_va = aob_va.add(ADDITIONAL).offset(offset_value as isize);

            resolved_va
        };

        let pointer: *const *mut GameDataMan = resolved_va as *const *mut GameDataMan;
        let game_data_man_ptr: *mut GameDataMan = unsafe { *pointer };
        break unsafe { &mut *game_data_man_ptr };
    };
    game_data_man
}

fn get_dll_path() -> String {
    unsafe {
        let mut module = HMODULE::default();

        let addr = get_dll_path as *const () as *const u16;

        GetModuleHandleExW(
            GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
            PCWSTR(addr),
            &mut module,
        ).expect("GetModuleHandleExW failed");

        let mut buf = [0u16; MAX_PATH as usize];
        let len = GetModuleFileNameW(Some(module), &mut buf);
        let path = String::from_utf16_lossy(&buf[..len as usize]);

        Path::new(&path)
            .parent()
            .unwrap()
            .to_string_lossy()
            .to_string()
    }
}

fn config_key_to_action(key: &String) -> Action {
    match key.strip_prefix("memory_slot_") {
        Some(s) => {
            let slot: u8 = s.parse().unwrap();
            Action::SetMemorySlot(slot)
        }
        None => {
            if key.contains("cycle_back") {
                return Action::CycleBack;
            } else if key.contains("cycle_forward") {
                return Action::CycleForward;
            }
            Action::NoOp
        }
    }
}

fn read_keybinds_config(config: &HashMap<String, HashMap<String, Option<String>>>) -> HashMap<Shortcut, Action> {
    let config: HashMap<Shortcut, Action> = config["keybinds"].iter()
        .map(|(k, v)| { (k, parse_input(&v.clone().unwrap_or(String::new()))) })
        .filter(|kv| kv.1.is_ok())
        .map(|(k, v)| (k, v.unwrap()))
        .map(|(k, v)| { (config_key_to_action(k), v) })
        .filter(|(action, _)| !matches!(action, Action::NoOp))
        .map(|(k, v)| (v, k))
        .collect();

    config
}

fn read_other_settings_config(config: &HashMap<String, HashMap<String, Option<String>>>) -> OtherSettings {
    let Some(settings_map) = config.get("settings") else {
        return OtherSettings { cycle_debouce_milliseconds: DEFAULT_CYCLE_DEBOUNCE_MILLISECONDS };
    };
    OtherSettings {
        cycle_debouce_milliseconds: settings_map.get("cycle_debounce_milliseconds")
                                                .and_then(|v| v.as_deref())
                                                .and_then(|s| s.parse::<u64>().ok())
                                                .unwrap_or(DEFAULT_CYCLE_DEBOUNCE_MILLISECONDS)
    }
}

// fn set_equipment_slot(game_data_man: &mut GameDataMan, slot_index: u8) {
//     let equip_inventory_ptr = game_data_man.main_player_game_data.equipment.equip_inventory_data.as_ptr();
//     let equipped_inventory = unsafe { &mut *equip_inventory_ptr };
//     let last_slot = equipped_inventory
// }

fn set_memory_slot(game_data_man: &mut GameDataMan, slot_index: u8) {
    let equipped_magic_ptr = game_data_man.main_player_game_data.equipment.equip_magic_data.as_ptr();
    let equipped_magic = unsafe { &mut *equipped_magic_ptr };

    let last_slot = equipped_magic.entries.iter()
        .filter(|e| e.param_id > 1)
        .count() - 1;

    if slot_index > last_slot as u8 {
        equipped_magic.selected_slot = last_slot as i32;
        return;
    }

    equipped_magic.selected_slot = slot_index as i32;
}

fn forward_cycle_memory_slot(game_data_man: &mut GameDataMan) {
    let equipped_magic_ptr = game_data_man.main_player_game_data.equipment.equip_magic_data.as_ptr();
    let equipped_magic = unsafe { &mut *equipped_magic_ptr };

    let last_slot: i32 = (equipped_magic.entries.iter()
        .filter(|e| e.param_id > 1)
        .count() - 1)
        .try_into()
        .unwrap();

    let next_slot = equipped_magic.selected_slot + 1;

    if next_slot > last_slot {
        equipped_magic.selected_slot = 0 as i32;
        return;
    }

    equipped_magic.selected_slot = next_slot;
}

fn back_cycle_memory_slot(game_data_man: &mut GameDataMan) {
    let equipped_magic_ptr = game_data_man.main_player_game_data.equipment.equip_magic_data.as_ptr();
    let equipped_magic = unsafe { &mut *equipped_magic_ptr };

    let previous_slot = equipped_magic.selected_slot - 1;

    if previous_slot < 0 {
        let last_slot = equipped_magic.entries.iter()
            .filter(|e| e.param_id > 1)
            .count() - 1;
        equipped_magic.selected_slot = last_slot as i32;
        return;
    }

    equipped_magic.selected_slot = previous_slot;
}

fn cartesian_product(keycodes: Vec<HashSet<Keycode>>) -> Vec<HashSet<Keycode>> {
    if keycodes.is_empty() {
        return vec![HashSet::new()];
    }
    let set = &keycodes[0];
    let cartesian = cartesian_product(keycodes[1..].to_vec());

    let mut product = Vec::new();

    for keycode in set {
        for set in &cartesian {
            let mut current_set = set.clone();
            current_set.insert(*keycode);
            product.push(current_set);
        }
    }

    product
}

fn expand_combinations(key: Keycode, modifiers: Vec<Modifier>, action: Action) -> Vec<(HashSet<Keycode>, Action)> {
    if modifiers.is_empty() {
        let mut set = HashSet::new();
        set.insert(key);
        return vec![(set, action)];
    }
    let mod_keycodes = modifiers.iter()
        .map(|m| {
            let mut set = HashSet::new();

            match map_modifier(m) {
                (base_modifier, None) => {
                    set.insert(base_modifier);
                }
                (base_modifier, Some(add_modifier)) => {
                    set.insert(base_modifier);
                    set.insert(add_modifier);
                }
            }
            set
        })
        .collect::<Vec<HashSet<Keycode>>>();

    let mut key_keycode = HashSet::new();
    key_keycode.insert(key);

    let mut joined_keycodes = mod_keycodes.clone();
    joined_keycodes.push(key_keycode);

    cartesian_product(joined_keycodes)
        .into_iter()
        .map(|p| (p, action))
        .collect()
}

fn is_all_keybinding_keys_pressed(keybindings: &HashSet<Keycode>, pressed_keys: &Vec<Keycode>) -> bool {
    for key in keybindings {
        if !pressed_keys.contains(key) {
            return false;
        }
    }
    true
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn DllMain(_hmodule: u64, reason: u32) -> bool {
    if reason != 1 {
        return true;
    }

    std::thread::spawn(|| {
        wait_for_system_init(&Program::current(), Duration::MAX)
            .expect("Timeout waiting for system init");

        let device_state = DeviceState::new();

        let config = ini!(&(get_dll_path() + "\\spell_keybinds.ini"));
        let other_settings_config = read_other_settings_config(&config);
        let keybinds_config = read_keybinds_config(&config);

        let mut last_cycle_back_run = Instant::now();
        let mut last_cycle_forward_run = Instant::now();
        let cycle_rebound = Duration::from_millis(other_settings_config.cycle_debouce_milliseconds);

        let mut last_hud_update_run = Instant::now();
        let hud_update_rebound = Duration::from_secs(3);
        let mut player_hud_type = None;
        let mut is_hud_restored = true;

        let cs_task = unsafe { CSTaskImp::instance().unwrap() };

        let mut keybindings = keybinds_config.iter()
                    .map(|(s, a)| (s.key, s.modifiers.clone(), a.clone()))
                    .map(|(k, m, a)| (map_key(&k), m, a))
                    .filter(|kma| kma.0.is_some())
                    .map(|(k, m, a)| (k.unwrap(), m, a))
                    .flat_map(|(k, m, a)| expand_combinations(k, m, a))
                    .collect::<Vec<(HashSet<Keycode>, Action)>>();

        keybindings.sort_by(|(x, _), (y, _)| y.len().cmp(&x.len()));

        cs_task.run_recurring(
            move |_: &FD4TaskData| {
                let Some(main_player) = unsafe { WorldChrMan::instance() }
                    .ok()
                    .and_then(|wcm| wcm.main_player.as_mut())
                else {
                    return
                };

                let game_data_man = get_game_data_man();

                if main_player.chr_ins.module_container.data.hp <= 0 {
                    return;
                }

                if player_hud_type.is_none() {
                    player_hud_type = Some(game_data_man.game_settings.hud_type);
                }

                let is_need_to_restore_hud = !is_hud_restored &&
                    last_hud_update_run.elapsed() > hud_update_rebound &&
                    game_data_man.game_settings.hud_type != player_hud_type.unwrap();

                if is_need_to_restore_hud {
                    game_data_man.game_settings.hud_type = player_hud_type.unwrap();
                    is_hud_restored = true;
                }

                let pressed_keys = device_state.get_keys();
                let mut is_cycling = false;

                for (keybinds, action) in &keybindings {
                    if !is_all_keybinding_keys_pressed(&keybinds, &pressed_keys) {
                        continue;
                    }
                    match action {
                        Action::SetMemorySlot(slot) => {
                            game_data_man.game_settings.hud_type = HudType::On;
                            last_hud_update_run = Instant::now();
                            is_hud_restored = false;

                            set_memory_slot(game_data_man, slot - 1);
                        }
                        Action::CycleBack => {
                            is_cycling = true;
                            if last_cycle_back_run.elapsed() < cycle_rebound {
                                return;
                            }
                            game_data_man.game_settings.hud_type = HudType::On;
                            last_hud_update_run = Instant::now();
                            is_hud_restored = false;

                            back_cycle_memory_slot(game_data_man);
                            last_cycle_back_run = Instant::now();
                        }
                        Action::CycleForward => {
                            is_cycling = true;
                            if last_cycle_forward_run.elapsed() < cycle_rebound {
                                return;
                            }
                            game_data_man.game_settings.hud_type = HudType::On;
                            last_hud_update_run = Instant::now();
                            is_hud_restored = false;

                            forward_cycle_memory_slot(game_data_man);
                            last_cycle_forward_run = Instant::now();
                        }
                        Action::NoOp => { }
                    }
                    break;
                }

                if !is_cycling {
                    // When no keys are pressed, it's safe to reset the debounce so the player can press the buttons rapidly
                    last_cycle_forward_run = Instant::now() - cycle_rebound;
                    last_cycle_back_run = Instant::now() - cycle_rebound;
                }
            },
            CSTaskGroupIndex::FrameBegin,
        );
    });
    true
}
