//! LaTeX-Mathe → Typst-Mathe über `mitex`.
//!
//! `mitex` erzeugt Typst-Mathe, die teils Hilfsfunktionen des Typst-Pakets `@preview/mitex`
//! erwartet. Damit das Ergebnis ohne Paket (offline) kompiliert, liefern wir die benötigten
//! Definitionen selbst mit – nur die, die im Dokument tatsächlich vorkommen.
//! Nachgebildet nach `mitex/packages/mitex/specs/latex/standard.typ` (Apache-2.0).

use std::collections::BTreeSet;

/// Wandelt LaTeX-Mathe in Typst-Mathe um. `Err` enthält die Fehlermeldung von mitex.
pub fn latex_to_typst(latex: &str) -> Result<String, String> {
    // `1{,}5` ist in LaTeX ein Dezimalkomma ohne Abstand; mitex macht daraus ein normales Komma.
    let latex = latex.replace("{,}", "\\mathord{,}");
    let converted = mitex::convert_math(&latex, None)?;
    Ok(tidy(&converted))
}

/// mitex hängt nach jedem Token Leerzeichen an; das ist korrekt, aber schwer lesbar.
/// Entfernt nur Leerzeichen, die in Typst-Mathe bedeutungslos sind (`\ ` bleibt erhalten).
fn tidy(math: &str) -> String {
    let chars: Vec<char> = math.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != ' ' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        while i < chars.len() && chars[i] == ' ' {
            i += 1;
        }
        let next = chars.get(i).copied();
        let prev = out.last().copied();
        let escaped = |ix: usize| ix > 0 && out.get(ix - 1) == Some(&'\\');
        let keep = match (prev, next) {
            (None, _) | (_, None) => false,
            (Some('\\'), _) => true,
            (Some('('), _) if !escaped(out.len() - 1) => false,
            (_, Some(')' | ',' | ';' | '^' | '_')) => false,
            _ => true,
        };
        if keep {
            out.push(' ');
        }
    }
    out.into_iter().collect()
}

/// Namen von Hilfsfunktionen, die in `math` vorkommen.
pub fn used_helpers(math: &str, used: &mut BTreeSet<&'static str>) {
    for ident in math
        .split(|c: char| !(c.is_ascii_alphanumeric()))
        .filter(|s| !s.is_empty())
    {
        if let Some((name, _)) = HELPERS.iter().find(|(name, _)| *name == ident) {
            used.insert(name);
        }
    }
}

/// Typst-Definitionen der benötigten Hilfsfunktionen, in stabiler Reihenfolge.
pub fn helper_definitions(used: &BTreeSet<&'static str>) -> String {
    let mut out = String::new();
    for (name, definition) in HELPERS {
        if used.contains(name) {
            out.push_str(definition);
            out.push('\n');
        }
    }
    out
}

const HELPERS: &[(&str, &str)] = &[
    ("mitexsqrt", "#let mitexsqrt(..args) = if args.pos().len() == 1 { math.sqrt(args.pos().at(0)) } else { math.root(args.pos().at(0).children.filter(it => it != [\\[] and it != [\\]]).sum(), args.pos().at(1)) }"),
    ("textmath", "#let textmath(it) = it"),
    ("textmd", "#let textmd(it) = it"),
    ("textnormal", "#let textnormal(it) = it"),
    ("textbf", "#let textbf(it) = math.bold(it)"),
    ("textit", "#let textit(it) = math.italic(it)"),
    ("textrm", "#let textrm(it) = math.upright(it)"),
    ("textup", "#let textup(it) = math.upright(it)"),
    ("textsf", "#let textsf(it) = math.sans(it)"),
    ("texttt", "#let texttt(it) = math.mono(it)"),
    ("mitexmathbf", "#let mitexmathbf(it) = math.bold(math.upright(it))"),
    ("mitexbold", "#let mitexbold(..args) = math.bold(math.upright(args.pos().sum()))"),
    ("mitexupright", "#let mitexupright(..args) = math.upright(args.pos().sum())"),
    ("mitexitalic", "#let mitexitalic(..args) = math.italic(args.pos().sum())"),
    ("mitexsans", "#let mitexsans(..args) = math.sans(args.pos().sum())"),
    ("mitexfrak", "#let mitexfrak(..args) = math.frak(args.pos().sum())"),
    ("mitexmono", "#let mitexmono(..args) = math.mono(args.pos().sum())"),
    ("mitexcal", "#let mitexcal(..args) = math.cal(args.pos().sum())"),
    ("mitexdisplay", "#let mitexdisplay(..args) = math.display(args.pos().sum())"),
    ("mitexinline", "#let mitexinline(..args) = math.inline(args.pos().sum())"),
    ("mitexscript", "#let mitexscript(..args) = math.script(args.pos().sum())"),
    ("mitexsscript", "#let mitexsscript(..args) = math.sscript(args.pos().sum())"),
    ("mitexoverbrace", "#let mitexoverbrace(it) = math.limits(math.overbrace(it))"),
    ("mitexunderbrace", "#let mitexunderbrace(it) = math.limits(math.underbrace(it))"),
    ("mitexoverbracket", "#let mitexoverbracket(it) = math.limits(math.overbracket(it))"),
    ("mitexunderbracket", "#let mitexunderbracket(it) = math.limits(math.underbracket(it))"),
    ("mitexnot", "#let mitexnot(it) = math.cancel(angle: 20deg, it)"),
    ("mitexset", "#let mitexset(it) = math.lr(${$ + it + $}$)"),
    ("mitexlabel", "#let mitexlabel(..args) = none"),
    ("mitexcaption", "#let mitexcaption(..args) = none"),
    ("mitexcomment", "#let mitexcomment(..args) = none"),
    ("mitexbibliography", "#let mitexbibliography(..args) = none"),
    ("mitexref", "#let mitexref(it) = it"),
    ("mitexcite", "#let mitexcite(it) = it"),
    ("mitexcolor", "#let mitexcolor(..args) = { let names = (red: red, blue: blue, green: green, black: black, white: white, gray: gray, grey: gray, orange: orange, purple: purple, cyan: aqua, magenta: fuchsia, yellow: yellow, brown: rgb(\"#8b4513\")); let pos = args.pos(); let spec = pos.at(0); let key = if spec.has(\"children\") { spec.children.filter(it => it.has(\"text\")).map(it => it.text).join() } else if spec.has(\"text\") { spec.text } else { \"\" }; let body = pos.slice(1).sum(default: []); let fill = names.at(lower(key), default: none); if fill == none { body } else { text(fill: fill, body) } }"),
    ("mathord", "#let mathord(it) = math.class(\"normal\", it)"),
    ("mathbin", "#let mathbin(it) = math.class(\"binary\", it)"),
    ("mathrel", "#let mathrel(it) = math.class(\"relation\", it)"),
    ("mathopen", "#let mathopen(it) = math.class(\"opening\", it)"),
    ("mathclose", "#let mathclose(it) = math.class(\"closing\", it)"),
    ("mathpunct", "#let mathpunct(it) = math.class(\"punctuation\", it)"),
    ("matrix", "#let matrix = math.mat.with(delim: none)"),
    ("pmatrix", "#let pmatrix = math.mat.with(delim: \"(\")"),
    ("bmatrix", "#let bmatrix = math.mat.with(delim: \"[\")"),
    ("Bmatrix", "#let Bmatrix = math.mat.with(delim: \"{\")"),
    ("vmatrix", "#let vmatrix = math.mat.with(delim: \"|\")"),
    ("Vmatrix", "#let Vmatrix = math.mat.with(delim: \"||\")"),
    ("smallmatrix", "#let smallmatrix(..args) = math.inline(math.mat(delim: none, ..args))"),
    ("mitexarray", "#let mitexarray(arg0: (\"l\",), ..args) = { if args.pos().len() == 0 { return }; let spec = if type(arg0) == content { if arg0.has(\"children\") { arg0.children.filter(it => it.has(\"text\")).map(it => it.text).filter(it => it in (\"l\", \"c\", \"r\")) } else { (arg0.text,) } } else { arg0 }; let rows = args.pos().map(row => if type(row) == array { row } else { (row,) }); let m = calc.max(..rows.map(row => row.len())); let rows = rows.map(row => row + (m - row.len()) * (none,)); let spec = if spec.len() == 0 { (\"l\",) } else { spec }; set align((l: left, c: center, r: right).at(spec.at(0), default: left)); pad(y: 0.2em, grid(columns: m, column-gutter: 0.5em, row-gutter: 0.5em, ..rows.flatten().map(it => $it$))) }"),
    ("aligned", "#let aligned(..args) = if args.pos().len() > 0 { pad(y: 0.2em, block(math.op(math.display(args.pos().at(0))))) }"),
    ("alignedat", "#let alignedat(arg0: none, it) = pad(y: 0.2em, block(math.op(it)))"),
    ("rcases", "#let rcases = math.cases.with(reverse: true)"),
    ("operatorname", "#let operatorname(it) = math.op(math.upright(it))"),
    ("operatornamewithlimits", "#let operatornamewithlimits(it) = math.op(limits: true, math.upright(it))"),
    ("boxed", "#let boxed(it) = box(stroke: 0.5pt, inset: 6pt, $it$)"),
    ("fbox", "#let fbox(it) = box(stroke: 0.5pt, $it$)"),
    ("hbox", "#let hbox(it) = it"),
    ("mbox", "#let mbox(it) = it"),
    ("dfrac", "#let dfrac(num, den) = math.display(math.frac(num, den))"),
    ("cfrac", "#let cfrac(num, den) = math.display(math.frac(num, den))"),
    ("tfrac", "#let tfrac(num, den) = math.inline(math.frac(num, den))"),
    ("dbinom", "#let dbinom(n, k) = math.display(math.binom(n, k))"),
    ("tbinom", "#let tbinom(n, k) = math.inline(math.binom(n, k))"),
    ("stackrel", "#let stackrel(sup, base) = math.attach(math.limits(base), t: sup)"),
    ("overset", "#let overset(sup, base) = math.attach(math.limits(base), t: sup)"),
    ("underset", "#let underset(sub, base) = math.attach(math.limits(base), b: sub)"),
    ("substack", "#let substack(it) = it"),
    ("xcancel", "#let xcancel(it) = math.cancel(cross: true, it)"),
    ("bcancel", "#let bcancel(it) = math.cancel(inverted: true, it)"),
    ("sout", "#let sout(it) = math.cancel(angle: 90deg, it)"),
    ("pmod", "#let pmod(it) = $quad (mod thick #it)$"),
    ("pod", "#let pod(it) = $quad (#it)$"),
    ("xrightarrow", "#let xrightarrow(it) = math.attach(math.limits(math.stretch(sym.arrow.r)), t: it)"),
    ("xleftarrow", "#let xleftarrow(it) = math.attach(math.limits(math.stretch(sym.arrow.l)), t: it)"),
    ("mathring", "#let mathring(it) = math.circle(it)"),
    ("middle", "#let middle(it) = math.mid(it)"),
    ("phantom", "#let phantom(it) = hide(it)"),
    ("hphantom", "#let hphantom(it) = box(height: 0pt, hide(it))"),
    ("vphantom", "#let vphantom(it) = box(width: 0pt, hide(it))"),
    ("smash", "#let smash(it) = box(height: 0pt, align(bottom, $it$))"),
    ("negthinspace", "#let negthinspace = h(-(3 / 18) * 1em)"),
    ("negmedspace", "#let negmedspace = h(-(4 / 18) * 1em)"),
    ("negthickspace", "#let negthickspace = h(-(5 / 18) * 1em)"),
    ("enspace", "#let enspace = h(0.5em)"),
    ("lvert", "#let lvert = math.class(\"opening\", \"|\")"),
    ("rvert", "#let rvert = math.class(\"closing\", \"|\")"),
    ("lVert", "#let lVert = math.class(\"opening\", \"‖\")"),
    ("rVert", "#let rVert = math.class(\"closing\", \"‖\")"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_common_latex() {
        assert_eq!(latex_to_typst(r"\frac{a}{b}").unwrap(), "frac(a,b)");
        assert_eq!(latex_to_typst(r"x \in \mathbb{R}").unwrap(), "x in bb(R)");
        assert!(latex_to_typst(r"\mathbbm{1}").is_err());
        assert_eq!(latex_to_typst(r"1{,}5").unwrap(), "1 mathord(\\,)5");
    }

    #[test]
    fn detects_helpers() {
        let mut used = BTreeSet::new();
        used_helpers(&latex_to_typst(r"\sqrt{x} + \text{a}").unwrap(), &mut used);
        assert!(used.contains("mitexsqrt") && used.contains("textmath"));
        assert!(helper_definitions(&used).contains("#let mitexsqrt"));
    }
}
