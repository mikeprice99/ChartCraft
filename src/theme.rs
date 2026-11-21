//! Light and dark colours for the window around the diagram (menus, toolbar, toolbox, rulers,
//! scroll bars and dialogs). The diagram itself keeps its own colours, like paper on a desk.

use egui::Color32;

/// Which look the person picked in View → Theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeChoice {
    System,
    Light,
    Dark,
}

impl ThemeChoice {
    pub const ALL: [ThemeChoice; 3] = [ThemeChoice::System, ThemeChoice::Light, ThemeChoice::Dark];

    pub fn label(self) -> &'static str {
        match self {
            ThemeChoice::System => "Same as system",
            ThemeChoice::Light => "Light",
            ThemeChoice::Dark => "Dark",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ThemeChoice::System => "system",
            ThemeChoice::Light => "light",
            ThemeChoice::Dark => "dark",
        }
    }

    pub fn parse(s: &str) -> Option<ThemeChoice> {
        ThemeChoice::ALL.into_iter().find(|t| t.key().eq_ignore_ascii_case(s.trim()))
    }

    pub fn preference(self) -> egui::ThemePreference {
        match self {
            ThemeChoice::System => egui::ThemePreference::System,
            ThemeChoice::Light => egui::ThemePreference::Light,
            ThemeChoice::Dark => egui::ThemePreference::Dark,
        }
    }
}

/// The colours of the window, for the theme egui is showing right now.
#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    /// Status bar, tab strip and side panels.
    pub panel: Color32,
    /// Menu bar, toolbar and rulers.
    pub bar: Color32,
    /// Plain text and the lines of small pictures (icons, arrows, line widths).
    pub ink: Color32,
    /// Greyed-out hints and notes.
    pub hint: Color32,
    pub ink_disabled: Color32,
    /// Icon parts that stand for a sheet of paper.
    pub paper: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub selected_border: Color32,
    pub hover_border: Color32,
    /// Toolbox and attribute buttons.
    pub cell: Color32,
    pub cell_border: Color32,
    pub tab_idle: Color32,
    pub tab_text_idle: Color32,
    pub tab_border: Color32,
    /// List boxes in dialogs.
    pub field: Color32,
    pub field_border: Color32,
    pub ruler_edge: Color32,
    pub track: Color32,
    pub thumb: Color32,
    pub thumb_hot: Color32,
    pub danger: Color32,
    pub good: Color32,
    pub close_x: Color32,
}

pub const LIGHT: Pal = Pal {
    dark: false,
    panel: Color32::from_rgb(240, 240, 240),
    bar: Color32::WHITE,
    ink: Color32::from_rgb(20, 20, 20),
    hint: Color32::from_gray(120),
    ink_disabled: Color32::from_gray(160),
    paper: Color32::WHITE,
    hover: Color32::from_rgb(229, 241, 251),
    selected: Color32::from_rgb(204, 228, 247),
    selected_border: Color32::from_rgb(90, 140, 210),
    hover_border: Color32::from_rgb(126, 180, 234),
    cell: Color32::from_rgb(253, 253, 253),
    cell_border: Color32::from_rgb(214, 214, 214),
    tab_idle: Color32::from_rgb(225, 225, 225),
    tab_text_idle: Color32::from_gray(70),
    tab_border: Color32::from_gray(205),
    field: Color32::WHITE,
    field_border: Color32::from_gray(120),
    ruler_edge: Color32::BLACK,
    track: Color32::from_rgb(236, 236, 236),
    thumb: Color32::from_gray(185),
    thumb_hot: Color32::from_gray(150),
    danger: Color32::from_rgb(200, 40, 40),
    good: Color32::from_rgb(30, 110, 40),
    close_x: Color32::from_gray(120),
};

pub const DARK: Pal = Pal {
    dark: true,
    panel: Color32::from_rgb(32, 33, 36),
    bar: Color32::from_rgb(43, 44, 48),
    ink: Color32::from_rgb(222, 222, 222),
    hint: Color32::from_gray(150),
    ink_disabled: Color32::from_gray(100),
    paper: Color32::from_rgb(58, 60, 66),
    hover: Color32::from_rgb(50, 62, 80),
    selected: Color32::from_rgb(38, 72, 112),
    selected_border: Color32::from_rgb(100, 155, 225),
    hover_border: Color32::from_rgb(80, 120, 175),
    cell: Color32::from_rgb(50, 51, 56),
    cell_border: Color32::from_rgb(72, 74, 80),
    tab_idle: Color32::from_rgb(40, 41, 45),
    tab_text_idle: Color32::from_gray(165),
    tab_border: Color32::from_gray(64),
    field: Color32::from_rgb(28, 29, 32),
    field_border: Color32::from_gray(85),
    ruler_edge: Color32::from_gray(90),
    track: Color32::from_rgb(38, 39, 43),
    thumb: Color32::from_gray(85),
    thumb_hot: Color32::from_gray(120),
    danger: Color32::from_rgb(235, 95, 95),
    good: Color32::from_rgb(110, 200, 120),
    close_x: Color32::from_gray(150),
};

/// The palette for the theme egui is showing now (after "Same as system" is worked out).
pub fn pal(ctx: &egui::Context) -> Pal {
    if ctx.theme() == egui::Theme::Dark {
        DARK
    } else {
        LIGHT
    }
}

/// Give egui's own widgets (menus, buttons, text boxes, windows) the same backgrounds.
pub fn setup_styles(ctx: &egui::Context) {
    for (theme, p) in [(egui::Theme::Light, LIGHT), (egui::Theme::Dark, DARK)] {
        ctx.style_mut_of(theme, |s| {
            s.spacing.item_spacing = egui::vec2(6.0, 4.0);
            s.visuals.panel_fill = p.panel;
            if p.dark {
                s.visuals.window_fill = p.bar;
                s.visuals.extreme_bg_color = p.field;
            }
        });
    }
}

/// Make a toolbox shape picture (drawn black on white, premultiplied RGBA) suit a dark
/// background: greys are flipped (white fill to dark, black lines to light), colours kept.
pub fn darken_icon(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let (r, g, b, a) = (px[0] as i32, px[1] as i32, px[2] as i32, px[3] as i32);
        if a == 0 || (r - g).abs() > 12 || (g - b).abs() > 12 || (r - b).abs() > 12 {
            continue;
        }
        // Premultiplied: a grey of value v at alpha a is stored as v*a; flipping gives (1-v)*a.
        // Keep the flipped fill a little lighter than black so it reads as a shape, not a hole.
        let flip = |c: i32| {
            let v = (a - c).max(0);
            (v * 205 / 255 + a * 40 / 255).min(a) as u8
        };
        px[0] = flip(r);
        px[1] = flip(g);
        px[2] = flip(b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_choice_round_trips() {
        for t in ThemeChoice::ALL {
            assert_eq!(ThemeChoice::parse(t.key()), Some(t));
        }
        assert_eq!(ThemeChoice::parse(" Dark\n"), Some(ThemeChoice::Dark));
        assert_eq!(ThemeChoice::parse("purple"), None);
    }

    #[test]
    fn dark_icons_flip_greys_and_keep_colours() {
        // White fill, black line, red, and a transparent pixel.
        let mut px = vec![255, 255, 255, 255, 0, 0, 0, 255, 200, 30, 30, 255, 0, 0, 0, 0];
        darken_icon(&mut px);
        assert!(px[0] < 60, "white fill becomes dark");
        assert!(px[4] > 200, "black line becomes light");
        assert_eq!(&px[8..12], &[200, 30, 30, 255]);
        assert_eq!(&px[12..16], &[0, 0, 0, 0]);
    }
}
