use crate::worker::{InstallStep, ProgressMsg, TaskResult};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;
use wxdragon::dialogs::Dialog;
use wxdragon::prelude::*;
use wxdragon::timer::Timer;
use wxdragon::widgets::Gauge;

/// Result of a successful install.
pub struct InstallResult {
    pub mod_name: String,
    pub version: String,
}

/// Show the install log+gauge dialog. Blocks until the user closes it.
/// Returns `Some(InstallResult)` on success, `None` on failure.
pub fn show(
    parent: &impl WxWidget,
    title: &str,
    rx: mpsc::Receiver<ProgressMsg>,
) -> Option<InstallResult> {
    let dialog = Dialog::builder(parent, title)
        .with_size(500, 350)
        .build();

    let panel = Panel::builder(&dialog).build();
    let sizer = BoxSizer::builder(Orientation::Vertical).build();

    let log_label = StaticText::builder(&panel)
        .with_label("Installation log")
        .build();
    let log_list = ListBox::builder(&panel).build();
    let gauge = Gauge::builder(&panel).with_range(100).build();
    let close_btn = Button::builder(&panel)
        .with_id(wxdragon::id::ID_OK)
        .with_label("Close")
        .build();
    close_btn.enable(false);

    sizer.add(
        &log_label,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Top,
        8,
    );
    sizer.add(&log_list, 1, SizerFlag::Expand | SizerFlag::All, 8);
    sizer.add(
        &gauge,
        0,
        SizerFlag::Expand | SizerFlag::Left | SizerFlag::Right | SizerFlag::Bottom,
        8,
    );
    sizer.add(&close_btn, 0, SizerFlag::All | SizerFlag::AlignRight, 8);
    panel.set_sizer(sizer, true);

    dialog.set_affirmative_id(wxdragon::id::ID_OK);

    append_log(&log_list, "Preparing installation...");
    gauge.set_value(2);

    let result: Rc<RefCell<Option<InstallResult>>> = Rc::new(RefCell::new(None));
    let rx = Rc::new(RefCell::new(Some(rx)));
    let current_pct: Rc<RefCell<i32>> = Rc::new(RefCell::new(0));

    let timer = Rc::new(Timer::new(&dialog));
    let timer_ref = timer.clone();
    let rx_for_tick = rx.clone();
    let result_for_tick = result.clone();
    let pct_for_tick = current_pct.clone();

    timer.on_tick(move |_| {
        let _ = &timer_ref;
        let mut rx_borrow = rx_for_tick.borrow_mut();
        let Some(rx) = rx_borrow.as_ref() else { return };

        for _ in 0..50 {
            match rx.try_recv() {
                Ok(ProgressMsg::Status(_)) => {
                    // Status messages are internal — don't log or update gauge
                }
                Ok(ProgressMsg::InstallProgress { step, detail }) => {
                    let mut pct = pct_for_tick.borrow_mut();
                    *pct = (*pct + step_increment(&step)).min(95);
                    gauge.set_value(*pct);

                    if !detail.is_empty() {
                        append_log(&log_list, &format!("{step}: {detail}"));
                    } else {
                        append_log(&log_list, &step.to_string());
                    }
                }
                Ok(ProgressMsg::Done(TaskResult::Install { mod_name, version })) => {
                    gauge.set_value(100);
                    append_log(
                        &log_list,
                        &format!("{mod_name} {version} installed successfully!"),
                    );
                    *result_for_tick.borrow_mut() = Some(InstallResult {
                        mod_name,
                        version,
                    });
                    close_btn.enable(true);
                    close_btn.set_focus();
                    *rx_borrow = None;
                    return;
                }
                Ok(ProgressMsg::Failed(e)) => {
                    bell();
                    append_log(&log_list, &format!("Error: {e}"));
                    close_btn.enable(true);
                    close_btn.set_focus();
                    *rx_borrow = None;
                    return;
                }
                Ok(_) => {}
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    append_log(&log_list, "Installation stopped unexpectedly.");
                    close_btn.enable(true);
                    close_btn.set_focus();
                    *rx_borrow = None;
                    return;
                }
            }
        }
    });
    timer.start(50, false);

    dialog.show_modal();

    result.borrow_mut().take()
}

/// How much to increment the gauge for each step.
fn step_increment(step: &InstallStep) -> i32 {
    match step {
        InstallStep::InstallingLoader => 15,
        InstallStep::InstallingDependency => 10,
        InstallStep::InstallingMod => 15,
        InstallStep::PostInstall => 5,
        InstallStep::SavingState => 5,
    }
}

fn append_log(log_list: &ListBox, msg: &str) {
    log_list.append(msg);
    let count = log_list.get_count();
    if count > 0 {
        log_list.set_selection(count - 1, true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_increments_sum_does_not_exceed_95() {
        let total = step_increment(&InstallStep::InstallingLoader)
            + step_increment(&InstallStep::InstallingDependency)
            + step_increment(&InstallStep::InstallingMod)
            + step_increment(&InstallStep::PostInstall)
            + step_increment(&InstallStep::SavingState);
        assert!(
            total <= 95,
            "steps sum {total} would overflow gauge before Done fires"
        );
    }

    #[test]
    fn each_step_increment_is_positive() {
        let steps = [
            InstallStep::InstallingLoader,
            InstallStep::InstallingDependency,
            InstallStep::InstallingMod,
            InstallStep::PostInstall,
            InstallStep::SavingState,
        ];
        for step in &steps {
            assert!(step_increment(step) > 0, "step_increment for {step:?} must be positive");
        }
    }
}
