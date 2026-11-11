//! Quit confirmation, unsaved files and the final sync share one quit path.

use gpui_kit::component::{
    WindowExt as _, button::ButtonVariant, dialog::DialogButtonProps, v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use super::{Workspace, editors::describe_files};
use crate::i18n::t;

impl Workspace {
    /// Hold a quit until it is confirmed and any final sync has finished.
    /// Turning off confirmation never suppresses an unsaved-file warning.
    pub(super) fn ask_before_quit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if self.quit_pending {
            return true;
        }
        let unsaved = self.all_unsaved_files(cx);
        if unsaved.is_empty()
            && !self
                .settings()
                .read(cx)
                .settings()
                .window
                .confirm_before_quit
        {
            return self.sync_before_quit(cx);
        }

        // A hidden or minimized window must show the decision in front.
        crate::app::bring_forward(window, cx);
        self.quit_pending = true;
        let this = cx.weak_entity();
        window.open_alert_dialog(cx, move |alert, _, _| {
            let has_unsaved = !unsaved.is_empty();
            alert
                .title(t!("workspace.quit.title"))
                .description(
                    v_flex()
                        .id("quit-confirmation")
                        .test_support()
                        .aria_label(t!("workspace.quit.description"))
                        .gap_2()
                        .child(t!("workspace.quit.description"))
                        .when(has_unsaved, |body| {
                            let warning =
                                t!("workspace.unsaved.quit", files = describe_files(&unsaved));
                            body.child(
                                div()
                                    .id("quit-unsaved")
                                    .test_support()
                                    .aria_label(warning.clone())
                                    .child(warning),
                            )
                        }),
                )
                .button_props(
                    DialogButtonProps::default()
                        .ok_text(if has_unsaved {
                            t!("workspace.unsaved.discard_and_quit")
                        } else {
                            t!("workspace.quit.confirm")
                        })
                        .ok_variant(if has_unsaved {
                            ButtonVariant::Danger
                        } else {
                            ButtonVariant::Primary
                        })
                        .cancel_text(t!("common.cancel")),
                )
                .show_cancel(true)
                .on_cancel({
                    let this = this.clone();
                    move |_, _, cx| {
                        _ = this.update(cx, |this, _| this.quit_pending = false);
                        true
                    }
                })
                .on_ok({
                    let this = this.clone();
                    move |_, _, cx| {
                        _ = this.update(cx, |this, cx| {
                            if !this.sync_before_quit(cx) {
                                cx.quit();
                            }
                        });
                        true
                    }
                })
        });
        true
    }
}
