//! Moving between the center's tabs and splitting the terminal in front.

use gpui_kit::component::{
    Placement,
    dock::{BasePanel, DockPlacement, InsertTarget, PanelId, TabGroup, panel_handle},
};
use gpui_kit::*;

use crate::app::{
    CenterTab, MoveTerminalPane, NextTab, PreviousTab, ShowTerminalPaneDropTab, SplitTerminalDown,
    SplitTerminalRight, SwitchToTab, TerminalPaneDestination,
};

use super::{Workspace, terminal_panes::TerminalPanes};

#[derive(Clone, Copy, PartialEq, Eq)]
enum RemovePane {
    Close,
    Move,
}

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

    /// Add a connection beside the source in its own tab. The dock's tab
    /// groups are unchanged, even when tabs have been dragged into splits.
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
        let anchor = self.split_tab_owner(source, cx).unwrap_or(source);
        let Some((group, _)) = self.center_tab_location(anchor, cx) else {
            return;
        };
        let opened = match source {
            CenterTab::Terminal(id) => {
                let Some(panel) = self.terminals.get(&id) else {
                    return;
                };
                let panel = panel.read(cx);
                let (host, copy) = (panel.host_id(), panel.copy_for_new_tab(cx));
                let Some(id) = self.new_remote_pane(host, Some(copy), window, cx) else {
                    return;
                };
                CenterTab::Terminal(id)
            }
            CenterTab::LocalTerminal(_) => {
                CenterTab::LocalTerminal(self.new_local_pane(None, window, cx))
            }
            _ => return,
        };
        self.insert_terminal_pane(anchor, &group, source, opened, placement, cx);
        self.set_active_tab(Some(opened), window, cx);
        self.focus_center(window, cx);
        cx.notify();
    }

    fn insert_terminal_pane(
        &mut self,
        anchor: CenterTab,
        group: &Entity<TabGroup>,
        sibling: CenterTab,
        pane: CenterTab,
        placement: Placement,
        cx: &mut Context<Self>,
    ) {
        let new_view = self
            .terminal_panel_view(pane)
            .expect("the pane is registered");
        let panes = match self.split_tabs.get(&anchor) {
            Some(panes) => panes.clone(),
            None => {
                let view = self
                    .terminal_panel_view(anchor)
                    .expect("the tab is a terminal");
                let target = self.focus_handle.clone();
                let panes = cx.new(|_| TerminalPanes::new(anchor, view.clone(), target));
                self.skin
                    .set_terminal_panes(view.entity_id(), Some(panes.clone()));
                self.split_tabs.insert(anchor, panes.clone());
                group.update(cx, |_, cx| cx.notify());
                panes
            }
        };
        panes.update(cx, |panes, cx| {
            panes.split(sibling, pane, new_view, placement, cx)
        });
    }

    pub(super) fn on_show_terminal_pane_drop_tab(
        &mut self,
        action: &ShowTerminalPaneDropTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !cx.has_active_drag() || crate::app::content_focus_blocked(window, cx) {
            return;
        }
        if let Some((group, panel)) = self.center_tab_location(action.0, cx)
            && group
                .read(cx)
                .active_panel(cx)
                .is_some_and(|active| active.panel_id(cx) != panel)
        {
            self.select_center_panel(&group, panel, window, cx);
        }
    }

    pub(super) fn on_move_terminal_pane(
        &mut self,
        action: &MoveTerminalPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if crate::app::content_focus_blocked(window, cx) {
            return;
        }
        let source = action.source;
        let Some(owner) = self.split_tab_owner(source, cx) else {
            return;
        };
        let (destination, sibling, placement) = match action.destination {
            TerminalPaneDestination::Tab(tab) if tab != owner => {
                (tab, self.selected_terminal_pane(tab, cx), Placement::Right)
            }
            TerminalPaneDestination::Beside(pane, placement) if pane != source => (
                self.split_tab_owner(pane, cx).unwrap_or(pane),
                pane,
                placement,
            ),
            _ => return,
        };
        let Some(view) = self.terminal_panel_view(source) else {
            return;
        };
        if self.terminal_panel_view(sibling).is_none() {
            return;
        }
        let Some((group, panel)) = self.center_tab_location(destination, cx) else {
            return;
        };
        let replaces_tab_in_another_group = source == owner
            && self
                .center_tab_location(owner, cx)
                .is_some_and(|(source_group, _)| source_group != group);
        if owner == destination {
            self.split_tabs[&owner].update(cx, |panes, cx| {
                panes.remove(source, cx);
                panes.split(sibling, source, view, placement, cx);
            });
        } else {
            if self
                .detach_split_pane(source, RemovePane::Move, window, cx)
                .is_none()
            {
                return;
            }
            self.insert_terminal_pane(destination, &group, sibling, source, placement, cx);
        }
        self.select_terminal_pane(source, cx);
        self.select_center_panel(&group, panel, window, cx);
        self.set_active_tab(Some(source), window, cx);
        self.focus_center(window, cx);
        cx.notify();
        if replaces_tab_in_another_group {
            // The replacement tab in the source group also activates after
            // the dock reconciles. Let that settle before restoring focus.
            cx.spawn_in(window, async move |this, cx| {
                _ = this.update_in(cx, |this, window, cx| {
                    if this.visible_center_tabs(cx).contains(&source)
                        && !crate::app::content_focus_blocked(window, cx)
                    {
                        this.set_active_tab(Some(source), window, cx);
                        this.focus_center(window, cx);
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

    pub(super) fn split_tab_owner(&self, tab: CenterTab, cx: &App) -> Option<CenterTab> {
        if !matches!(tab, CenterTab::Terminal(_) | CenterTab::LocalTerminal(_)) {
            return None;
        }
        self.split_tabs
            .iter()
            .find_map(|(&anchor, panes)| panes.read(cx).contains(tab).then_some(anchor))
    }

    pub(super) fn selected_terminal_pane(&self, anchor: CenterTab, cx: &App) -> CenterTab {
        self.split_tabs
            .get(&anchor)
            .map_or(anchor, |panes| panes.read(cx).active())
    }

    pub(super) fn select_terminal_pane(&self, tab: CenterTab, cx: &mut App) {
        if let Some(anchor) = self.split_tab_owner(tab, cx) {
            self.split_tabs[&anchor].update(cx, |panes, cx| panes.select(tab, cx));
            let focus = self.tab_focus_handle(tab, cx);
            self.set_split_focus(anchor, focus, cx);
        }
    }

    fn set_split_focus(&self, tab: CenterTab, focus: Option<FocusHandle>, cx: &mut App) {
        match tab {
            CenterTab::Terminal(id) => {
                if let Some(panel) = self.terminals.get(&id) {
                    panel.update(cx, |panel, _| panel.set_split_focus(focus));
                }
            }
            CenterTab::LocalTerminal(id) => {
                if let Some(panel) = self.local_terminals.get(&id) {
                    panel.update(cx, |panel, _| panel.set_split_focus(focus));
                }
            }
            _ => {}
        }
    }

    fn terminal_panel_view(&self, tab: CenterTab) -> Option<AnyView> {
        match tab {
            CenterTab::Terminal(id) => Some(self.terminals.get(&id)?.clone().into()),
            CenterTab::LocalTerminal(id) => Some(self.local_terminals.get(&id)?.clone().into()),
            _ => None,
        }
    }

    pub(super) fn dock_terminal(&self, tab: CenterTab, window: &mut Window, cx: &mut App) {
        let handle = match tab {
            CenterTab::Terminal(id) => panel_handle(self.terminals[&id].clone()),
            CenterTab::LocalTerminal(id) => panel_handle(self.local_terminals[&id].clone()),
            _ => return,
        };
        self.dock_area.update(cx, |area, cx| {
            area.add_panel_view(handle, DockPlacement::Center, None, window, cx);
        });
    }

    /// Collapse only the removed pane's split. When the first terminal is
    /// closed, a surviving terminal takes over the same tab bar slot.
    pub(super) fn remove_split_pane(
        &mut self,
        tab: CenterTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let was_front = self.active_tab == Some(tab);
        let Some(selected) = self.detach_split_pane(tab, RemovePane::Close, window, cx) else {
            return false;
        };
        if was_front {
            self.set_active_tab(Some(selected), window, cx);
            self.focus_center(window, cx);
        }
        cx.notify();
        true
    }

    fn detach_split_pane(
        &mut self,
        tab: CenterTab,
        removal: RemovePane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<CenterTab> {
        let anchor = self.split_tab_owner(tab, cx)?;
        let (group, old_panel) = self.center_tab_location(anchor, cx)?;
        let layout = self.split_tabs[&anchor].clone();
        layout.update(cx, |panes, cx| panes.remove(tab, cx));
        let remaining = layout.read(cx).terminals();
        let selected = layout.read(cx).active();
        let next_anchor = if tab == anchor { remaining[0] } else { anchor };

        if next_anchor != anchor || remaining.len() == 1 {
            self.split_tabs.remove(&anchor);
            if let Some(view) = self.terminal_panel_view(anchor) {
                self.skin.set_terminal_panes(view.entity_id(), None);
            }
            self.set_split_focus(anchor, None, cx);
        }
        if next_anchor != anchor && remaining.len() > 1 {
            layout.update(cx, |panes, cx| panes.set_tab(next_anchor, cx));
            let view = self
                .terminal_panel_view(next_anchor)
                .expect("a surviving terminal");
            self.skin
                .set_terminal_panes(view.entity_id(), Some(layout.clone()));
            self.split_tabs.insert(next_anchor, layout);
        }
        self.select_terminal_pane(selected, cx);

        if next_anchor != anchor {
            if removal == RemovePane::Move {
                match tab {
                    CenterTab::Terminal(id) => {
                        self.terminals[&id].update(cx, |panel, _| panel.move_to_split_on_removal())
                    }
                    CenterTab::LocalTerminal(id) => self.local_terminals[&id]
                        .update(cx, |panel, _| panel.move_to_split_on_removal()),
                    _ => unreachable!(),
                }
            }
            self.replace_terminal_tab(anchor, old_panel, next_anchor, &group, window, cx);
        } else {
            if removal == RemovePane::Close {
                // Run the same shutdown and Closed event as a docked terminal.
                match tab {
                    CenterTab::Terminal(id) => {
                        self.terminals[&id].update(cx, |panel, cx| panel.on_removed(window, cx))
                    }
                    CenterTab::LocalTerminal(id) => self.local_terminals[&id]
                        .update(cx, |panel, cx| panel.on_removed(window, cx)),
                    _ => unreachable!(),
                }
            }
            if remaining.len() == 1 {
                group.update(cx, |_, cx| cx.notify());
            }
        }
        Some(selected)
    }

    fn replace_terminal_tab(
        &self,
        old: CenterTab,
        old_panel: PanelId,
        replacement: CenterTab,
        group: &Entity<TabGroup>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selected = self.selected_center_tabs(cx);
        let Some(ix) = group
            .read(cx)
            .panels()
            .iter()
            .position(|panel| panel.panel_id(cx) == old_panel)
        else {
            return;
        };
        let view = self
            .terminal_panel_view(replacement)
            .expect("a surviving terminal");
        let panel = PanelId::from(view.entity_id());
        self.dock_terminal(replacement, window, cx);
        self.dock_area.update(cx, |area, cx| {
            if let Some(node) = area
                .layout(DockPlacement::Center)
                .and_then(|tree| tree.find_panel_node(old_panel))
            {
                area.move_panel(
                    panel,
                    InsertTarget::Tabs {
                        node,
                        ix: Some(ix),
                        activate: false,
                    },
                    window,
                    cx,
                );
            }
            match old {
                CenterTab::Terminal(id) => {
                    area.remove_panel(self.terminals[&id].clone(), window, cx)
                }
                CenterTab::LocalTerminal(id) => {
                    area.remove_panel(self.local_terminals[&id].clone(), window, cx)
                }
                _ => unreachable!(),
            }
        });
        for tab in selected {
            let tab = if tab == old { replacement } else { tab };
            if let Some((group, panel)) = self.center_tab_location(tab, cx) {
                self.select_center_panel(&group, panel, window, cx);
            }
        }
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
