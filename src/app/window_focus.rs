//! Focus policy for background changes in the main window.

use gpui_kit::component::{Root, WindowExt as _};
use gpui_kit::{App, FocusHandle, Window};

pub(crate) fn content_focus_blocked(window: &mut Window, cx: &mut App) -> bool {
    crate::app_lock::is_locked(cx)
        || (window.root::<Root>().flatten().is_some() && window.has_active_dialog(cx))
}

/// A dock tab can finish activating after a dialog opened. Its activation
/// still counts, but the dialog must keep the keyboard and its action path.
pub(crate) fn focus_content(focus: &FocusHandle, window: &mut Window, cx: &mut App) {
    if !content_focus_blocked(window, cx) {
        window.focus(focus, cx);
    }
}
