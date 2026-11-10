use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, Hsla, WindowAppearance, rgb};
use loregalia_core::settings::Theme as ThemeChoice;

pub const DISPLAY_FONT: &str = "Segoe UI Variable Display";
pub const SELECTED_ALPHA: f32 = 0.15;
pub const HOVER_ALPHA: f32 = 0.05;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    pub bg: u32,
    pub surface: u32,
    pub raised: u32,
    pub sunken: u32,
    pub content: u32,
    pub muted: u32,
    pub line: u32,
    pub line_strong: u32,
    pub accent: u32,
    pub on_accent: u32,
    pub danger: u32,
    pub status_planned: u32,
    pub status_active: u32,
    pub status_paused: u32,
    pub status_done: u32,
    pub status_dropped: u32,
}

impl Global for Palette {}

pub const DARK: Palette = Palette {
    bg: 0x121420,
    surface: 0x181b29,
    raised: 0x20243a,
    sunken: 0x0d0f18,
    content: 0xeceaf4,
    muted: 0xa3a6bd,
    line: 0x2a2e44,
    line_strong: 0x6e7391,
    accent: 0xf0a742,
    on_accent: 0x241503,
    danger: 0xff7a8a,
    status_planned: 0xa3a6bd,
    status_active: 0x6cb6ff,
    status_paused: 0xb49cff,
    status_done: 0x4fd1a5,
    status_dropped: 0xff7a8a,
};

pub const LIGHT: Palette = Palette {
    bg: 0xf4f5fa,
    surface: 0xffffff,
    raised: 0xffffff,
    sunken: 0xeceef5,
    content: 0x1b1d2b,
    muted: 0x5b5f78,
    line: 0xe2e4ee,
    line_strong: 0x7a7f99,
    accent: 0x9a5405,
    on_accent: 0xffffff,
    danger: 0xb82d42,
    status_planned: 0x5b5f78,
    status_active: 0x1f6fd1,
    status_paused: 0x7a5bd6,
    status_done: 0x12805c,
    status_dropped: 0xc8364a,
};

pub fn apply(mode: ThemeMode, cx: &mut App) {
    let palette = if mode.is_dark() { DARK } else { LIGHT };
    cx.set_global(palette);
    Theme::change(mode, None, cx);
    Theme::update(cx, |theme| {
        theme.background = rgb(palette.bg).into();
        theme.foreground = rgb(palette.content).into();
        theme.border = rgb(palette.line).into();
        theme.input = rgb(palette.line_strong).into();
        theme.muted = rgb(palette.sunken).into();
        theme.muted_foreground = rgb(palette.muted).into();
        theme.popover = rgb(palette.raised).into();
        theme.popover_foreground = rgb(palette.content).into();
        theme.primary = rgb(palette.accent).into();
        theme.primary_foreground = rgb(palette.on_accent).into();
        theme.ring = rgb(palette.accent).into();
        theme.danger = rgb(palette.danger).into();
        theme.sidebar = rgb(palette.surface).into();
        theme.link = rgb(palette.accent).into();

        let accent: Hsla = rgb(palette.accent).into();
        let content: Hsla = rgb(palette.content).into();
        theme.accent = accent.opacity(SELECTED_ALPHA);
        theme.accent_foreground = content;
        theme.secondary_active = accent.opacity(SELECTED_ALPHA);
        theme.secondary_hover = content.opacity(HOVER_ALPHA);
        theme.button_active = accent.opacity(SELECTED_ALPHA);
        theme.primary_hover = accent.opacity(0.9);
        theme.primary_active = accent.opacity(0.8);
        theme.button_primary = accent;
        theme.button_primary_hover = accent.opacity(0.9);
        theme.button_primary_active = accent.opacity(0.8);
        theme.button_primary_foreground = rgb(palette.on_accent).into();
    });
}

pub fn apply_choice(choice: ThemeChoice, cx: &mut App) {
    let mode = match choice {
        ThemeChoice::Dark => ThemeMode::Dark,
        ThemeChoice::Light => ThemeMode::Light,
        ThemeChoice::System => match cx.window_appearance() {
            WindowAppearance::Dark | WindowAppearance::VibrantDark => ThemeMode::Dark,
            _ => ThemeMode::Light,
        },
    };

    apply(mode, cx);
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT, Palette};

    fn luminance(hex: u32) -> f64 {
        let channel = |shift: u32| {
            let value = f64::from((hex >> shift) & 0xff) / 255.0;
            if value <= 0.03928 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };

        0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
    }

    fn contrast(first: u32, second: u32) -> f64 {
        let (first, second) = (luminance(first), luminance(second));

        (first.max(second) + 0.05) / (first.min(second) + 0.05)
    }

    fn assert_readable(palette: Palette) {
        for background in [palette.bg, palette.surface, palette.raised, palette.sunken] {
            let requirements = [
                ("content", palette.content, 7.0),
                ("muted", palette.muted, 4.5),
                ("accent", palette.accent, 4.5),
                ("danger", palette.danger, 4.5),
                ("line_strong", palette.line_strong, 3.0),
                ("status_planned", palette.status_planned, 3.0),
                ("status_active", palette.status_active, 3.0),
                ("status_paused", palette.status_paused, 3.0),
                ("status_done", palette.status_done, 3.0),
                ("status_dropped", palette.status_dropped, 3.0),
            ];

            for (name, color, minimum) in requirements {
                let ratio = contrast(color, background);
                assert!(
                    ratio >= minimum,
                    "{name} on {background:06x} is {ratio:.2}:1, needs {minimum}:1"
                );
            }
        }

        assert!(contrast(palette.on_accent, palette.accent) >= 4.5);
    }

    #[test]
    fn dark_palette_meets_contrast_requirements() {
        assert_readable(DARK);
    }

    #[test]
    fn light_palette_meets_contrast_requirements() {
        assert_readable(LIGHT);
    }
}
