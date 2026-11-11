//! The split tree inside one terminal tab. The outer dock only sees its tab.

use std::{cell::Cell, rc::Rc};

use gpui_kit::base::ElementExt as _;
use gpui_kit::component::{
    ActiveTheme as _, Icon, Placement, Sizable as _, h_flex,
    resizable::{ResizableState, h_resizable, resizable_panel, v_resizable},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::{
    app::{CatalogIcon, CenterTab, MoveTerminalPane, TerminalPaneDestination},
    i18n::t,
    shared::{RenamableTab as _, TerminalPaneDrag},
    terminal::{LocalTerminalPanel, TerminalPanel},
};

pub(super) struct TerminalPanes {
    root: Option<Pane>,
    active: CenterTab,
    tab: CenterTab,
    target: FocusHandle,
}

enum Pane {
    Terminal(CenterTab, AnyView),
    Split {
        axis: Axis,
        size: Entity<ResizableState>,
        children: Box<[Pane; 2]>,
    },
}

impl TerminalPanes {
    pub(super) fn new(tab: CenterTab, view: AnyView, target: FocusHandle) -> Self {
        Self {
            root: Some(Pane::Terminal(tab, view)),
            active: tab,
            tab,
            target,
        }
    }

    pub(super) fn set_tab(&mut self, tab: CenterTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    pub(super) fn active(&self) -> CenterTab {
        self.active
    }

    pub(super) fn select(&mut self, tab: CenterTab, cx: &mut Context<Self>) {
        if self.active != tab {
            self.active = tab;
            cx.notify();
        }
    }

    pub(super) fn contains(&self, tab: CenterTab) -> bool {
        self.root.as_ref().is_some_and(|pane| pane.contains(tab))
    }

    pub(super) fn terminals(&self) -> Vec<CenterTab> {
        let mut tabs = Vec::new();
        if let Some(root) = &self.root {
            root.collect(&mut tabs);
        }
        tabs
    }

    pub(super) fn split(
        &mut self,
        source: CenterTab,
        tab: CenterTab,
        view: AnyView,
        placement: Placement,
        cx: &mut Context<Self>,
    ) {
        if let Some(root) = self.root.take() {
            self.root = Some(root.split(source, Pane::Terminal(tab, view), placement, cx));
            self.select(tab, cx);
        }
    }

    pub(super) fn remove(&mut self, tab: CenterTab, cx: &mut Context<Self>) {
        if self.active == tab {
            let tabs = self.terminals();
            if let Some(ix) = tabs.iter().position(|&id| id == tab)
                && let Some(&next) = ix
                    .checked_sub(1)
                    .and_then(|ix| tabs.get(ix))
                    .or(tabs.get(ix + 1))
            {
                self.active = next;
            }
        }
        self.root = self.root.take().and_then(|root| root.remove(tab));
        cx.notify();
    }
}

impl Pane {
    fn contains(&self, tab: CenterTab) -> bool {
        match self {
            Self::Terminal(id, _) => *id == tab,
            Self::Split { children, .. } => children.iter().any(|pane| pane.contains(tab)),
        }
    }

    fn collect(&self, tabs: &mut Vec<CenterTab>) {
        match self {
            Self::Terminal(id, _) => tabs.push(*id),
            Self::Split { children, .. } => {
                for child in children.iter() {
                    child.collect(tabs);
                }
            }
        }
    }

    fn split(self, source: CenterTab, new: Self, placement: Placement, cx: &mut App) -> Self {
        match self {
            Self::Terminal(..) => Self::Split {
                axis: placement.axis(),
                size: cx.new(|_| ResizableState::default()),
                children: Box::new(match placement {
                    Placement::Top | Placement::Left => [new, self],
                    Placement::Bottom | Placement::Right => [self, new],
                }),
            },
            Self::Split {
                axis: parent_axis,
                size,
                children,
            } => {
                let [first, second] = *children;
                let children = if first.contains(source) {
                    [first.split(source, new, placement, cx), second]
                } else {
                    [first, second.split(source, new, placement, cx)]
                };
                Self::Split {
                    axis: parent_axis,
                    size,
                    children: Box::new(children),
                }
            }
        }
    }

    fn remove(self, tab: CenterTab) -> Option<Self> {
        match self {
            Self::Terminal(id, _) => (id != tab).then_some(self),
            Self::Split {
                axis,
                size,
                children,
            } => {
                let [first, second] = *children;
                match (first.remove(tab), second.remove(tab)) {
                    (Some(first), Some(second)) => Some(Self::Split {
                        axis,
                        size,
                        children: Box::new([first, second]),
                    }),
                    (first, second) => first.or(second),
                }
            }
        }
    }

    fn element(&self, panes: &TerminalPanes, minimum: Pixels, cx: &App) -> AnyElement {
        match self {
            Self::Terminal(id, view) => {
                let dim_id = match id {
                    CenterTab::Terminal(id) => ("dim-remote-pane", id.0),
                    CenterTab::LocalTerminal(id) => ("dim-local-pane", id.0),
                    _ => unreachable!("only terminals can be split"),
                };
                let terminal = div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .min_h_0()
                    .overflow_hidden()
                    .child(view.clone())
                    .when(*id != panes.active, |frame| {
                        // The shade leaves input and scrolling with the terminal.
                        frame.child(
                            div()
                                .id(dim_id)
                                .test_support()
                                .absolute()
                                .inset_0()
                                .bg(cx.theme().muted.opacity(0.28)),
                        )
                    });
                let title = match id {
                    CenterTab::Terminal(_) => view
                        .clone()
                        .downcast::<TerminalPanel>()
                        .ok()
                        .map(|panel| panel.read(cx).tab_title(cx)),
                    CenterTab::LocalTerminal(_) => view
                        .clone()
                        .downcast::<LocalTerminalPanel>()
                        .ok()
                        .map(|panel| panel.read(cx).display_title(cx)),
                    _ => None,
                }
                .unwrap_or_default();
                panes
                    .pane_frame(*id, title, terminal, cx)
                    .into_any_element()
            }
            Self::Split {
                axis,
                size,
                children,
            } => {
                let id = ("terminal-split", size.entity_id().as_u64());
                let group = match axis {
                    Axis::Horizontal => h_resizable(id),
                    Axis::Vertical => v_resizable(id),
                };
                group
                    .with_state(size)
                    .children(children.iter().map(|pane| {
                        resizable_panel()
                            .size_range(minimum..Pixels::MAX)
                            .child(pane.element(panes, minimum, cx))
                    }))
                    .into_any_element()
            }
        }
    }
}

impl Render for TerminalPanes {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let minimum = rems(4.).to_pixels(window.rem_size());
        div().size_full().children(
            self.root
                .as_ref()
                .map(|root| root.element(self, minimum, cx)),
        )
    }
}

impl TerminalPanes {
    fn pane_frame(
        &self,
        pane: CenterTab,
        title: SharedString,
        body: Div,
        cx: &App,
    ) -> impl IntoElement {
        let (frame_id, drag_id) = match pane {
            CenterTab::Terminal(id) => (("remote-pane", id.0), ("drag-remote-pane", id.0)),
            CenterTab::LocalTerminal(id) => {
                (("local-terminal-pane", id.0), ("drag-local-pane", id.0))
            }
            _ => unreachable!("only terminals can be split"),
        };
        let drag = TerminalPaneDrag {
            pane,
            tab: self.tab,
            title: title.clone(),
        };
        let header = h_flex()
            .id(drag_id)
            .test_support()
            .aria_label(t!("terminal.pane.drag", title = title))
            .cursor_grab()
            .w_full()
            .h_5()
            .min_w_0()
            .flex_shrink_0()
            .bg(cx.theme().muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .px_1()
            .gap_1()
            .on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
            .child(
                Icon::new(CatalogIcon::GripVertical)
                    .xsmall()
                    .flex_shrink_0(),
            )
            .child(div().truncate().text_xs().child(title));
        terminal_drop_target(
            v_flex().id(frame_id).size_full().child(header).child(body),
            pane,
            &self.target,
            cx,
        )
        .test_support()
    }
}

/// Both an unsplit terminal and a pane accept a drag at their edges.
pub(super) fn terminal_drop_target(
    frame: Stateful<Div>,
    pane: CenterTab,
    target: &FocusHandle,
    cx: &App,
) -> Stateful<Div> {
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let drop_focus = target.clone();
    frame
        .relative()
        .on_prepaint({
            let bounds = bounds.clone();
            move |current, _, _| bounds.set(current)
        })
        .when(cx.has_active_drag(), |frame| {
            frame.children(
                [
                    Placement::Left,
                    Placement::Right,
                    Placement::Top,
                    Placement::Bottom,
                ]
                .map(|side| drop_preview(pane, side, bounds.clone())),
            )
        })
        .on_drop(move |drag: &TerminalPaneDrag, window, cx| {
            if drag.pane != pane {
                drop_focus.dispatch_action(
                    &MoveTerminalPane {
                        source: drag.pane,
                        destination: TerminalPaneDestination::Beside(
                            pane,
                            nearest_edge(bounds.get(), window.mouse_position()),
                        ),
                    },
                    window,
                    cx,
                );
            }
        })
}

/// Shade the half the dragged pane will occupy. The same direction calculation
/// drives painting and the drop, and the overlay never participates in layout.
fn drop_preview(
    pane: CenterTab,
    side: Placement,
    bounds: Rc<Cell<Bounds<Pixels>>>,
) -> impl IntoElement {
    let (id, half) = match side {
        Placement::Left => (
            "pane-drop-left",
            div().left_0().top_0().w(relative(0.5)).h_full(),
        ),
        Placement::Right => (
            "pane-drop-right",
            div().right_0().top_0().w(relative(0.5)).h_full(),
        ),
        Placement::Top => (
            "pane-drop-top",
            div().left_0().top_0().w_full().h(relative(0.5)),
        ),
        Placement::Bottom => (
            "pane-drop-bottom",
            div().left_0().bottom_0().w_full().h(relative(0.5)),
        ),
    };
    half.id(id)
        .absolute()
        .invisible()
        .drag_over::<TerminalPaneDrag>(move |style, drag, window, cx| {
            if drag.pane == pane || nearest_edge(bounds.get(), window.mouse_position()) != side {
                return style;
            }
            let style = style
                .visible()
                .bg(cx.theme().primary.opacity(0.22))
                .border_color(cx.theme().primary);
            match side {
                Placement::Left => style.border_r_1(),
                Placement::Right => style.border_l_1(),
                Placement::Top => style.border_b_1(),
                Placement::Bottom => style.border_t_1(),
            }
        })
        .test_support()
}

fn nearest_edge(bounds: Bounds<Pixels>, position: Point<Pixels>) -> Placement {
    if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
        return Placement::Right;
    }
    let x = (position.x - bounds.left()) / bounds.size.width;
    let y = (position.y - bounds.top()) / bounds.size.height;
    [
        (x, Placement::Left),
        (1. - x, Placement::Right),
        (y, Placement::Top),
        (1. - y, Placement::Bottom),
    ]
    .into_iter()
    .min_by(|(a, _), (b, _)| a.total_cmp(b))
    .map_or(Placement::Right, |(_, edge)| edge)
}
