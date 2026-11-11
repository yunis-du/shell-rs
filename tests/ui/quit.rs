//! Quitting asks once, can be canceled, and follows the saved preference.

use crate::support::*;
use shellrs::app::Quit;

#[gpui_kit::test]
fn quit_shortcut_asks_once_and_cancel_keeps_terminals_open(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| window.click("host-search", cx));
    let connection_state = cx.update(|cx| {
        workspace
            .read(cx)
            .remote_terminal(RemoteTerminalId(INITIAL_WEB_TERMINAL))
            .unwrap()
            .read(cx)
            .connection_state(cx)
    });
    in_frame(cx, handle, |window, cx| {
        window.press("secondary-q", cx);
    });
    in_frame(cx, handle, |window, _| {
        assert_eq!(
            window.find("quit-confirmation").label(),
            Some("所有终端和连接都将关闭。")
        );
        assert_eq!(window.find("ok").label(), Some("退出"));
        assert_eq!(window.find("cancel").label(), Some("取消"));
    });

    // Native menus, the tray and a shortcut use the same global action,
    // including while another quit confirmation holds focus.
    cx.update(|cx| cx.dispatch_action(&Quit));
    cx.run_until_parked();
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("quit-confirmation").visible());
        window.click("cancel", cx);
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("quit-confirmation").is_none());
        assert!(
            window
                .find(("terminal-tab", INITIAL_WEB_TERMINAL))
                .visible()
        );
        assert!(
            window
                .find(("terminal-tab", INITIAL_STAGING_TERMINAL))
                .visible()
        );
    });
    cx.update(|cx| {
        assert_eq!(
            workspace
                .read(cx)
                .remote_terminal(RemoteTerminalId(INITIAL_WEB_TERMINAL))
                .unwrap()
                .read(cx)
                .connection_state(cx),
            connection_state
        );
    });

    // Escape also cancels, and another quit can ask again afterward.
    for _ in 0..2 {
        cx.update(|cx| cx.dispatch_action(&Quit));
        cx.run_until_parked();
        in_frame(cx, handle, |window, cx| {
            assert!(window.find("quit-confirmation").visible());
            window.press("escape", cx);
        });
        in_frame(cx, handle, |window, _| {
            assert!(window.try_find("quit-confirmation").is_none());
        });
    }
}

#[gpui_kit::test]
fn quit_confirmation_can_be_accepted_with_an_empty_workspace(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace_with_store(cx, HostStore::empty());
    cx.update(|cx| {
        workspace
            .read(cx)
            .settings()
            .clone()
            .update(cx, |store, cx| {
                store.update(
                    |settings| {
                        settings.language = InterfaceLanguage::English;
                        settings.appearance = Appearance::Dark;
                        settings.interface.font_size = 20.0;
                    },
                    cx,
                );
            });
    });
    cx.run_until_parked();
    cx.update(|cx| cx.dispatch_action(&Quit));
    cx.run_until_parked();
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("quit-confirmation").visible());
        assert_eq!(window.find("ok").label(), Some("Quit"));
        assert_eq!(window.find("cancel").label(), Some("Cancel"));
        assert_eq!(
            window.find("quit-confirmation").label(),
            Some("All terminals and connections will close.")
        );
        assert!(
            window.find("ok").bounds().top() >= window.find("quit-confirmation").bounds().bottom()
        );
        window.press("enter", cx);
    });
    in_frame(cx, handle, |window, _| {
        assert!(window.try_find("quit-confirmation").is_none());
    });
}

#[gpui_kit::test]
fn quit_confirmation_can_be_disabled_and_reset_in_settings(cx: &mut TestAppContext) {
    let (handle, workspace) = open_workspace(cx);
    in_frame(cx, handle, |window, cx| window.click("open-settings", cx));
    in_frame(cx, handle, |window, cx| {
        window.within("settings").click("0-7", cx);
    });
    in_frame(cx, handle, |window, cx| {
        assert_eq!(
            window
                .within("settings")
                .within("group-0")
                .within("item-3")
                .find("check")
                .checked(),
            Some(true)
        );
        window
            .within("settings")
            .within("group-0")
            .within("item-3")
            .click("check", cx);
    });
    cx.update(|cx| {
        assert!(
            !workspace
                .read(cx)
                .settings()
                .read(cx)
                .settings()
                .window
                .confirm_before_quit
        );
        cx.dispatch_action(&Quit);
    });
    cx.run_until_parked();
    in_frame(cx, handle, |window, cx| {
        assert!(window.try_find("quit-confirmation").is_none());
        window.within("settings").click("reset", cx);
    });
    cx.update(|cx| {
        assert!(
            workspace
                .read(cx)
                .settings()
                .read(cx)
                .settings()
                .window
                .confirm_before_quit
        );
        cx.dispatch_action(&Quit);
    });
    cx.run_until_parked();
    in_frame(cx, handle, |window, cx| {
        assert!(window.find("quit-confirmation").visible());
        window.click("cancel", cx);
    });
}
