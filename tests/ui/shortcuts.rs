//! 设置 › 键盘快捷键, and the tab keys it lists.

use gpui_kit::{Keystroke, point, px};
use shellrs::app::{CenterTab, Shortcut};
use shellrs::terminal::RemoteTerminalId;

use crate::support::*;

fn default_keys(id: &str) -> &'static str {
    Shortcut::find(id).unwrap().default_keys()
}

/// cmd on macOS, ctrl elsewhere.
fn primary(key: &str) -> String {
    let modifier = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    format!("{modifier}-{key}")
}

fn active_tab(workspace: &Entity<Workspace>, cx: &mut TestAppContext) -> Option<CenterTab> {
    cx.update(|cx| workspace.read(cx).active_tab())
}

fn terminal(id: u64) -> Option<CenterTab> {
    Some(CenterTab::Terminal(RemoteTerminalId(id)))
}

#[gpui_kit::test]
fn split_shortcuts_open_terminals_on_the_requested_side_and_follow_focus(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, _| window.activate_window());
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-left"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        let source = window.find(("terminal", INITIAL_WEB_TERMINAL));
        let right = window.find(("terminal", FIRST_NEW_TERMINAL));
        assert!(source.visible() && right.visible());
        assert!(source.bounds().right() <= right.bounds().left());
        assert_eq!(right.focused(), Some(true));
        assert!(
            window
                .find(("dim-remote-pane", INITIAL_WEB_TERMINAL))
                .visible()
        );
        assert!(
            window
                .try_find(("dim-remote-pane", FIRST_NEW_TERMINAL))
                .is_none()
        );
        assert_eq!(workspace.read(cx).terminal_count(HostId(WEB_01), cx), 2);
        window.click(("terminal", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window.find(("terminal", INITIAL_WEB_TERMINAL)).focused(),
            Some(true)
        );
        assert!(
            window
                .find(("dim-remote-pane", FIRST_NEW_TERMINAL))
                .visible()
        );
        window.click(("terminal", FIRST_NEW_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-down"), cx);
    });
    assert_eq!(active_tab(&workspace, cx), terminal(FIRST_NEW_TERMINAL + 1));
    in_frame(cx, handle, |window, cx| {
        let right = window.find(("terminal", FIRST_NEW_TERMINAL));
        let below = window.find(("terminal", FIRST_NEW_TERMINAL + 1));
        let source = window.find(("terminal", INITIAL_WEB_TERMINAL));
        assert!(right.visible() && below.visible() && source.visible());
        assert!(right.bounds().bottom() <= below.bounds().top());
        assert!(source.bounds().right() <= below.bounds().left());
        assert_eq!(below.focused(), Some(true));
        assert_eq!(workspace.read(cx).terminal_count(HostId(WEB_01), cx), 3);
        // A split is another connection, and closes without closing its source.
        window.press(default_keys("close-tab"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .try_find(("terminal", FIRST_NEW_TERMINAL + 1))
                .is_none()
        );
        let surviving_pane = window.find(("terminal", FIRST_NEW_TERMINAL));
        assert!(surviving_pane.visible());
        assert_eq!(surviving_pane.focused(), Some(true));
        assert_eq!(workspace.read(cx).terminal_count(HostId(WEB_01), cx), 2);
        // Closing the original pane keeps its sibling in the same tab slot.
        window.click(("terminal", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("close-tab"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .try_find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .is_none()
        );
        assert!(window.find(("terminal-tab", FIRST_NEW_TERMINAL)).visible());
        assert_eq!(
            window.find(("terminal", FIRST_NEW_TERMINAL)).focused(),
            Some(true)
        );
        assert!(
            window
                .try_find(("dim-remote-pane", FIRST_NEW_TERMINAL))
                .is_none()
        );
        assert_eq!(
            workspace.read(cx).active_tab(),
            terminal(FIRST_NEW_TERMINAL)
        );
        assert_eq!(workspace.read(cx).terminal_count(HostId(WEB_01), cx), 1);
    });
}

#[gpui_kit::test]
fn split_shortcuts_work_with_a_single_local_terminal_and_leave_other_views_alone(
    cx: &mut TestAppContext,
) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    in_frame(cx, handle, |window, cx| {
        window.click("new-local-terminal", cx)
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-left"), cx);
    });
    assert_eq!(
        active_tab(&workspace, cx),
        Some(CenterTab::LocalTerminal(LocalTerminalId(2)))
    );
    in_frame(cx, handle, |window, cx| {
        let first = window.find(("local-terminal", 1u64));
        let second = window.find(("local-terminal", 2u64));
        assert!(first.visible() && second.visible());
        assert!(first.bounds().right() <= second.bounds().left());
        assert_eq!(second.focused(), Some(true));
        window.press(default_keys("split-terminal-down"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        let above = window.find(("local-terminal", 2u64));
        let below = window.find(("local-terminal", 3u64));
        assert!(above.bounds().bottom() <= below.bounds().top());
        assert_eq!(below.focused(), Some(true));
        window.click("open-settings", cx);
    });
    for id in ["split-terminal-left", "split-terminal-down"] {
        in_frame(cx, handle, |window, cx| window.press(default_keys(id), cx));
    }
    assert_eq!(active_tab(&workspace, cx), Some(CenterTab::Settings));
    cx.update(|cx| {
        assert!(
            workspace
                .read(cx)
                .local_terminal(LocalTerminalId(4))
                .is_none()
        )
    });
}

#[gpui_kit::test]
fn splitting_one_tab_keeps_other_tabs_full_size_and_restores_its_focused_pane(
    cx: &mut TestAppContext,
) {
    let (handle, workspace) = open_workspace(cx);
    let full_bounds = in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
        window.find(("terminal", INITIAL_WEB_TERMINAL)).bounds()
    });
    in_frame(cx, handle, |window, cx| {
        window.click("new-local-terminal", cx);
    });
    for id in ["split-terminal-left", "split-terminal-down"] {
        in_frame(cx, handle, |window, cx| window.press(default_keys(id), cx));
    }
    let original_width = in_frame(cx, handle, |window, cx| {
        let left = window.find(("local-terminal", 1u64)).bounds();
        let right = window.find(("local-terminal", 2u64)).bounds();
        let divider = point((left.right() + right.left()) / 2., left.center().y);
        window.drag(divider, divider + point(px(48.), px(0.)), cx);
        left.size.width
    });
    let split_bounds = in_frame(cx, handle, |window, cx| {
        let first = window.find(("local-terminal", 1u64)).bounds();
        let second = window.find(("local-terminal", 2u64)).bounds();
        let third = window.find(("local-terminal", 3u64)).bounds();
        assert!(first.size.width > original_width);
        assert!(first.right() <= second.left());
        assert!(second.bottom() <= third.top());
        assert!(window.try_find(("local-terminal-tab", 2u64)).is_none());
        assert!(window.try_find(("local-terminal-tab", 3u64)).is_none());
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
        [first, second, third]
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window.find(("terminal", INITIAL_WEB_TERMINAL)).bounds(),
            full_bounds
        );
        for id in 1..=3u64 {
            assert!(window.try_find(("local-terminal", id)).is_none());
        }
        window.click(("local-terminal-tab", 1u64), cx);
    });
    in_frame(cx, handle, |window, cx| {
        for (id, bounds) in (1..=3u64).zip(split_bounds) {
            assert_eq!(window.find(("local-terminal", id)).bounds(), bounds);
        }
        assert_eq!(window.find(("local-terminal", 3u64)).focused(), Some(true));
        assert!(window.try_find(("dim-local-pane", 3u64)).is_none());
        window.click(("close-local-terminal", 1u64), cx);
    });
    in_frame(cx, handle, |window, cx| {
        for id in 1..=3u64 {
            assert!(
                workspace
                    .read(cx)
                    .local_terminal(LocalTerminalId(id))
                    .is_none()
            );
        }
        assert!(
            window
                .find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .visible()
        );
    });
}

#[gpui_kit::test]
async fn dragging_panes_between_tabs_keeps_their_running_connections(cx: &mut TestAppContext) {
    use std::{
        sync::{Arc, atomic::Ordering},
        time::Duration,
    };

    let factory = Arc::new(FakeTerminalFactory::printing("kept output"));
    let (handle, workspace) = open_workspace_with_terminals(
        cx,
        HostStore::seed(),
        factory.clone(),
        fake_serial_terminals(),
    );
    in_frame(cx, handle, |window, cx| {
        window.activate_window();
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-left"), cx);
    });
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, _| {
        factory.starts.load(Ordering::SeqCst) == 3
    })
    .await;
    let panel = workspace.read_with(cx, |workspace, _| {
        workspace
            .remote_terminal(RemoteTerminalId(FIRST_NEW_TERMINAL))
            .unwrap()
            .clone()
    });
    cx.wait_for(handle.into(), Duration::from_secs(3), |_, cx| {
        panel
            .read(cx)
            .terminal()
            .read(cx)
            .screen_text(cx)
            .contains("kept output")
    })
    .await;
    let staging = workspace.read_with(cx, |workspace, _| {
        workspace
            .remote_terminal(RemoteTerminalId(INITIAL_STAGING_TERMINAL))
            .unwrap()
            .clone()
    });

    in_frame(cx, handle, |window, cx| {
        window.drag_to(
            ("drag-remote-pane", FIRST_NEW_TERMINAL),
            ("terminal-tab", INITIAL_STAGING_TERMINAL),
            cx,
        );
    });
    in_frame(cx, handle, |window, cx| {
        let left = window.find(("terminal", INITIAL_STAGING_TERMINAL));
        let right = window.find(("terminal", FIRST_NEW_TERMINAL));
        assert!(left.bounds().right() <= right.bounds().left());
        assert_eq!(right.focused(), Some(true));
        assert!(
            window
                .try_find(("terminal", INITIAL_WEB_TERMINAL))
                .is_none()
        );
        assert_eq!(
            workspace
                .read(cx)
                .remote_terminal(RemoteTerminalId(FIRST_NEW_TERMINAL)),
            Some(&panel)
        );
        assert!(
            panel
                .read(cx)
                .terminal()
                .read(cx)
                .screen_text(cx)
                .contains("kept output")
        );
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(
            window
                .try_find(("drag-remote-pane", INITIAL_WEB_TERMINAL))
                .is_none()
        );
        assert!(window.try_find(("terminal", FIRST_NEW_TERMINAL)).is_none());
        window.click(("terminal-tab", INITIAL_STAGING_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        use gpui_kit::component::{
            Placement,
            dock::{DockPlacement, InsertTarget, PanelId},
        };
        // Place Staging alongside Web as if its whole tab had been dragged.
        let area = workspace.read(cx).dock_area().clone();
        area.update(cx, |area, cx| {
            let staging = PanelId::from(staging.entity_id());
            let node = area
                .layout(DockPlacement::Center)
                .unwrap()
                .find_panel_node(staging)
                .unwrap();
            area.move_panel(
                staging,
                InsertTarget::Split {
                    node,
                    placement: Placement::Right,
                    size: None,
                },
                window,
                cx,
            );
        });
    });
    in_frame(cx, handle, |window, cx| {
        // The original pane can leave its tab too. Drop it below Web.
        let from = window
            .find(("drag-remote-pane", INITIAL_STAGING_TERMINAL))
            .bounds()
            .center();
        let web = window.find(("terminal", INITIAL_WEB_TERMINAL)).bounds();
        window.drag(from, point(web.center().x, web.bottom() - px(8.)), cx);
    });
    in_frame(cx, handle, |window, cx| {
        let web = window.find(("terminal", INITIAL_WEB_TERMINAL));
        let moved = window.find(("terminal", INITIAL_STAGING_TERMINAL));
        assert!(web.bounds().bottom() <= moved.bounds().top());
        assert!(window.find(("terminal", FIRST_NEW_TERMINAL)).visible());
        assert_eq!(
            window
                .find(("terminal", INITIAL_STAGING_TERMINAL))
                .focused(),
            Some(true)
        );
        assert!(window.find(("terminal-tab", FIRST_NEW_TERMINAL)).visible());
        assert!(
            window
                .try_find(("terminal-tab", INITIAL_STAGING_TERMINAL))
                .is_none()
        );
        assert_eq!(
            workspace
                .read(cx)
                .remote_terminal(RemoteTerminalId(INITIAL_STAGING_TERMINAL)),
            Some(&staging)
        );
        assert_eq!(factory.starts.load(Ordering::SeqCst), 3);
    });
}

#[gpui_kit::test]
fn hovering_another_tab_reveals_it_and_previews_each_drop_side(cx: &mut TestAppContext) {
    use gpui_kit::{MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PlatformInput};

    let (handle, _) = open_workspace_with_store(cx, HostStore::empty());
    for _ in 0..2 {
        in_frame(cx, handle, |window, cx| {
            window.click("new-local-terminal", cx)
        });
    }
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-left"), cx);
    });
    let drag_move = |window: &mut gpui_kit::Window, position, cx: &mut App| {
        window.dispatch_event(
            PlatformInput::MouseMove(MouseMoveEvent {
                position,
                pressed_button: Some(MouseButton::Left),
                modifiers: Default::default(),
            }),
            cx,
        );
    };
    in_frame(cx, handle, |window, cx| {
        let from = window.find(("drag-local-pane", 3u64)).bounds().center();
        let tab = window.find(("local-terminal-tab", 1u64)).bounds().center();
        window.dispatch_event(
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: from,
                modifiers: Default::default(),
                click_count: 1,
                first_mouse: false,
            }),
            cx,
        );
        drag_move(window, from + point(px(8.), px(0.)), cx);
        drag_move(window, tab, cx);
    });
    let bounds = in_frame(cx, handle, |window, cx| {
        assert!(cx.has_active_drag(), "hovering does not finish the drag");
        assert!(window.find(("local-terminal", 1u64)).visible());
        assert!(
            window.try_find(("local-terminal", 3u64)).is_none(),
            "the pane hasn't moved yet"
        );
        window.find("terminal-tab-drop-target").bounds()
    });
    let positions = [
        (
            "pane-drop-left",
            point(bounds.left() + px(16.), bounds.center().y),
        ),
        (
            "pane-drop-right",
            point(bounds.right() - px(16.), bounds.center().y),
        ),
        (
            "pane-drop-top",
            point(bounds.center().x, bounds.top() + px(16.)),
        ),
        (
            "pane-drop-bottom",
            point(bounds.center().x, bounds.bottom() - px(16.)),
        ),
    ];
    for (side, position) in positions {
        in_frame(cx, handle, |window, cx| drag_move(window, position, cx));
        in_frame(cx, handle, |window, _| {
            for (id, _) in positions {
                let preview = window.find(id);
                assert_eq!(
                    preview.visible(),
                    id == side,
                    "preview: {id}, pointer: {side}"
                );
                if id == side {
                    let half = preview.bounds();
                    assert!(bounds.contains(&half.center()));
                    let center = bounds.center();
                    let offset = match side {
                        "pane-drop-left" => half.right() - center.x,
                        "pane-drop-right" => half.left() - center.x,
                        "pane-drop-top" => half.bottom() - center.y,
                        _ => half.top() - center.y,
                    };
                    // A split through a fractional pixel is snapped to the device grid.
                    assert!(offset.abs() <= px(1.));
                }
            }
        });
    }
    in_frame(cx, handle, |window, cx| {
        window.dispatch_event(
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: positions[3].1,
                modifiers: Default::default(),
                click_count: 1,
            }),
            cx,
        );
    });
    in_frame(cx, handle, |window, cx| {
        assert!(!cx.has_active_drag());
        assert!(window.try_find("pane-drop-bottom").is_none());
        let above = window.find(("local-terminal", 1u64));
        let below = window.find(("local-terminal", 3u64));
        assert!(above.bounds().bottom() <= below.bounds().top());
        assert_eq!(below.focused(), Some(true));
    });
}

#[gpui_kit::test]
fn dragging_a_pane_to_another_panes_edge_rearranges_the_current_tab(cx: &mut TestAppContext) {
    let (handle, _) = open_workspace_with_store(cx, HostStore::empty());
    in_frame(cx, handle, |window, cx| {
        window.click("new-local-terminal", cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("split-terminal-left"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        let from = window.find(("drag-local-pane", 2u64)).bounds().center();
        let left = window.find(("local-terminal-pane", 1u64)).bounds();
        window.drag(from, point(left.left() + px(8.), left.center().y), cx);
    });
    in_frame(cx, handle, |window, _| {
        let first = window.find(("local-terminal", 1u64));
        let second = window.find(("local-terminal", 2u64));
        assert!(second.bounds().right() <= first.bounds().left());
        assert_eq!(second.focused(), Some(true));
        assert!(window.find(("local-terminal-tab", 1u64)).visible());
    });
}

#[gpui_kit::test]
async fn quick_connect_shortcut_searches_and_opens_a_host_from_a_terminal(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx);
    });
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("quick-connect"), cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("quick-connect").visible());
        window.input("db-01", cx);
    });
    cx.wait_for(handle.into(), Duration::from_secs(2), |window, cx| {
        window.render_frame(cx);
        window.try_find(("quick-connect-host", WEB_01)).is_none()
    })
    .await;
    in_frame(cx, handle, |window, cx| {
        assert!(window.find(("quick-connect-host", DB_01)).visible());
        window.press("enter", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert!(window.try_find("quick-connect").is_none());
        assert_eq!(
            window.find(("terminal-tab", FIRST_NEW_TERMINAL)).label(),
            Some("db-01")
        );
        assert_eq!(workspace.read(cx).terminal_count(HostId(DB_01), cx), 1);
    });
}

/// The workspace's opener and SFTP's folder dialog keep separate keys.
#[gpui_kit::test]
async fn quick_connect_and_sftp_directory_shortcuts_open_their_own_dialogs(
    cx: &mut TestAppContext,
) {
    let (handle, _) = open_workspace_with_sftp(cx, Arc::new(FakeSftpProvider::default()));
    open_test_explorer(cx, handle).await;
    let directory_keys = if cfg!(target_os = "macos") {
        "cmd-shift-o"
    } else {
        "ctrl-o"
    };
    for pane in ["local-pane", "remote-pane"] {
        press_on_row(cx, handle, pane, "目录", directory_keys);
        in_frame(cx, handle, |window, cx| {
            assert!(window.find("open-directory-path").visible());
            assert!(window.try_find("quick-connect").is_none());
            window.press("escape", cx);
        });
        press_on_row(cx, handle, pane, "目录", default_keys("quick-connect"));
        in_frame(cx, handle, |window, cx| {
            assert!(window.find("quick-connect").visible());
            assert!(window.try_find("open-directory-path").is_none());
            window.press("escape", cx);
        });
    }
}

/// Open 设置 › 键盘快捷键, the fifth category.
fn open_shortcuts(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    in_frame(cx, handle, |window, cx| window.click("open-settings", cx));
    in_frame(cx, handle, |window, cx| {
        window.within("settings").click("0-4", cx)
    });
}

/// Scroll the page down to the tab shortcuts, below the window's fold.
fn scroll_to_tab_shortcuts(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    in_frame(cx, handle, |window, cx| {
        let position = window.find("shortcut-keys-new-host").bounds().center();
        window.dispatch_event(
            gpui_kit::ScrollWheelEvent {
                position,
                delta: gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-600.))),
                modifiers: gpui_kit::Modifiers::default(),
                touch_phase: gpui_kit::TouchPhase::Moved,
            }
            .to_platform_input(),
            cx,
        );
    });
}

fn overrides(workspace: &Entity<Workspace>, cx: &mut TestAppContext) -> serde_json::Value {
    cx.update(|cx| {
        serde_json::to_value(workspace.read(cx).settings().read(cx).settings().shortcuts).unwrap()
    })
}

#[gpui_kit::test]
fn tabs_are_switched_from_the_keyboard(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx)
    });
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));

    let switch = default_keys("switch-to-tab");
    let press = |keys: String, cx: &mut TestAppContext| {
        in_frame(cx, handle, |window, cx| window.press(&keys, cx))
    };
    press(default_keys("next-tab").into(), cx);
    assert_eq!(
        active_tab(&workspace, cx),
        terminal(INITIAL_STAGING_TERMINAL)
    );
    // Around the end and back.
    press(default_keys("next-tab").into(), cx);
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
    press(default_keys("previous-tab").into(), cx);
    assert_eq!(
        active_tab(&workspace, cx),
        terminal(INITIAL_STAGING_TERMINAL)
    );
    press(switch.into(), cx);
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
    // 9 is the last; past the last, nothing.
    press(switch.replace('1', "9"), cx);
    assert_eq!(
        active_tab(&workspace, cx),
        terminal(INITIAL_STAGING_TERMINAL)
    );
    press(switch.replace('1', "5"), cx);
    assert_eq!(
        active_tab(&workspace, cx),
        terminal(INITIAL_STAGING_TERMINAL)
    );
}

#[gpui_kit::test]
fn a_shortcut_takes_the_keys_pressed_for_it(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    open_shortcuts(cx, handle);
    scroll_to_tab_shortcuts(cx, handle);
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("shortcut-keys-next-tab").visible());
        window.click("shortcut-keys-next-tab", cx);
    });
    in_frame(cx, handle, |window, cx| {
        let recorder = window.find("shortcut-recorder-next-tab");
        assert!(recorder.visible());
        assert_eq!(recorder.focused(), Some(true));
        window.press(&primary("right"), cx);
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("shortcut-recorder-next-tab").is_none());
    });
    let pressed = Keystroke::parse(&primary("right")).unwrap().unparse();
    assert_eq!(
        overrides(&workspace, cx),
        serde_json::json!({ "next-tab": pressed })
    );

    // The new keys move on from the settings tab, the last, to the first;
    // the old ones no longer do anything.
    assert_eq!(active_tab(&workspace, cx), Some(CenterTab::Settings));
    in_frame(cx, handle, |window, cx| window.press(&primary("right"), cx));
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
    in_frame(cx, handle, |window, cx| {
        window.click("host-search", cx);
        window.input("abcd", cx);
        window.press("home", cx);
        window.press(&primary("right"), cx);
        window.input("x", cx);
        assert_eq!(window.find("host-search").value(), Some("abcdx"));
    });
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("next-tab"), cx)
    });
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
}

#[gpui_kit::test]
fn a_shortcut_turned_off_does_nothing_until_put_back(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    open_shortcuts(cx, handle);
    scroll_to_tab_shortcuts(cx, handle);
    in_frame(cx, handle, |window, cx| {
        window.click("shortcut-disable-next-tab", cx)
    });
    assert_eq!(
        overrides(&workspace, cx),
        serde_json::json!({ "next-tab": null })
    );
    in_frame(cx, handle, |window, cx| {
        assert_eq!(window.find("shortcut-keys-next-tab").label(), Some("无"));
        window.press(default_keys("next-tab"), cx);
    });
    assert_eq!(active_tab(&workspace, cx), Some(CenterTab::Settings));

    in_frame(cx, handle, |window, cx| {
        window.click("shortcut-reset-next-tab", cx)
    });
    assert_eq!(overrides(&workspace, cx), serde_json::json!({}));
    in_frame(cx, handle, |window, cx| {
        window.press(default_keys("next-tab"), cx)
    });
    assert_eq!(active_tab(&workspace, cx), terminal(INITIAL_WEB_TERMINAL));
}

#[gpui_kit::test]
fn keys_that_are_taken_or_plain_are_refused_and_escape_gives_up(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    open_shortcuts(cx, handle);
    scroll_to_tab_shortcuts(cx, handle);
    in_frame(cx, handle, |window, cx| {
        window.click("shortcut-keys-next-tab", cx)
    });
    // A plain letter and another shortcut's keys: each said
    // and refused, still listening. ⌘W does not close the tab meanwhile.
    for keys in [
        "j".to_string(),
        default_keys("new-host").to_string(),
        default_keys("close-tab").to_string(),
    ] {
        in_frame(cx, handle, |window, cx| window.press(&keys, cx));
        in_frame(cx, handle, |window, _| {
            assert!(
                window.find("shortcut-recorder-next-tab").visible(),
                "{keys}"
            );
        });
    }
    in_frame(cx, handle, |window, cx| {
        assert_eq!(window.notifications(cx).len(), 3);
    });
    assert_eq!(active_tab(&workspace, cx), Some(CenterTab::Settings));
    in_frame(cx, handle, |window, cx| window.press("escape", cx));
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("shortcut-recorder-next-tab").is_none());
        assert!(window.find("shortcut-keys-next-tab").visible());
    });
    assert_eq!(overrides(&workspace, cx), serde_json::json!({}));
}

#[gpui_kit::test]
fn the_system_wide_shortcut_starts_with_no_key_and_takes_one_the_system_knows(
    cx: &mut TestAppContext,
) {
    let (handle, workspace) = open_workspace(cx);
    open_shortcuts(cx, handle);
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window.find("shortcut-keys-show-hide-app").label(),
            Some("无")
        );
        window.click("shortcut-keys-show-hide-app", cx);
    });
    // No key the system has a name for: said and refused, still listening.
    in_frame(cx, handle, |window, cx| window.press("ctrl-alt-+", cx));
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("shortcut-recorder-show-hide-app").visible());
        assert_eq!(window.notifications(cx).len(), 1);
        window.press("alt-space", cx);
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("shortcut-recorder-show-hide-app").is_none());
    });
    assert_eq!(
        overrides(&workspace, cx),
        serde_json::json!({ "show-hide-app": "alt-space" })
    );
}

#[gpui_kit::test]
fn the_zoom_keys_size_a_terminals_text_and_the_interface_elsewhere(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    let terminal_size = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            workspace
                .read(cx)
                .settings()
                .read(cx)
                .settings()
                .terminal_font
                .size
        })
    };
    // What the interface is drawn at, and what 外观 › 界面字号 holds.
    let interface_size = |cx: &mut TestAppContext| {
        cx.update(|cx| {
            let saved = workspace
                .read(cx)
                .settings()
                .read(cx)
                .settings()
                .interface
                .font_size;
            assert_eq!(cx.theme().font_size.as_f32(), saved);
            saved
        })
    };
    in_frame(cx, handle, |window, cx| {
        window.click(("terminal-tab", INITIAL_WEB_TERMINAL), cx)
    });
    in_frame(cx, handle, |window, _| {
        assert_eq!(
            window.find(("terminal", INITIAL_WEB_TERMINAL)).focused(),
            Some(true)
        );
    });
    let press = |keys: String, cx: &mut TestAppContext| {
        in_frame(cx, handle, |window, cx| window.press(&keys, cx))
    };

    // 外观 › 界面字号 by default: 16, or 14 on Windows.
    let default = DEFAULT_INTERFACE_FONT_SIZE;

    // In a terminal: its text, a pixel a step, kept in the settings.
    press(primary("="), cx);
    assert_eq!((terminal_size(cx), interface_size(cx)), (14., default));
    press(primary("-"), cx);
    press(primary("-"), cx);
    assert_eq!((terminal_size(cx), interface_size(cx)), (12., default));
    // No smaller than 设置 › 终端 › 字号 goes.
    for _ in 0..5 {
        press(primary("-"), cx);
    }
    assert_eq!(terminal_size(cx), 10.);
    press(primary("0"), cx);
    assert_eq!((terminal_size(cx), interface_size(cx)), (13., default));

    // Elsewhere, the interface: two pixels a step, kept in the settings.
    in_frame(cx, handle, |window, cx| window.click("open-settings", cx));
    press(primary("="), cx);
    assert_eq!((terminal_size(cx), interface_size(cx)), (13., default + 2.));
    // No larger than 设置 › 外观 › 界面字号 goes.
    for _ in 0..3 {
        press(primary("="), cx);
    }
    assert_eq!(interface_size(cx), 20.);
    press(primary("0"), cx);
    assert_eq!((terminal_size(cx), interface_size(cx)), (13., default));
    // The settings' own one pixel steps, which the keys step on from.
    let settings = cx.update(|cx| workspace.read(cx).settings().clone());
    cx.update(|cx| {
        settings.update(cx, |settings, cx| {
            settings.update(|settings| settings.interface.font_size = 15., cx)
        })
    });
    cx.run_until_parked();
    assert_eq!(interface_size(cx), 15.);
    press(primary("-"), cx);
    assert_eq!(interface_size(cx), 13.);
}
