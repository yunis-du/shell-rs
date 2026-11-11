//! Which center tab is in front. Without a split it is the tab shown; with
//! the center split, several show at once and the one in front is the one
//! the user works in: the focus decides, not only a tab switch. The status
//! bar and the right sidebar follow it.

use gpui_kit::base::dock::Panel;
use gpui_kit::component::dock::{DockPlacement, NodeId, PaneRef, PanelId};
use gpui_kit::*;

use crate::app::CenterTab;

use super::Workspace;

impl Workspace {
    /// Removing a focused tab can remove its entire group. In that case
    /// the remaining groups already show their active panels, so none of
    /// them receives a new activation to take over the keyboard.
    pub(super) fn remove_center_panel<P: Panel>(
        &mut self,
        panel: Entity<P>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let focus = panel.read(cx).focus_handle(cx);
        let closing = (focus.is_focused(window) || focus.contains_focused(window, cx))
            .then(|| self.center_tab_for_panel(PanelId::from(panel.entity_id())))
            .flatten();
        let handoff = closing.map(|tab| {
            (
                tab,
                self.center_tab_location(tab, cx).map(|(group, _)| group),
                self.visible_center_tabs(cx),
            )
        });

        self.dock_area
            .update(cx, |area, cx| area.remove_panel(panel, window, cx));

        let Some((closed, group, before)) = handoff else {
            return;
        };
        if crate::app::content_focus_blocked(window, cx) {
            return;
        }
        let visible = self.visible_center_tabs(cx);
        if visible.contains(&closed) {
            return;
        }
        let same_group = group
            .and_then(|group| group.read(cx).active_panel(cx))
            .and_then(|panel| self.center_tab_for_panel(panel.panel_id(cx)))
            .filter(|tab| visible.contains(tab));
        let next = same_group
            .or_else(|| {
                let ix = before.iter().position(|&tab| tab == closed)?;
                before[..ix]
                    .iter()
                    .rev()
                    .chain(&before[ix + 1..])
                    .copied()
                    .find(|tab| visible.contains(tab))
            })
            .or_else(|| visible.first().copied());
        if let Some(next) = next {
            self.set_active_tab(Some(next), window, cx);
            self.focus_center(window, cx);
        }
        // When no tabs remain, on_layout_changed focuses the start page.
    }

    /// The center tabs on screen: the tab each group shows, or only the
    /// zoomed group's.
    pub(super) fn visible_center_tabs(&self, cx: &App) -> Vec<CenterTab> {
        self.selected_tabs(self.dock_area.read(cx).zoomed_group(), cx)
    }

    /// The tab selected in every group, even those hidden by dock zoom.
    pub(super) fn selected_center_tabs(&self, cx: &App) -> Vec<CenterTab> {
        self.selected_tabs(None, cx)
    }

    fn selected_tabs(&self, only: Option<NodeId>, cx: &App) -> Vec<CenterTab> {
        let area = self.dock_area.read(cx);
        let Some(tree) = area.layout(DockPlacement::Center) else {
            return Vec::new();
        };
        let mut shown: Vec<PanelId> = Vec::new();
        tree.root().walk(&mut |node| {
            if let PaneRef::Tabs { panels, active_ix } = node.kind()
                && only.is_none_or(|id| id == node.id())
                && let Some(panel) = panels.get(active_ix).or(panels.last())
            {
                shown.push(*panel);
            }
        });
        shown
            .into_iter()
            .filter_map(|panel| self.center_tab_for_panel(panel))
            .collect()
    }

    /// The focus went into `tab`: in a split, clicking into the other half
    /// brings that tab to the front without any tab being switched to.
    /// Does what the tab's own activation does.
    pub(super) fn focus_entered(
        &mut self,
        tab: CenterTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_tab == Some(tab) {
            return;
        }
        let open = match tab {
            CenterTab::Terminal(id) => self.terminals.contains_key(&id),
            CenterTab::Explorer(id) => self.explorers.contains_key(&id),
            CenterTab::Editor(id) => self.editors.contains_key(&id),
            CenterTab::LocalTerminal(_) | CenterTab::Settings => true,
        };
        if !open {
            return;
        }
        self.set_active_tab(Some(tab), window, cx);
        cx.notify();
    }

    /// After the center's layout changed (a tab dragged into another group,
    /// a group closed): a tab in front that no longer shows gives way to the
    /// one with the focus, or to one that shows; then the right sidebar
    /// follows.
    pub(super) fn sync_front(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.visible_center_tabs(cx);
        if let Some(tab) = self.active_tab
            && !visible.contains(&tab)
        {
            self.active_tab = visible
                .iter()
                .copied()
                .find(|tab| {
                    self.tab_focus_handle(*tab, cx)
                        .is_some_and(|focus| focus.contains_focused(window, cx))
                })
                .or_else(|| visible.first().copied());
        }
        self.sync_tool_sidebar(window, cx);
    }
}

/// Bring `tab` to the front whenever the focus goes into it. Registered with
/// every center tab, held for as long as the workspace.
pub(super) fn follow_focus(
    tab: CenterTab,
    focus: &FocusHandle,
    window: &mut Window,
    cx: &mut Context<Workspace>,
) -> Subscription {
    cx.on_focus_in(focus, window, move |_, window, cx| {
        // Focus listeners run in the frame's focus phase, where a change
        // would not be drawn until something else asks for a frame.
        cx.defer_in(window, move |this, window, cx| {
            this.focus_entered(tab, window, cx)
        });
    })
}
