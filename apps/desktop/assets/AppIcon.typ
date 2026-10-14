// App-Icon für DevMark (Quelle). Erzeugen: scripts/make-icon.sh
#set page(width: 1024pt, height: 1024pt, margin: 0pt, fill: none)
#place(center + horizon, rect(
  width: 824pt, height: 824pt, radius: 185pt,
  fill: gradient.linear(rgb("#1b2f5e"), rgb("#2a7f95"), angle: 60deg),
))
// Papierbogen
#place(center + horizon, dy: 24pt, rect(
  width: 470pt, height: 560pt, radius: 28pt,
  fill: white.transparentize(4%),
))
// Zeilen
#for (i, w) in (300pt, 340pt, 250pt).enumerate() {
  place(center + horizon, dx: (w - 340pt) / 2, dy: 140pt + i * 52pt,
    rect(width: w, height: 18pt, radius: 9pt, fill: rgb("#1b2f5e").transparentize(78%)))
}
#place(center + horizon, dy: -70pt,
  text(font: "Libertinus Serif", weight: "bold", size: 330pt, fill: rgb("#1b2f5e"))[M]
)
#place(center + horizon, dx: 165pt, dy: 30pt,
  text(font: "Libertinus Serif", weight: "bold", size: 150pt, fill: rgb("#2a7f95"))[↓]
)
