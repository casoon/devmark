---
title: "DevMark: Markdown nach Typst"
subtitle: Ein Testdokument mit allem, was Markdown kann
author:
  - Ada Lovelace
  - Grace Hopper
date: 2026-10-14
tags: [markdown, typst, test]
toc: true
numbering: true
---

# Einleitung

Dies ist ein *Testdokument* mit **fettem Text**, ***beidem***, ~~Durchgestrichenem~~ und `Inline-Code`.
Sonderzeichen müssen überleben: #hashtag, @erwähnung, 5 $ und 10 €, a_b_c, 2*3*4, [eckig], <spitz>,
Backslash \\ und Tilde ~. Ein „Zitat" mit "geraden" und 'einfachen' Anführungszeichen.
Gedankenstrich -- und Geviertstrich --- sowie ... Auslassung. Pfad: C:\Users\x und /usr/bin.
Kommentarartiges: a // b und /* c */. URL im Text: https://typst.app/docs und Mail <ada@example.com>.

Eine harte  
Zeilenumbruch-Zeile und ein [Link zu Typst](https://typst.app "Titel") sowie ein Autolink <https://example.com>.
Interner Verweis auf [Tabellen](#tabellen) und auf [Mathe](#mathematik).

1984\. war ein Jahr. Und **fett**.danach.

## Listen

- Erster Punkt
- Zweiter Punkt mit `code`
  - Unterpunkt A
  - Unterpunkt B
    1. Tief nummeriert
    2. Noch tiefer
- Dritter Punkt

3. Start bei drei
4. Vier
5. Fünf

- [x] Erledigt
- [ ] Offen
- [ ] Auch offen mit *Betonung*

Lockere Liste:

- Erster Absatz im Punkt.

  Zweiter Absatz im selben Punkt.

  ```rust
  fn im_listenpunkt() {
      println!("eingerückt");
  }
  ```

- Nächster Punkt.

## Code

```rust title="main.rs" {2}
fn main() {
    let x = "#nicht-typst $kein-mathe$ // kein Kommentar";
    println!("{x}");
}
```

```
Ohne Sprache, mit ``` drei Backticks im Text.
```

    eingerückter Codeblock

## Zitate und Hinweise

> Ein Zitat über
> mehrere Zeilen mit **Formatierung**.
>
> > Verschachteltes Zitat.

> [!NOTE]
> Ein Hinweis im GitHub-Stil.

> [!TIP]
> Ein Tipp mit `Code`.

> [!IMPORTANT]
> Wichtig!

> [!WARNING]
> Eine Warnung.

> [!CAUTION]
> Vorsicht, gefährlich.

## Tabellen

| Name | Wert | Einheit |
|:-----|-----:|:-------:|
| Länge | 42 | m |
| Breite mit *Betonung* | 3,5 | m |
| Code | `x_y` | – |

| Ohne | Ausrichtung |
|---|---|
| a | b |

## Mathematik

Inline $E = mc^2$ und $\alpha + \beta = \gamma$ und $\sqrt{x^2 + y^2}$ sowie $\mathbb{R}^n$.

$$
\int_0^\infty e^{-x^2}\,dx = \frac{\sqrt{\pi}}{2}
$$

$$
\begin{pmatrix} a & b \\ c & d \end{pmatrix}
\begin{bmatrix} 1 \\ 2 \end{bmatrix}
$$

$$
f(x) = \begin{cases} x^2 & \text{wenn } x > 0 \\ 0 & \text{sonst} \end{cases}
$$

$$
\begin{aligned} a &= b + c \\ d &= e \cdot f \end{aligned}
$$

Unbekannt: $\mathbbm{1}$ bleibt als LaTeX.

## Fußnoten

Hier ist eine Fußnote[^eins] und noch eine[^zwei]. Und die erste nochmal[^eins].

[^eins]: Erste Fußnote mit *Betonung*.
[^zwei]: Zweite Fußnote mit [Link](https://example.com).

## Bilder

![Ein Farbverlauf als Testbild](img/verlauf.png "Abbildung: Farbverlauf")

Inline-Bild ![icon](img/verlauf.png) im Text.

![Fehlt](img/gibt-es-nicht.png)

![Remote](https://example.com/bild.png)

## HTML

H<sub>2</sub>O, E = mc<sup>2</sup>, <kbd>Cmd</kbd>+<kbd>S</kbd>, <mark>markiert</mark>, <u>unterstrichen</u>.<br>
Neue Zeile nach br.

<!-- Ein HTML-Kommentar -->

<details>
<summary>Mehr anzeigen</summary>

Versteckter Inhalt mit **Formatierung**.

</details>

<div align="center">
<img src="img/verlauf.png" alt="zentriert" width="200">
</div>

## Definitionen

Typst
: Ein modernes Satzsystem.

Markdown
: Eine leichtgewichtige Auszeichnungssprache.

---

### Überschrift mit Sonderzeichen: Größe & Maß (v2.0) {#eigene-id}

Verweis auf [eigene ID](#eigene-id). Ende.
