//! Splitting terminals and moving between the center's tabs from the keyboard.

use gpui_kit::component::{
    Placement,
    dock::{DockPlacement, InsertTarget, TabGroup},
};
use gpui_kit::*;

use crate::app::{
    CenterTab, NextTab, PreviousTab, SplitTerminalDown, SplitTerminalRight, SwitchToTab,
};

use super::Workspace;

impl Workspace {
    pub(super) fn on_split_terminal_right(
        &mut self,
        _: &SplitTerminalRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_terminal(Placement::Right, window, cx);
    }

    pub(super) fn on_split_terminal_down(
        &mut self,
        _: &SplitTerminalDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_terminal(Placement::Bottom, window, cx);
    }

    /// Keep the source terminal running and open another in its own group.
    /// Resolve the source's node after opening: adding a panel reconciles
    /// the dock, and the new tab need not land in the source's group.
    fn split_terminal(
        &mut self,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::app::content_focus_blocked(window, cx) {
            return;
        }
        let Some(source) = self.active_tab else {
            return;
        };
        let Some((_, anchor)) = self.center_tab_location(source, cx) else {
            return;
        };
        let selected = self.selected_center_tabs(cx);
        let opened = match source {
            CenterTab::Terminal(id) => {
                let Some(panel) = self.terminals.get(&id) else {
                    return;
                };
                let panel = panel.read(cx);
                let (host, copy) = (panel.host_id(), panel.copy_for_new_tab(cx));
                let Some(id) = self.open_remote_terminal(host, Some(copy), window, cx) else {
                    return;
                };
                CenterTab::Terminal(id)
            }
            CenterTab::LocalTerminal(_) => {
                CenterTab::LocalTerminal(self.open_local_terminal(None, window, cx))
            }
            CenterTab::Explorer(_) | CenterTab::Settings | CenterTab::Editor(_) => return,
        };
        let Some((_, panel)) = self.center_tab_location(opened, cx) else {
            return;
        };
        self.dock_area.update(cx, |area, cx| {
            area.set_zoomed_out(window, cx);
            if let Some(node) = area
                .layout(DockPlacement::Center)
                .and_then(|tree| tree.find_panel_node(anchor))
            {
                area.move_panel(
                    panel,
                    InsertTarget::Split {
                        node,
                        placement,
                        size: None,
                    },
                    window,
                    cx,
                );
            }
        });
        // Adding a tab selects it in the first group, which may be a
        // different group from the source. Restore each group's selection
        // after the new tab has moved out.
        for tab in selected {
            let Some((group, panel)) = self.center_tab_location(tab, cx) else {
                continue;
            };
            group.update(cx, |group, cx| {
                if let Some(ix) = group
                    .panels()
                    .iter()
                    .position(|candidate| candidate.panel_id(cx) == panel)
                {
                    group.select_tab(ix, window, cx);
                }
            });
        }
        // Dock activation is deferred. Focus the new terminal after the
        // layout settles so repeated splits follow the new pane.
        cx.defer_in(window, move |this, window, cx| {
            if let Some(focus) = this.tab_focus_handle(opened, cx) {
                crate::app::focus_content(&focus, window, cx);
            }
        });
    }

    pub(super) fn on_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        self.step_tab(1, window, cx);
    }

    pub(super) fn on_previous_tab(
        &mut self,
        _: &PreviousTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_tab(-1, window, cx);
    }

    /// From 1; 9 is the last, as in browsers. A position past the last
    /// does nothing.
    pub(super) fn on_switch_to_tab(
        &mut self,
        action: &SwitchToTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::app::content_focus_blocked(window, cx) {
            return;
        }
        let Some((group, _)) = self.current_tab_group(cx) else {
            return;
        };
        group.update(cx, |group, cx| {
            let count = group.panels().len();
            let ix = if action.0 >= 9 {
                count.checked_sub(1)
            } else {
                action.0.checked_sub(1).filter(|ix| *ix < count)
            };
            if let Some(ix) = ix {
                group.select_tab(ix, window, cx);
            }
        });
    }

    /// The tab `step` places away from the current one, around the ends.
    fn step_tab(&mut self, step: isize, window: &mut Window, cx: &mut Context<Self>) {
        if crate::app::content_focus_blocked(window, cx) {
            return;
        }
        let Some((group, ix)) = self.current_tab_group(cx) else {
            return;
        };
        group.update(cx, |group, cx| {
            let count = group.panels().len() as isize;
            if count > 1 {
                let next = (ix as isize + step).rem_euclid(count) as usize;
                group.select_tab(next, window, cx);
            }
        });
    }

    /// The group holding the current center tab, and where in it the tab
    /// is. Its `select_tab` activates the panel, which reports back to
    /// `set_active_tab`.
    fn current_tab_group(&self, cx: &App) -> Option<(Entity<TabGroup>, usize)> {
        let (group, panel) = self.center_tab_location(self.active_tab?, cx)?;
        let ix = group
            .read(cx)
            .panels()
            .iter()
            .position(|candidate| candidate.panel_id(cx) == panel)?;
        Some((group, ix))
    }
}
