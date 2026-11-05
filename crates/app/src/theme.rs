use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::{App, Global, rgb};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Palette {
    pub bg: u32,
    pub surface: u32,
    pub raised: u32,
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
    bg: 0x111315,
    surface: 0x181b1e,
    raised: 0x202427,
    content: 0xe9ecea,
    muted: 0xa3aaa6,
    line: 0x272b2e,
    line_strong: 0x737a76,
    accent: 0x34d399,
    on_accent: 0x04261a,
    danger: 0xf87171,
    status_planned: 0xa3aaa6,
    status_active: 0x38bdf8,
    status_paused: 0xfbbf24,
    status_done: 0x34d399,
    status_dropped: 0xfb7185,
};

pub const LIGHT: Palette = Palette {
    bg: 0xeceeed,
    surface: 0xf7f8f7,
    raised: 0xffffff,
    content: 0x14201b,
    muted: 0x4f5c56,
    line: 0xd5dad7,
    line_strong: 0x7d8782,
    accent: 0x047857,
    on_accent: 0xffffff,
    danger: 0xb91c1c,
    status_planned: 0x4f5c56,
    status_active: 0x0369a1,
    status_paused: 0xb45309,
    status_done: 0x047857,
    status_dropped: 0xbe123c,
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
        theme.muted = rgb(palette.surface).into();
        theme.muted_foreground = rgb(palette.muted).into();
        theme.popover = rgb(palette.raised).into();
        theme.popover_foreground = rgb(palette.content).into();
        theme.primary = rgb(palette.accent).into();
        theme.primary_foreground = rgb(palette.on_accent).into();
        theme.ring = rgb(palette.accent).into();
        theme.danger = rgb(palette.danger).into();
        theme.sidebar = rgb(palette.surface).into();
    });
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
        for background in [palette.bg, palette.surface, palette.raised] {
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
