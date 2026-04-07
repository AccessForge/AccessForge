mod add_mod_dialog;
mod detail;
mod install_dialog;
mod tabs;

use detail::ModEntry;
use tabs::build_tab;

use crate::installer;
use crate::manifest::Manifest;
use crate::state::AppState;
use crate::path_setup;
use crate::updater;
use crate::worker::{self, ProgressMsg, TaskResult};
use anyhow::Result;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use wxdragon::dialogs::message_dialog::{MessageDialog, MessageDialogStyle};
use wxdragon::prelude::*;
use wxdragon::timer::Timer;

pub fn run(mock: bool) -> Result<()> {
    let _ = wxdragon::main(move |_| {
        let frame = Frame::builder()
            .with_title("AccessForge")
            .with_size(Size::new(800, 600))
            .build();

        let panel = Panel::builder(&frame).build();
        let main_sizer = BoxSizer::builder(Orientation::Vertical).build();

        // Status bar at top
        let status_label = StaticText::builder(&panel)
            .with_label("Loading mods...")
            .build();
        main_sizer.add(&status_label, 0, SizerFlag::Expand | SizerFlag::All, 4);

        // Notebook (tabs)
        let notebook = Notebook::builder(&panel).build();
        main_sizer.add(&notebook, 1, SizerFlag::Expand | SizerFlag::All, 4);

        // Build tabs
        let (browse, add_mod_btn) = tabs::build_tab_with_button(
            &notebook,
            "Browse",
            "Available mods",
            "Select a mod from the list to see its details.",
            "Add mod manually",
            true,
        );
        let installed = build_tab(
            &notebook,
            "Installed",
            "Installed mods",
            "Select an installed mod to see its details.",
            false,
        );
        let updates = build_tab(
            &notebook,
            "Updates",
            "Mods with updates available",
            "Select a mod to see update details.",
            false,
        );

        // --- About tab ---
        let about_panel = Panel::builder(&notebook).build();
        let about_sizer = BoxSizer::builder(Orientation::Vertical).build();

        let version_label = StaticText::builder(&about_panel)
            .with_label(&format!("AccessForge {}", updater::current_version()))
            .build();
        let update_btn = Button::builder(&about_panel)
            .with_label("Check for updates")
            .build();
        let github_btn = Button::builder(&about_panel)
            .with_label("AccessForge on GitHub")
            .build();
        let path_btn = Button::builder(&about_panel)
            .with_label("Add to PATH")
            .build();
        if path_setup::is_on_path().unwrap_or(false) {
            path_btn.enable(false);
            path_btn.set_label("Already on PATH");
        }

        about_sizer.add(&version_label, 0, SizerFlag::All, 8);
        about_sizer.add(&update_btn, 0, SizerFlag::All, 4);
        about_sizer.add(&github_btn, 0, SizerFlag::All, 4);
        about_sizer.add(&path_btn, 0, SizerFlag::All, 4);
        about_panel.set_sizer(about_sizer, true);
        notebook.add_page(&about_panel, "About", false, None);

        github_btn.on_click(move |_| {
            let _ = wxdragon::utils::launch_default_browser("https://github.com/AccessForge", wxdragon::utils::BrowserLaunchFlags::Default);
        });

        path_btn.on_click(move |_| {
            match path_setup::add_to_path() {
                Ok(()) => {
                    let dlg = MessageDialog::builder(
                        &path_btn,
                        "AccessForge has been added to your PATH. Restart your terminal for the change to take effect.",
                        "Added to PATH",
                    )
                    .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
                    .build();
                    dlg.show_modal();
                    path_btn.enable(false);
                    path_btn.set_label("Already on PATH");
                }
                Err(e) => {
                    add_mod_dialog::show_error(&path_btn, &format!("Failed to add to PATH: {e}"));
                }
            }
        });

        panel.set_sizer(main_sizer, true);

        // --- Mod data storage (shared via Rc<RefCell>) ---
        let browse_mods: Rc<RefCell<Vec<ModEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let installed_mods: Rc<RefCell<Vec<ModEntry>>> = Rc::new(RefCell::new(Vec::new()));
        let update_mods: Rc<RefCell<Vec<ModEntry>>> = Rc::new(RefCell::new(Vec::new()));

        // --- Discovery receiver ---
        let rx_cell: Rc<RefCell<Option<mpsc::Receiver<ProgressMsg>>>> =
            Rc::new(RefCell::new(None));

        // --- Spawn discovery ---
        let state = AppState::load().unwrap_or_else(|_| AppState::new());
        let rx = if mock {
            worker::spawn_discover_mock()
        } else {
            worker::spawn_discover(state)
        };
        *rx_cell.borrow_mut() = Some(rx);

        // --- Timer to poll discovery channel ---
        let timer = Rc::new(Timer::new(&frame));
        let rx_for_timer = rx_cell.clone();
        let browse_for_timer = browse_mods.clone();
        let installed_for_timer = installed_mods.clone();
        let update_for_timer = update_mods.clone();

        let browse_list = browse.list;
        let installed_list = installed.list;
        let updates_list = updates.list;

        let update_btn_for_timer = update_btn;
        let _timer_ref = timer.clone();
        timer.on_tick(move |_| {
            let _ = &_timer_ref;
            let mut borrow = rx_for_timer.borrow_mut();
            let Some(rx) = borrow.as_ref() else { return };

            for _ in 0..50 {
                match rx.try_recv() {
                    Ok(ProgressMsg::Status(s)) => {
                        status_label.set_label(&s);
                    }
                    Ok(ProgressMsg::DiscoveryStarted { .. }) => {}
                    Ok(ProgressMsg::ModLoaded(loaded)) => {
                        let entry = ModEntry(*loaded);
                        if !entry.is_installed() {
                            browse_list.append(&entry.list_label_browse());
                            browse_for_timer.borrow_mut().push(entry);
                        } else if entry.has_update() {
                            updates_list.append(&entry.list_label_update());
                            update_for_timer.borrow_mut().push(entry);
                        } else {
                            installed_list.append(&entry.list_label_installed());
                            installed_for_timer.borrow_mut().push(entry);
                        }
                    }
                    Ok(ProgressMsg::ModSkipped { .. }) => {}
                    Ok(ProgressMsg::DiscoveryFinished) => {
                        let total = browse_for_timer.borrow().len()
                            + installed_for_timer.borrow().len()
                            + update_for_timer.borrow().len();
                        let n_installed = installed_for_timer.borrow().len()
                            + update_for_timer.borrow().len();
                        let n_updates = update_for_timer.borrow().len();

                        if total == 0 {
                            status_label.set_label("No mods found.");
                        } else {
                            let update_note = if n_updates > 0 {
                                format!(", {} update(s) available", n_updates)
                            } else {
                                String::new()
                            };
                            status_label.set_label(&format!(
                                "{total} mods found, {n_installed} installed{update_note}"
                            ));
                        }
                    }
                    Ok(ProgressMsg::Done(TaskResult::Discovery)) => {
                        *borrow = None;
                        // Drop the borrow before showing modal dialogs
                        drop(borrow);
                        // Auto-check for app updates after discovery (daily)
                        if !mock {
                            let state = AppState::load().unwrap_or_else(|_| AppState::new());
                            if state.should_check_updates() {
                                check_and_apply_update(&update_btn_for_timer, false);
                            }
                        }
                        return;
                    }
                    Ok(ProgressMsg::Failed(e)) => {
                        status_label.set_label(&format!("Error: {e}"));
                        *borrow = None;
                        return;
                    }
                    Ok(_) => {}
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        *borrow = None;
                        return;
                    }
                }
            }
        });
        timer.start(50, false);

        // --- Browse tab events ---
        let browse_for_sel = browse_mods.clone();
        let browse_detail = browse.detail;
        let browse_splitter = browse.splitter;
        let browse_list_panel = browse.list_panel;
        let browse_detail_panel = browse.detail_panel;
        browse_list.on_selection_changed(move |_| {
            if let Some(idx) = browse_list.get_selection() {
                let mods = browse_for_sel.borrow();
                if let Some(entry) = mods.get(idx as usize) {
                    browse_detail.populate(entry);
                    browse_splitter.split_vertically(
                        &browse_list_panel, &browse_detail_panel, 300,
                    );
                }
            }
        });

        let browse_for_btn = browse_mods.clone();
        let installed_for_browse = installed_mods.clone();
        browse.detail.action_btn.on_click(move |_| {
            let Some(idx) = browse_list.get_selection() else { return };
            let manifest = {
                let mods = browse_for_btn.borrow();
                let Some(entry) = mods.get(idx as usize) else { return };
                entry.0.manifest.clone()
            };

            if let Some(result) = start_install(&manifest, &browse.detail.action_btn, &status_label) {
                move_to_installed(
                    idx as usize,
                    result,
                    &mut browse_for_btn.borrow_mut(),
                    &browse_list,
                    &mut installed_for_browse.borrow_mut(),
                    &installed_list,
                );
            }
        });

        // --- Add mod manually button ---
        let browse_for_add = browse_mods.clone();
        add_mod_btn.on_click(move |_| {
            if let Some(entry) = add_mod_dialog::show(&add_mod_btn, &status_label) {
                browse_list.append(&entry.list_label_browse());
                browse_for_add.borrow_mut().push(entry);
                status_label.set_label("Mod added.");
            }
        });

        // --- Installed tab events ---
        let installed_for_sel = installed_mods.clone();
        let installed_detail = installed.detail;
        let installed_splitter = installed.splitter;
        let installed_list_panel = installed.list_panel;
        let installed_detail_panel = installed.detail_panel;
        installed_list.on_selection_changed(move |_| {
            if let Some(idx) = installed_list.get_selection() {
                let mods = installed_for_sel.borrow();
                if let Some(entry) = mods.get(idx as usize) {
                    installed_detail.populate(entry);
                    installed_splitter.split_vertically(
                        &installed_list_panel, &installed_detail_panel, 300,
                    );
                }
            }
        });

        let installed_for_btn = installed_mods.clone();
        installed.detail.action_btn.on_click(move |_| {
            let Some(idx) = installed_list.get_selection() else { return };
            let manifest = {
                let mods = installed_for_btn.borrow();
                let Some(entry) = mods.get(idx as usize) else { return };
                entry.0.manifest.clone()
            };

            start_install(&manifest, &installed.detail.action_btn, &status_label);
        });

        // --- Updates tab events ---
        let update_for_sel = update_mods.clone();
        let updates_detail = updates.detail;
        let updates_splitter = updates.splitter;
        let updates_list_panel = updates.list_panel;
        let updates_detail_panel = updates.detail_panel;
        updates_list.on_selection_changed(move |_| {
            if let Some(idx) = updates_list.get_selection() {
                let mods = update_for_sel.borrow();
                if let Some(entry) = mods.get(idx as usize) {
                    updates_detail.populate(entry);
                    updates_splitter.split_vertically(
                        &updates_list_panel, &updates_detail_panel, 300,
                    );
                }
            }
        });

        let update_for_btn = update_mods.clone();
        let installed_for_update = installed_mods.clone();
        updates.detail.action_btn.on_click(move |_| {
            let Some(idx) = updates_list.get_selection() else { return };
            let manifest = {
                let mods = update_for_btn.borrow();
                let Some(entry) = mods.get(idx as usize) else { return };
                entry.0.manifest.clone()
            };

            if let Some(result) = start_install(&manifest, &updates.detail.action_btn, &status_label) {
                move_to_installed(
                    idx as usize,
                    result,
                    &mut update_for_btn.borrow_mut(),
                    &updates_list,
                    &mut installed_for_update.borrow_mut(),
                    &installed_list,
                );
            }
        });

        // --- Check for updates button ---
        update_btn.on_click(move |_| {
            check_and_apply_update(&update_btn, true);
        });

        frame.centre();
        frame.show(true);
    });

    Ok(())
}

/// Remove `entry` from `source_list`/`source_listbox` at `idx`, mark it as
/// installed using `result`, then append it to `installed_list`/`installed_listbox`.
fn move_to_installed(
    idx: usize,
    result: install_dialog::InstallResult,
    source_list: &mut Vec<ModEntry>,
    source_listbox: &ListBox,
    installed_list: &mut Vec<ModEntry>,
    installed_listbox: &ListBox,
) {
    let mut entry = source_list.remove(idx);
    source_listbox.delete(idx as u32);

    entry.0.installed = Some(crate::state::ModState {
        name: result.mod_name,
        source: entry.0.manifest.source.clone(),
        version: result.version.clone(),
        installed_at: chrono::Utc::now().to_rfc3339(),
        loader: entry.0.manifest.loader.name().to_string(),
        local_path: None,
        dependencies: std::collections::HashMap::new(),
    });
    entry.0.latest_tag = Some(result.version);
    installed_listbox.append(&entry.list_label_installed());
    installed_list.push(entry);
}

/// Resolve game path, spawn install worker, and show the install dialog.
/// Returns the install result on success.
fn start_install(
    manifest: &Manifest,
    parent: &Button,
    status_label: &StaticText,
) -> Option<install_dialog::InstallResult> {

    let state = AppState::load().unwrap_or_else(|_| AppState::new());
    let game_slug = manifest.game_slug();
    let saved_path = state
        .games
        .get(&game_slug)
        .map(|g| g.path.clone());

    let game_root = match installer::resolve_game_path(manifest, saved_path.as_deref()) {
        Ok(Some(p)) => p,
        Ok(None) => {
            // Game not found — ask user to locate it
            match ask_game_path(parent, &manifest.game.name, manifest.loader_kind().ok()) {
                Some(p) => {
                    // Save the path for next time
                    let mut state = AppState::load().unwrap_or_else(|_| AppState::new());
                    state.get_or_create_game(
                        &manifest.game_slug(),
                        &manifest.game.name,
                        &p.to_string_lossy(),
                    );
                    let _ = state.save();
                    p
                }
                None => {
                    return None;
                }
            }
        }
        Err(e) => {
            add_mod_dialog::show_error(parent, &format!("Error finding game: {e}"));
            return None;
        }
    };

    parent.enable(false);
    status_label.set_label(&format!("Installing {}...", manifest.name));

    let rx = worker::spawn_install(manifest.clone(), game_root);
    let result = install_dialog::show(
        parent,
        &format!("Installing {}", manifest.name),
        rx,
    );

    parent.enable(true);
    status_label.set_label("");
    result
}

/// Show a dialog with a DirPickerCtrl to locate a game's install folder.
/// Validates the selected path based on the loader type.
/// Returns the selected path, or None if cancelled.
fn ask_game_path(
    parent: &impl WxWidget,
    game_name: &str,
    loader: Option<crate::manifest::LoaderKind>,
) -> Option<std::path::PathBuf> {
    use crate::manifest::LoaderKind;
    use wxdragon::dialogs::Dialog;

    let dialog = Dialog::builder(parent, &format!("Locate {game_name}"))
        .with_size(500, 180)
        .build();

    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let label = StaticText::builder(&panel)
        .with_label(&format!("Select the install folder for {game_name}"))
        .build();

    let dir_picker = DirPickerCtrl::builder(&panel)
        .with_message(&format!("Select install folder for {game_name}"))
        .with_style(DirPickerCtrlStyle::DirMustExist | DirPickerCtrlStyle::UseTextCtrl)
        .build();

    let btn_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    let ok_btn = Button::builder(&panel)
        .with_id(wxdragon::id::ID_OK)
        .with_label("OK")
        .build();
    let cancel_btn = Button::builder(&panel)
        .with_id(wxdragon::id::ID_CANCEL)
        .with_label("Cancel")
        .build();
    btn_sizer.add_stretch_spacer(1);
    btn_sizer.add(&ok_btn, 0, SizerFlag::All, 4);
    btn_sizer.add(&cancel_btn, 0, SizerFlag::All, 4);

    sizer.add(&label, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(&dir_picker, 0, SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right, 8);
    sizer.add_stretch_spacer(1);
    sizer.add_sizer(&btn_sizer, 0, SizerFlag::Expand | SizerFlag::All, 4);
    panel.set_sizer(sizer, true);

    dialog.set_affirmative_id(wxdragon::id::ID_OK);
    dialog.set_escape_id(wxdragon::id::ID_CANCEL);

    loop {
        if dialog.show_modal() != wxdragon::id::ID_OK {
            return None;
        }

        let path = dir_picker.get_path();
        if path.is_empty() {
            return None;
        }

        let path = std::path::PathBuf::from(path);
        if !path.is_dir() {
            add_mod_dialog::show_error(&dialog, "That path is not a valid directory.");
            continue;
        }

        // Loader-specific validation
        match loader {
            Some(LoaderKind::Ue4ss) => {
                if crate::steam::find_ue_binaries(&path).ok().flatten().is_none() {
                    let dlg = MessageDialog::builder(
                        &dialog,
                        "This folder doesn't appear to contain an Unreal Engine game. Expected a subfolder with Binaries/Win64 inside. Would you like to pick a different folder?",
                        "Folder check",
                    )
                    .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconWarning)
                    .build();
                    if dlg.show_modal() == wxdragon::id::ID_YES {
                        continue;
                    }
                }
            }
            Some(LoaderKind::BepInEx) | Some(LoaderKind::MelonLoader) => {
                let has_exe = std::fs::read_dir(&path)
                    .ok()
                    .map(|entries| {
                        entries.filter_map(|e| e.ok()).any(|e| {
                            let name = e.file_name();
                            let name = name.to_string_lossy();
                            name.ends_with(".exe") && !name.starts_with("Uninstall")
                        })
                    })
                    .unwrap_or(false);

                if !has_exe {
                    let dlg = MessageDialog::builder(
                        &dialog,
                        "This folder doesn't appear to contain a game. No .exe file was found. Would you like to pick a different folder?",
                        "Folder check",
                    )
                    .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconWarning)
                    .build();
                    if dlg.show_modal() == wxdragon::id::ID_YES {
                        continue;
                    }
                }
            }
            _ => {}
        }

        return Some(path);
    }
}

/// Check for app updates and apply if the user confirms.
/// `manual` = true means the user clicked the button (bypass daily check).
fn check_and_apply_update(parent: &impl WxWidget, manual: bool) {
    let info = if manual {
        // Always check, but still record the timestamp
        updater::check_and_record()
    } else {
        updater::check_for_update()
    };

    let info = match info {
        Ok(Some(info)) => info,
        Ok(None) => {
            if manual {
                let dlg = MessageDialog::builder(
                    parent,
                    &format!("You are running the latest version ({}).", updater::current_version()),
                    "No updates available",
                )
                .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconInformation)
                .build();
                dlg.show_modal();
            }
            return;
        }
        Err(e) => {
            if manual {
                add_mod_dialog::show_error(
                    parent,
                    &format!("Could not check for updates: {e}"),
                );
            }
            return;
        }
    };

    // Ask user to confirm
    let dlg = MessageDialog::builder(
        parent,
        &format!(
            "AccessForge {} is available (you have {}). Update now?",
            info.version,
            updater::current_version()
        ),
        "Update available",
    )
    .with_style(MessageDialogStyle::YesNo | MessageDialogStyle::IconInformation)
    .build();

    if dlg.show_modal() != wxdragon::id::ID_YES {
        return;
    }

    // Run the update via install dialog
    let (tx, rx) = mpsc::channel();
    std::thread::spawn({
        let info_version = info.version.clone();
        let info_url = info.download_url.clone();
        let update_info = updater::UpdateInfo {
            version: info_version,
            download_url: info_url,
        };
        move || {
            if let Err(e) = updater::apply_update(&update_info, &tx) {
                let _ = tx.send(ProgressMsg::Failed(format!("{e:#}")));
            }
        }
    });

    let result = install_dialog::show(
        parent,
        &format!("Updating AccessForge to {}", info.version),
        rx,
    );

    if result.is_some() {
        // Update succeeded — restart
        updater::restart();
    }
}

