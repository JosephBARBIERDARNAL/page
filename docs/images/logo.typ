// Generate logo, 4 scenarios:
// light/dark
// with or without label
// Always render all 4 when making changes

#let navy = rgb("#071A38")
#let teal = rgb("#10B981")
#let mint = rgb("#A7F3D0")
#let white = rgb("#FFFFFF")
#let dark = sys.inputs.at("surface", default: "dark") == "dark"
#let labeled = sys.inputs.at("label", default: "true") == "true"
#let foreground = if dark { white } else { navy }
#let fold = if dark { navy } else { white }

#set page(
  width: if labeled { 40mm } else { 14mm },
  height: 14mm,
  margin: 0mm,
  fill: none,
)
#set text(font: "Inter Display", fill: foreground)

#let logo-symbol(size: 14mm) = {
  let ratio = size / 18mm
  let paper-size = 12.2mm * ratio
  let paper-radius = 2.3mm * ratio
  let fold-size = 4.9mm * ratio
  let fold-radius = 0.5mm * ratio
  let fold-crease = 0mm * ratio
  let fold-stroke = 1.2pt * ratio
  let stroke-half = fold-stroke / 2
  let drawing-width = 14.3mm * ratio + stroke-half
  let drawing-height = 14.6mm * ratio + stroke-half

  box(
    width: drawing-width,
    height: drawing-height,
    {
      place(
        dx: 0mm,
        dy: 2.4mm * ratio + stroke-half,
        rect(
          width: 12.2mm * ratio,
          height: 12.2mm * ratio,
          radius: 2.3mm * ratio,
          fill: teal,
        ),
      )
      place(
        dx: 2.2mm * ratio,
        dy: stroke-half,
        curve(
          fill: foreground,
          stroke: none,
          curve.move((paper-radius, 0mm)),
          curve.line((paper-size - fold-size - fold-radius, 0mm)),
          curve.quad(
            (paper-size - fold-size, 0mm),
            (paper-size - fold-size + fold-radius, fold-radius),
          ),
          curve.line((paper-size - fold-radius, fold-size - fold-radius)),
          curve.quad(
            (paper-size, fold-size),
            (paper-size, fold-size + fold-radius),
          ),
          curve.line((paper-size, paper-size - paper-radius)),
          curve.quad(
            (paper-size, paper-size),
            (paper-size - paper-radius, paper-size),
          ),
          curve.line((paper-radius, paper-size)),
          curve.quad((0mm, paper-size), (0mm, paper-size - paper-radius)),
          curve.line((0mm, paper-radius)),
          curve.quad((0mm, 0mm), (paper-radius, 0mm)),
          curve.close(mode: "straight"),
        ),
      )
      place(
        dx: 9.2mm * ratio,
        dy: stroke-half,
        curve(
          stroke: (
            paint: fold,
            thickness: fold-stroke,
            cap: "round",
            join: "round",
          ),
          fill: none,
          curve.move((0mm, fold-crease + 0.15mm)),
          curve.line((-0mm, fold-size - 2 * fold-crease)),
          curve.quad(
            (0mm, fold-size - fold-crease),
            (fold-crease, fold-size - fold-crease),
          ),
          curve.line((fold-size - fold-crease, fold-size - fold-crease)),
        ),
      )
    },
  )
}

#if labeled {
  align(
    center + horizon,
    grid(
      columns: (auto, auto),
      column-gutter: 4mm,
      logo-symbol(), text(size: 26pt, weight: "bold", tracking: -1pt, [page]),
    ),
  )
} else {
  align(center + horizon, logo-symbol())
}
