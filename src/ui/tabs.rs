use crate::ui::detail::DetailPanel;
use wxdragon::prelude::*;

/// All the widgets produced by building one tab.
#[derive(Copy, Clone)]
pub struct TabWidgets {
    pub splitter: SplitterWindow,
    pub list_panel: Panel,
    pub detail_panel: Panel,
    pub list: ListBox,
    pub detail: DetailPanel,
}

/// Build a single tab with the standard layout: splitter with list on the left,
/// detail panel on the right (initially hidden).
pub fn build_tab(
    notebook: &Notebook,
    label: &str,
    list_label: &str,
    detail_placeholder: &str,
    selected: bool,
) -> TabWidgets {
    let tab_panel = Panel::builder(notebook).build();
    let splitter = SplitterWindow::builder(&tab_panel).build();
    let list_panel = Panel::builder(&splitter).build();
    let detail_panel = Panel::builder(&splitter).build();

    let sr_label = StaticText::builder(&list_panel)
        .with_label(list_label)
        .build();
    let list = ListBox::builder(&list_panel).build();
    let list_sizer = BoxSizer::builder(Orientation::Vertical).build();
    list_sizer.add(
        &sr_label,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        4,
    );
    list_sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 4);
    list_panel.set_sizer(list_sizer, true);

    let detail = DetailPanel::build(&detail_panel, detail_placeholder);

    splitter.initialize(&list_panel);
    splitter.set_minimum_pane_size(150);

    let tab_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    tab_sizer.add(&splitter, 1, SizerFlag::Expand, 0);
    tab_panel.set_sizer(tab_sizer, true);
    notebook.add_page(&tab_panel, label, selected, None);

    TabWidgets {
        splitter,
        list_panel,
        detail_panel,
        list,
        detail,
    }
}

/// Build a tab with an extra button below the list (used for Browse's "Add mod manually").
pub fn build_tab_with_button(
    notebook: &Notebook,
    label: &str,
    list_label: &str,
    detail_placeholder: &str,
    button_label: &str,
    selected: bool,
) -> (TabWidgets, Button) {
    let tab_panel = Panel::builder(notebook).build();
    let splitter = SplitterWindow::builder(&tab_panel).build();
    let list_panel = Panel::builder(&splitter).build();
    let detail_panel = Panel::builder(&splitter).build();

    let sr_label = StaticText::builder(&list_panel)
        .with_label(list_label)
        .build();
    let list = ListBox::builder(&list_panel).build();
    let btn = Button::builder(&list_panel)
        .with_label(button_label)
        .build();
    let list_sizer = BoxSizer::builder(Orientation::Vertical).build();
    list_sizer.add(
        &sr_label,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        4,
    );
    list_sizer.add(&list, 1, SizerFlag::Expand | SizerFlag::All, 4);
    list_sizer.add(&btn, 0, SizerFlag::All, 4);
    list_panel.set_sizer(list_sizer, true);

    let detail = DetailPanel::build(&detail_panel, detail_placeholder);

    splitter.initialize(&list_panel);
    splitter.set_minimum_pane_size(150);

    let tab_sizer = BoxSizer::builder(Orientation::Horizontal).build();
    tab_sizer.add(&splitter, 1, SizerFlag::Expand, 0);
    tab_panel.set_sizer(tab_sizer, true);
    notebook.add_page(&tab_panel, label, selected, None);

    let widgets = TabWidgets {
        splitter,
        list_panel,
        detail_panel,
        list,
        detail,
    };
    (widgets, btn)
}
