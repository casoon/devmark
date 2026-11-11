//! Schriften: JetBrains Mono ist eingebaut und Standard für Code und Typst; für Markdown gibt es
//! eine Lesefont. Beide lassen sich im Menü „Schrift“ wählen und werden im Datenordner gemerkt.

use std::borrow::Cow;
use std::collections::HashSet;
use std::path::Path;

use gpui_kit::App;

/// Familienname der eingebauten Schrift.
pub const BUNDLED: &str = "JetBrains Mono";

/// Die eingebauten Schnitte (SIL Open Font License, siehe `assets/fonts/OFL-JetBrainsMono.txt`).
pub fn register(cx: &App) {
    let fonts: Vec<Cow<'static, [u8]>> = vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
        Cow::Borrowed(include_bytes!(
            "../assets/fonts/JetBrainsMono-BoldItalic.ttf"
        )),
    ];
    if let Err(err) = cx.text_system().add_fonts(fonts) {
        eprintln!("Eingebaute Schrift konnte nicht geladen werden: {err}");
    }
}

/// Gewählte Schriften.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fonts {
    /// Code, Typst und sonstige Textdateien.
    pub code: String,
    /// Markdown (Notizen und Dokumente).
    pub text: String,
}

impl Default for Fonts {
    fn default() -> Self {
        Self {
            code: BUNDLED.into(),
            text: default_text().into(),
        }
    }
}

/// Lesefreundliche Schrift für Fließtext; Systemschrift, wo keine Serifenschrift sicher da ist.
fn default_text() -> &'static str {
    if cfg!(target_os = "macos") {
        "Charter"
    } else if cfg!(target_os = "windows") {
        "Georgia"
    } else {
        "Noto Serif"
    }
}

impl Fonts {
    /// Aus `fonts` im Datenordner (`code=…`, `text=…`); fehlende oder leere Werte sind Standard.
    pub fn load(data_dir: &Path) -> Self {
        let mut fonts = Self::default();
        for line in std::fs::read_to_string(data_dir.join("fonts"))
            .unwrap_or_default()
            .lines()
        {
            match line.split_once('=').map(|(k, v)| (k.trim(), v.trim())) {
                Some(("code", v)) if !v.is_empty() => fonts.code = v.into(),
                Some(("text", v)) if !v.is_empty() => fonts.text = v.into(),
                _ => {}
            }
        }
        fonts
    }

    pub fn save(&self, data_dir: &Path) {
        std::fs::create_dir_all(data_dir).ok();
        std::fs::write(
            data_dir.join("fonts"),
            format!("code={}\ntext={}\n", self.code, self.text),
        )
        .ok();
    }
}

/// Zur Auswahl: die eingebaute Schrift und installierte Festbreitenschriften.
pub fn code_choices(cx: &App) -> Vec<String> {
    let installed: HashSet<String> = cx.text_system().all_font_names().into_iter().collect();
    let mut names: Vec<String> = installed
        .iter()
        .filter(|name| is_monospace_name(name) && name.as_str() != BUNDLED)
        .cloned()
        .collect();
    names.sort_by_key(|n| n.to_lowercase());
    names.insert(0, BUNDLED.into());
    names
}

/// Zur Auswahl: gängige Lesefonts, soweit installiert, und die Systemschrift.
pub fn text_choices(cx: &App) -> Vec<String> {
    let installed: HashSet<String> = cx.text_system().all_font_names().into_iter().collect();
    [
        "Charter",
        "Georgia",
        "Iowan Old Style",
        "Palatino",
        "New York",
        "Noto Serif",
        "Source Serif 4",
        "Helvetica Neue",
        "Inter",
        "Noto Sans",
    ]
    .into_iter()
    .filter(|name| installed.contains(*name))
    .map(String::from)
    .chain([BUNDLED.to_string()])
    .collect()
}

/// Festbreitenschriften erkennt man nur am Namen; Nerd-Font-Abwandlungen bleiben draußen,
/// sonst füllen sie die Liste mit Dutzenden Einträgen.
fn is_monospace_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    !lower.contains("nerd")
        && !lower.contains("propo")
        && [
            "mono", "code", "courier", "menlo", "consolas", "hack", "terminal",
        ]
        .iter()
        .any(|m| lower.contains(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("devmark-fonts-{}", std::process::id()));
        assert_eq!(Fonts::load(&dir), Fonts::default());
        let fonts = Fonts {
            code: "Menlo".into(),
            text: "Georgia".into(),
        };
        fonts.save(&dir);
        assert_eq!(Fonts::load(&dir), fonts);
        std::fs::write(dir.join("fonts"), "code=\ntext=Palatino\nquatsch\n").unwrap();
        let loaded = Fonts::load(&dir);
        assert_eq!(loaded.code, BUNDLED);
        assert_eq!(loaded.text, "Palatino");
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn monospace_names() {
        assert!(is_monospace_name("SF Mono"));
        assert!(is_monospace_name("Menlo"));
        assert!(!is_monospace_name("JetBrainsMono Nerd Font"));
        assert!(!is_monospace_name("Charter"));
    }
}
