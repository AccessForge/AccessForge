use crate::manifest::{Manifest, Source};
use crate::registry;
use crate::ui::detail::ModEntry;
use crate::worker::LoadedMod;
use wxdragon::dialogs::Dialog;
use wxdragon::dialogs::message_dialog::{MessageDialog, MessageDialogStyle};
use wxdragon::prelude::*;

/// Show the "Add mod manually" dialog. Returns a `ModEntry` if the user
/// entered a valid source and the manifest was fetched successfully.
pub fn show(parent: &impl WxWidget, status_label: &StaticText) -> Option<ModEntry> {
    let dialog = Dialog::builder(parent, "Add mod manually")
        .with_size(450, 200)
        .build();

    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let label = StaticText::builder(&panel)
        .with_label("Enter a link to a mod. For example, https://github.com/owner/repo or https://example.com/my-mod")
        .build();
    let input_field = TextCtrl::builder(&panel)
        .with_style(TextCtrlStyle::ProcessEnter)
        .build();
    let btn_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    let ok_btn = Button::builder(&panel)
        .with_id(wxdragon::id::ID_OK)
        .with_label("Add")
        .build();
    let cancel_btn = Button::builder(&panel)
        .with_id(wxdragon::id::ID_CANCEL)
        .with_label("Cancel")
        .build();
    btn_sizer.add_stretch_spacer(1);
    btn_sizer.add(&ok_btn, 0, SizerFlag::All, 4);
    btn_sizer.add(&cancel_btn, 0, SizerFlag::All, 4);

    sizer.add(&label, 0, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(
        &input_field,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        8,
    );
    sizer.add_stretch_spacer(1);
    sizer.add_sizer(&btn_sizer, 0, SizerFlag::Expand | SizerFlag::All, 4);
    panel.set_sizer(sizer, true);

    dialog.set_affirmative_id(wxdragon::id::ID_OK);
    dialog.set_escape_id(wxdragon::id::ID_CANCEL);
    input_field.set_focus();

    if dialog.show_modal() != wxdragon::id::ID_OK {
        return None;
    }

    let input = input_field.get_value();
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let source = match Source::parse_user_input(input) {
        Ok(s) => s,
        Err(e) => {
            show_error(parent, &format!("Invalid source: {e}"));
            return None;
        }
    };

    status_label.set_label("Fetching manifest...");

    let yaml = match registry::fetch_manifest_for_source(&source) {
        Ok(y) => y,
        Err(e) => {
            show_error(parent, &format!("Could not fetch manifest: {e}"));
            return None;
        }
    };

    let manifest = match Manifest::from_yaml(&yaml) {
        Ok(m) => m,
        Err(e) => {
            show_error(parent, &format!("Invalid manifest: {e}"));
            return None;
        }
    };

    Some(ModEntry(LoadedMod {
        manifest,
        installed: None,
        latest_tag: None,
    }))
}

/// Show a modal error dialog.
pub fn show_error(parent: &impl WxWidget, message: &str) {
    let dlg = MessageDialog::builder(parent, message, "Error")
        .with_style(MessageDialogStyle::OK | MessageDialogStyle::IconError)
        .build();
    dlg.show_modal();
}
