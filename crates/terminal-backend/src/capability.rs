//! Terminal capability detection and deterministic visual fallbacks.

use editor_types::{
    ColorDepth, StyleRole, TerminalCapabilities, TerminalFeature, TerminalFeatures, UnderlineStyle,
};

/// An RGB color independent of terminal palette depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RgbColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

impl RgbColor {
    #[must_use]
    pub const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }
}

/// A color after adapting it to a terminal's supported palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedColor {
    TrueColor(RgbColor),
    Ansi256(u8),
    Ansi16(u8),
}

/// The rendering behavior used for a requested diagnostic underline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedUnderline {
    Curly,
    Straight,
    Emphasis,
}

/// Semantic colors consumed by the renderer. Higher layers continue to use [`StyleRole`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    colors: [RgbColor; 19],
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            colors: [
                RgbColor::new(212, 212, 212),
                RgbColor::new(30, 30, 30),
                RgbColor::new(38, 79, 120),
                RgbColor::new(42, 45, 46),
                RgbColor::new(133, 133, 133),
                RgbColor::new(51, 51, 51),
                RgbColor::new(0, 122, 204),
                RgbColor::new(37, 37, 38),
                RgbColor::new(244, 71, 71),
                RgbColor::new(204, 167, 0),
                RgbColor::new(117, 190, 255),
                RgbColor::new(181, 206, 168),
                RgbColor::new(115, 201, 145),
                RgbColor::new(197, 134, 192),
                RgbColor::new(240, 71, 71),
                RgbColor::new(97, 175, 239),
                RgbColor::new(152, 195, 121),
                RgbColor::new(206, 145, 120),
                RgbColor::new(106, 153, 85),
            ],
        }
    }
}

impl Theme {
    /// Returns the RGB value assigned to a semantic role.
    #[must_use]
    pub fn color(&self, role: StyleRole) -> RgbColor {
        self.colors[role_index(role)]
    }

    /// Replaces a semantic color.
    pub fn set_color(&mut self, role: StyleRole, color: RgbColor) {
        self.colors[role_index(role)] = color;
    }
}

/// Detects conservative capabilities from standard terminal environment markers.
///
/// Unknown terminals receive portable ANSI fallbacks rather than optimistic private sequences.
#[must_use]
pub fn detect_capabilities() -> TerminalCapabilities {
    let term = environment("TERM");
    let color_term = environment("COLORTERM");
    let windows_terminal = std::env::var_os("WT_SESSION").is_some();
    let kitty = std::env::var_os("KITTY_WINDOW_ID").is_some() || term.contains("kitty");
    let wezterm = std::env::var_os("WEZTERM_PANE").is_some();
    let usable_ansi = !term.eq_ignore_ascii_case("dumb") || windows_terminal;

    let color_depth = if windows_terminal
        || color_term.eq_ignore_ascii_case("truecolor")
        || color_term.eq_ignore_ascii_case("24bit")
    {
        ColorDepth::TrueColor
    } else if term.contains("256color") {
        ColorDepth::Color256
    } else {
        ColorDepth::Color16
    };
    let underline = if windows_terminal || kitty || wezterm {
        UnderlineStyle::Curly
    } else if usable_ansi {
        UnderlineStyle::Straight
    } else {
        UnderlineStyle::None
    };
    let mut features = vec![];
    if usable_ansi {
        features.extend([
            TerminalFeature::Mouse,
            TerminalFeature::BracketedPaste,
            TerminalFeature::AlternateScreen,
        ]);
    }
    if kitty || wezterm {
        features.push(TerminalFeature::EnhancedKeyboard);
    }
    TerminalCapabilities {
        color_depth,
        underline,
        features: TerminalFeatures::from_features(features),
    }
}

/// Resolves one RGB color to the selected terminal color depth.
#[must_use]
pub fn resolve_color(color: RgbColor, depth: ColorDepth) -> ResolvedColor {
    match depth {
        ColorDepth::TrueColor => ResolvedColor::TrueColor(color),
        ColorDepth::Color256 => ResolvedColor::Ansi256(quantize_ansi256(color)),
        ColorDepth::Color16 => ResolvedColor::Ansi16(quantize_ansi16(color)),
    }
}

/// Applies the required curly-to-straight-to-emphasis fallback chain.
#[must_use]
pub const fn resolve_underline(capability: UnderlineStyle) -> ResolvedUnderline {
    match capability {
        UnderlineStyle::Curly => ResolvedUnderline::Curly,
        UnderlineStyle::Straight => ResolvedUnderline::Straight,
        UnderlineStyle::None => ResolvedUnderline::Emphasis,
    }
}

/// Maps RGB to the nearest xterm 256-color palette entry.
#[must_use]
pub fn quantize_ansi256(color: RgbColor) -> u8 {
    (0_u8..=255)
        .min_by_key(|&index| color_distance(color, ansi256_color(index)))
        .unwrap_or(0)
}

/// Maps RGB to the nearest standard/bright ANSI color.
#[must_use]
pub fn quantize_ansi16(color: RgbColor) -> u8 {
    ANSI16
        .iter()
        .enumerate()
        .min_by_key(|(_, candidate)| color_distance(color, **candidate))
        .map_or(0, |(index, _)| u8::try_from(index).unwrap_or(0))
}

fn environment(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

const ANSI16: [RgbColor; 16] = [
    RgbColor::new(0, 0, 0),
    RgbColor::new(128, 0, 0),
    RgbColor::new(0, 128, 0),
    RgbColor::new(128, 128, 0),
    RgbColor::new(0, 0, 128),
    RgbColor::new(128, 0, 128),
    RgbColor::new(0, 128, 128),
    RgbColor::new(192, 192, 192),
    RgbColor::new(128, 128, 128),
    RgbColor::new(255, 0, 0),
    RgbColor::new(0, 255, 0),
    RgbColor::new(255, 255, 0),
    RgbColor::new(0, 0, 255),
    RgbColor::new(255, 0, 255),
    RgbColor::new(0, 255, 255),
    RgbColor::new(255, 255, 255),
];

fn ansi256_color(index: u8) -> RgbColor {
    match index {
        0..=15 => ANSI16[usize::from(index)],
        16..=231 => {
            let cube = index - 16;
            let red = cube / 36;
            let green = (cube % 36) / 6;
            let blue = cube % 6;
            RgbColor::new(cube_level(red), cube_level(green), cube_level(blue))
        }
        232..=255 => {
            let level = 8 + (index - 232) * 10;
            RgbColor::new(level, level, level)
        }
    }
}

const fn cube_level(level: u8) -> u8 {
    if level == 0 { 0 } else { 55 + level * 40 }
}

fn color_distance(left: RgbColor, right: RgbColor) -> u32 {
    let red = i32::from(left.red) - i32::from(right.red);
    let green = i32::from(left.green) - i32::from(right.green);
    let blue = i32::from(left.blue) - i32::from(right.blue);
    u32::try_from(red * red + green * green + blue * blue).unwrap_or(u32::MAX)
}

const fn role_index(role: StyleRole) -> usize {
    match role {
        StyleRole::EditorText => 0,
        StyleRole::EditorBackground => 1,
        StyleRole::Selection => 2,
        StyleRole::CurrentLine => 3,
        StyleRole::LineNumber => 4,
        StyleRole::Gutter => 5,
        StyleRole::StatusBar => 6,
        StyleRole::Panel => 7,
        StyleRole::Error => 8,
        StyleRole::Warning => 9,
        StyleRole::Information => 10,
        StyleRole::Hint => 11,
        StyleRole::GitAdded => 12,
        StyleRole::GitModified => 13,
        StyleRole::GitDeleted => 14,
        StyleRole::SearchMatch => 15,
        StyleRole::SyntaxKeyword | StyleRole::SemanticType => 16,
        StyleRole::SyntaxString => 17,
        StyleRole::SyntaxComment => 18,
    }
}

#[cfg(test)]
mod tests {
    use editor_types::{ColorDepth, UnderlineStyle};

    use super::{
        ResolvedColor, ResolvedUnderline, RgbColor, quantize_ansi16, quantize_ansi256,
        resolve_color, resolve_underline,
    };

    #[test]
    fn color_quantization_is_deterministic_and_hits_exact_palette_colors() {
        let orange = RgbColor::new(255, 95, 0);
        assert_eq!(quantize_ansi256(orange), 202);
        assert_eq!(quantize_ansi256(orange), quantize_ansi256(orange));
        assert_eq!(quantize_ansi16(RgbColor::new(255, 0, 0)), 9);
        assert_eq!(
            resolve_color(orange, ColorDepth::Color256),
            ResolvedColor::Ansi256(202)
        );
    }

    #[test]
    fn underline_fallback_never_loses_diagnostic_emphasis() {
        assert_eq!(
            resolve_underline(UnderlineStyle::Curly),
            ResolvedUnderline::Curly
        );
        assert_eq!(
            resolve_underline(UnderlineStyle::Straight),
            ResolvedUnderline::Straight
        );
        assert_eq!(
            resolve_underline(UnderlineStyle::None),
            ResolvedUnderline::Emphasis
        );
    }
}
