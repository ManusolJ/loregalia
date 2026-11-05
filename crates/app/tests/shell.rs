use gpui_kit::component::ThemeMode;
use gpui_kit::test::TestWindowExt;
use gpui_kit::{
    AnyWindowHandle, AppContext, Bounds, Pixels, Point, Result, Role, Size, TestAppContext,
    WindowBounds, WindowOptions, px, size,
};
use loregalia_app::shell::Shell;
use loregalia_app::theme;

fn open_shell(cx: &mut TestAppContext, window_size: Size<Pixels>) -> Result<AnyWindowHandle> {
    cx.update(|cx| {
        gpui_kit::init(cx);
        theme::apply(ThemeMode::Dark, cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: Point::default(),
                size: window_size,
            })),
            ..Default::default()
        };
        let (window, _) = gpui_kit::open_window(options, cx, |_, cx| cx.new(|_| Shell))?;

        Ok(window)
    })
}

#[gpui_kit::test]
fn wide_window_shows_the_full_sidebar_and_empty_state(cx: &mut TestAppContext) {
    let handle = open_shell(cx, size(px(1280.0), px(800.0))).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(240.0));

        let empty_state = window.find("empty-state");
        assert!(empty_state.visible());
        assert_eq!(empty_state.role(), Some(Role::Status));
        assert_eq!(empty_state.label(), Some("Nothing planned yet"));
    })
    .unwrap();
}

#[gpui_kit::test]
fn narrow_window_collapses_the_sidebar_to_a_rail(cx: &mut TestAppContext) {
    let handle = open_shell(cx, size(px(960.0), px(600.0))).unwrap();

    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);

        assert_eq!(window.find("sidebar").bounds().size.width, px(56.0));
        assert!(window.find("empty-state").visible());
    })
    .unwrap();
}
