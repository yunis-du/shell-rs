//! The searchable picker for running a saved command in one terminal.

use gpui_kit::component::{
    ActiveTheme as _, IndexPath, WindowExt as _,
    dialog::DialogFooter,
    h_flex,
    kbd::Kbd,
    list::{List, ListDelegate, ListItem, ListState},
    v_flex,
};
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::app::{CenterTab, RunQuickSnippet};
use crate::host::{Snippet, listed_order};
use crate::i18n::t;
use crate::shared::{command_tooltip, one_line};

struct SnippetList {
    snippets: Vec<Snippet>,
    shown: Vec<usize>,
    selected: Option<IndexPath>,
    terminal: CenterTab,
    generation: u64,
    dispatch: FocusHandle,
}

impl SnippetList {
    fn shown(&self, ix: IndexPath) -> Option<&Snippet> {
        self.shown.get(ix.row).map(|&ix| &self.snippets[ix])
    }
}

impl ListDelegate for SnippetList {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.shown = self
            .snippets
            .iter()
            .enumerate()
            .filter_map(|(ix, snippet)| snippet.matches(query).then_some(ix))
            .collect();
        // List chooses using its previous row count. Restore the first
        // match when a search with no results is refined.
        cx.defer_in(window, |list, window, cx| {
            let selected = (!list.delegate().shown.is_empty()).then(IndexPath::default);
            list.set_selected_index(selected, window, cx);
            cx.notify();
        });
        Task::ready(())
    }

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.shown.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let snippet = self.shown(ix)?;
        let command = one_line(&snippet.command);
        Some(
            ListItem::new(("quick-snippet-row", snippet.id.0))
                .py_1p5()
                .rounded(cx.theme().radius)
                .child(
                    v_flex()
                        .id(("quick-snippet-item", snippet.id.0))
                        .test_support()
                        .aria_label(format!("{} · {command}", snippet.name))
                        .min_w_0()
                        .when_some(command_tooltip(&snippet.command, cx), |row, tooltip| {
                            row.tooltip(tooltip)
                        })
                        .child(div().truncate().child(snippet.name.clone()))
                        .child(
                            div()
                                .truncate()
                                .text_sm()
                                .font_family(cx.theme().mono_font_family.clone())
                                .text_color(cx.theme().muted_foreground)
                                .child(command),
                        ),
                ),
        )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(snippet) = self.selected.and_then(|ix| self.shown(ix)) else {
            return;
        };
        let action = RunQuickSnippet {
            terminal: self.terminal,
            generation: self.generation,
            command: snippet.command.clone(),
        };
        let dispatch = self.dispatch.clone();
        window.close_dialog(cx);
        window.defer(cx, move |window, cx| {
            dispatch.dispatch_action(&action, window, cx)
        });
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        let message = if self.snippets.is_empty() {
            t!("snippets.panel.empty")
        } else {
            t!("snippets.panel.no_match")
        };
        h_flex()
            .id("quick-snippet-empty")
            .test_support()
            .aria_label(message.clone())
            .size_full()
            .justify_center()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(message)
    }
}

pub(crate) fn open_quick_snippet_dialog(
    mut snippets: Vec<Snippet>,
    terminal: CenterTab,
    generation: u64,
    terminal_name: SharedString,
    dispatch: FocusHandle,
    window: &mut Window,
    cx: &mut App,
) {
    snippets.sort_by(|a, b| listed_order((0, &a.name, a.id.0), (0, &b.name, b.id.0)));
    let list = cx.new(|cx| {
        let shown = (0..snippets.len()).collect();
        let delegate = SnippetList {
            snippets,
            shown,
            selected: None,
            terminal,
            generation,
            dispatch,
        };
        ListState::new(delegate, window, cx).searchable(true)
    });
    window.open_dialog(cx, {
        let list = list.clone();
        move |dialog, _, cx| {
            let theme = cx.theme();
            let target = t!("snippets.quick.target", terminal = terminal_name);
            dialog
                .title(t!("snippets.quick.title"))
                .overlay_closable(false)
                .child(
                    div()
                        .id("quick-snippet")
                        .test_support()
                        .w_full()
                        .h(rems(20.))
                        .border_1()
                        .border_color(theme.border)
                        .rounded(theme.radius)
                        .overflow_hidden()
                        .child(List::new(&list).search_placeholder(t!("snippets.panel.search"))),
                )
                .footer(
                    DialogFooter::new().w_full().child(
                        h_flex()
                            .gap_2()
                            .min_w_0()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .children(Keystroke::parse("enter").ok().map(Kbd::new))
                            .child(
                                div()
                                    .id("quick-snippet-target")
                                    .test_support()
                                    .aria_label(target.clone())
                                    .min_w_0()
                                    .child(target),
                            ),
                    ),
                )
        }
    });
    list.update(cx, |list, cx| list.focus(window, cx));
}
