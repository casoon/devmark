//! Zerlegung eines Markdown-Textes in Blöcke für die Blockansicht.
//!
//! Die Blöcke geben den Text lückenlos und unverändert wieder: aneinandergehängt ergeben sie
//! immer genau die Ausgangsdatei. Jeder Block trägt seine Leerzeilen am Ende mit, so bleibt auch
//! die Formatierung zwischen den Blöcken erhalten, wenn nichts geändert wurde.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Frontmatter,
    Heading,
    /// Absatz (alles, was keiner der anderen Arten entspricht).
    Paragraph,
    List,
    Quote,
    Code,
    Table,
    Rule,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// Text des Blocks samt Leerzeilen am Ende.
    pub text: String,
    pub kind: Kind,
}

impl Block {
    /// Inhalt ohne die Zeilenenden am Schluss; das ist, was beim Bearbeiten im Eingabefeld steht.
    pub fn body(&self) -> &str {
        self.text.trim_end_matches(['\n', '\r'])
    }

    /// Zeilenenden und Leerzeilen am Schluss (leer beim letzten Block ohne Zeilenende).
    pub fn separator(&self) -> &str {
        &self.text[self.body().len()..]
    }

    /// Ersetzt den Inhalt, behält das Ende bei; so bleibt der Abstand zum nächsten Block gleich.
    pub fn with_body(&self, body: &str) -> Block {
        let text = format!(
            "{}{}",
            body.trim_end_matches(['\n', '\r']),
            self.separator()
        );
        Block {
            kind: kind_of(&text, self.kind == Kind::Frontmatter),
            text,
        }
    }
}

/// Setzt die Blöcke wieder zu einem Text zusammen.
pub fn join(blocks: &[Block]) -> String {
    blocks.iter().map(|b| b.text.as_str()).collect()
}

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// `` ``` `` oder `~~~` (mindestens drei, höchstens drei Leerzeichen Einzug): Zeichen und Länge.
fn fence(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let c = rest.chars().next().filter(|c| matches!(c, '`' | '~'))?;
    let len = rest.chars().take_while(|x| *x == c).count();
    (len >= 3).then_some((c, len))
}

fn is_heading(line: &str) -> bool {
    let indent = line.len() - line.trim_start_matches(' ').len();
    let rest = &line[indent..];
    let hashes = rest.chars().take_while(|c| *c == '#').count();
    indent <= 3
        && (1..=6).contains(&hashes)
        && rest[hashes..]
            .chars()
            .next()
            .is_none_or(|c| c == ' ' || c == '\t' || c == '\n' || c == '\r')
}

fn is_rule(line: &str) -> bool {
    let t: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    t.len() >= 3 && ['-', '*', '_'].iter().any(|m| t.chars().all(|c| c == *m))
}

fn kind_of(text: &str, frontmatter: bool) -> Kind {
    if frontmatter {
        return Kind::Frontmatter;
    }
    let first = text.lines().next().unwrap_or("");
    let t = first.trim_start();
    if fence(first).is_some() {
        Kind::Code
    } else if is_heading(first) {
        Kind::Heading
    } else if is_rule(first) {
        Kind::Rule
    } else if t.starts_with('>') {
        Kind::Quote
    } else if t.starts_with('|') {
        Kind::Table
    } else if t.starts_with("- ")
        || t.starts_with("* ")
        || t.starts_with("+ ")
        || t.trim_start_matches(|c: char| c.is_ascii_digit())
            .starts_with(". ")
            && t.starts_with(|c: char| c.is_ascii_digit())
    {
        Kind::List
    } else {
        Kind::Paragraph
    }
}

/// Zerlegt `text` in Blöcke: Frontmatter, Überschriften (je eine Zeile), Codeblöcke (auch mit
/// Leerzeilen darin) und sonst Gruppen von Zeilen, die durch Leerzeilen getrennt sind.
pub fn split(text: &str) -> Vec<Block> {
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut blocks: Vec<Block> = Vec::new();
    let mut ix = 0;

    let mut push = |range: std::ops::Range<usize>, frontmatter: bool| {
        let text: String = lines[range].concat();
        blocks.push(Block {
            kind: kind_of(&text, frontmatter),
            text,
        });
    };

    // Frontmatter: `---` in der ersten Zeile bis zum nächsten `---`.
    if lines.first().is_some_and(|l| l.trim_end() == "---")
        && let Some(close) = lines
            .iter()
            .skip(1)
            .position(|l| l.trim_end() == "---" || l.trim_end() == "...")
    {
        let mut end = close + 2;
        while end < lines.len() && is_blank(lines[end]) {
            end += 1;
        }
        push(0..end, true);
        ix = end;
    }

    while ix < lines.len() {
        let start = ix;
        if let Some((c, len)) = fence(lines[ix]) {
            // Codeblock bis zur schließenden Zeile (oder zum Ende des Textes).
            ix += 1;
            while ix < lines.len() {
                let closes = fence(lines[ix]).is_some_and(|(c2, l2)| c2 == c && l2 >= len)
                    && lines[ix].trim().chars().all(|x| x == c);
                ix += 1;
                if closes {
                    break;
                }
            }
        } else if is_heading(lines[ix]) || is_rule(lines[ix]) && !is_blank(lines[ix]) {
            ix += 1;
        } else {
            ix += 1;
            while ix < lines.len()
                && !is_blank(lines[ix])
                && fence(lines[ix]).is_none()
                && !is_heading(lines[ix])
            {
                ix += 1;
            }
        }
        while ix < lines.len() && is_blank(lines[ix]) {
            ix += 1;
        }
        push(start..ix, false);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(text: &str) -> Vec<Kind> {
        split(text).into_iter().map(|b| b.kind).collect()
    }

    #[test]
    fn blocks_by_kind() {
        let text = "---\ntags: [a]\n---\n\n# Titel\nAbsatz eins\nzweite Zeile\n\n- a\n- b\n\n1. eins\n2. zwei\n\n> Zitat\n\n```rust\nfn a() {}\n\nfn b() {}\n```\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n---\n\nSchluss";
        assert_eq!(
            kinds(text),
            [
                Kind::Frontmatter,
                Kind::Heading,
                Kind::Paragraph,
                Kind::List,
                Kind::List,
                Kind::Quote,
                Kind::Code,
                Kind::Table,
                Kind::Rule,
                Kind::Paragraph,
            ]
        );
        let blocks = split(text);
        assert_eq!(blocks[0].text, "---\ntags: [a]\n---\n\n");
        assert_eq!(
            blocks[1].text, "# Titel\n",
            "Überschrift allein, auch ohne Leerzeile"
        );
        assert_eq!(blocks[2].text, "Absatz eins\nzweite Zeile\n\n");
        assert_eq!(blocks[6].body(), "```rust\nfn a() {}\n\nfn b() {}\n```");
        assert_eq!(blocks[9].separator(), "", "letzter Block ohne Zeilenende");
    }

    #[test]
    fn joining_gives_the_original_byte_for_byte() {
        let samples = [
            "",
            "\n",
            "\n\n\nText",
            "Text\r\nmit CRLF\r\n\r\nzweiter\r\n",
            "# Nur Überschrift",
            "# A\n## B\n### C\n",
            "```\nnie geschlossen\n\nimmer noch Code",
            "~~~\ncode\n```\nnoch Code\n~~~\nText\n",
            "---\nohne Ende\n\nText",
            "---\n---\n---\n",
            "Text\n\n\n\n- [ ] a\n- [x] b\n\n\n",
            "    eingerückter Code\n\n  # kein Heading? doch\n#kein Heading\n",
            "ä ö ü ß 日本語\n\n> zitat\n> weiter\n\n",
        ];
        for sample in samples {
            assert_eq!(join(&split(sample)), sample, "{sample:?}");
        }
        // Zufällige Zeilenfolgen aus Bausteinen, die die Zerlegung auf die Probe stellen.
        let parts = [
            "", " ", "# H", "text", "- li", "```", "~~~", "---", "> q", "| t |", "1. n", "\t", "#x",
        ];
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        for _ in 0..2000 {
            let mut text = String::new();
            for _ in 0..(seed % 14) {
                seed = seed
                    .wrapping_mul(6364136223846793005)
                    .wrapping_add(1442695040888963407);
                text.push_str(parts[(seed >> 33) as usize % parts.len()]);
                text.push_str(if seed & 1 == 0 { "\n" } else { "\r\n" });
            }
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            if seed & 2 == 0 {
                text.pop();
            }
            assert_eq!(join(&split(&text)), text, "{text:?}");
        }
    }

    #[test]
    fn editing_a_body_keeps_the_gap_to_the_next_block() {
        let blocks = split("Eins\n\nZwei\n");
        let edited = blocks[0].with_body("Eins, neu\n\n\n");
        assert_eq!(edited.text, "Eins, neu\n\n");
        let mut all = blocks.clone();
        all[0] = edited;
        assert_eq!(join(&all), "Eins, neu\n\nZwei\n");
        // Aus einem Absatz wird eine Überschrift.
        assert_eq!(blocks[0].with_body("# Eins").kind, Kind::Heading);
    }
}
