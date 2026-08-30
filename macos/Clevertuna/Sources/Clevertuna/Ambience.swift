import SwiftUI

/// The room the controls leave, given to the theme.
///
/// A theme we ship knows what it is: magma burns, the cyan ones move like
/// water. A theme you built is yours, and gets a drawn lattice rather than a
/// picture of something it is not.
///
/// Drawn in Canvas on a `TimelineView`, so it is one redrawn layer rather than
/// a pile of animating views — and it stops entirely under Reduce Motion,
/// because ambience is exactly the kind of movement that setting is about.
enum AmbienceKind: Sendable {
    case fire
    case water
    case lattice
}

struct Ambience: View {
    let kind: AmbienceKind
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: reduceMotion)) { ctx in
            Canvas { context, size in
                let t = ctx.date.timeIntervalSinceReferenceDate
                switch kind {
                case .fire: drawFire(context, size, t)
                case .water: drawWater(context, size, t)
                case .lattice: drawLattice(context, size, t)
                }
            }
        }
        .allowsHitTesting(false)
        .clipShape(RoundedRectangle(cornerRadius: DS.R.panel, style: .continuous))
    }

    /// Three plumes welling upward, each on its own clock so they never march.
    private func drawFire(_ context: GraphicsContext, _ size: CGSize, _ t: Double) {
        let plumes: [(x: Double, hue: Color, period: Double, phase: Double)] = [
            (0.22, Color(red: 1.0, green: 0.36, blue: 0.0), 9, 0),
            (0.54, Color(red: 1.0, green: 0.77, blue: 0.0), 13, 4),
            (0.82, Color(red: 0.81, green: 0.08, blue: 0.0), 11, 7),
        ]
        for p in plumes {
            let cycle = ((t + p.phase).truncatingRemainder(dividingBy: p.period)) / p.period
            let rise = sin(cycle * .pi * 2) * 0.5 + 0.5
            let cx = size.width * p.x + sin((t + p.phase) * 0.6) * size.width * 0.03
            let cy = size.height * (1.05 - rise * 0.12)
            let r = size.height * (0.55 + rise * 0.18)
            context.fill(
                Path(ellipseIn: CGRect(x: cx - r, y: cy - r * 0.9, width: r * 2, height: r * 1.8)),
                with: .radialGradient(
                    Gradient(colors: [p.hue.opacity(0.55), p.hue.opacity(0)]),
                    center: CGPoint(x: cx, y: cy), startRadius: 0, endRadius: r))
        }
    }

    /// Swells crossing, and a light chop over them.
    private func drawWater(_ context: GraphicsContext, _ size: CGSize, _ t: Double) {
        let swells: [(y: Double, colour: Color, period: Double)] = [
            (0.92, Color(red: 0.0, green: 0.55, blue: 0.86), 11),
            (0.84, Color(red: 0.0, green: 0.84, blue: 0.75), 17),
        ]
        for s in swells {
            let drift = sin(t * 2 * .pi / s.period)
            let cx = size.width * (0.5 + drift * 0.08)
            let cy = size.height * s.y
            let r = size.width * 0.55
            context.fill(
                Path(ellipseIn: CGRect(x: cx - r, y: cy - r * 0.45, width: r * 2, height: r * 0.9)),
                with: .radialGradient(
                    Gradient(colors: [s.colour.opacity(0.45), s.colour.opacity(0)]),
                    center: CGPoint(x: cx, y: cy), startRadius: 0, endRadius: r))
        }
        // The chop: thin lines sliding along, so the surface reads as moving.
        var chop = Path()
        let step = 26.0
        let slide = (t * 12).truncatingRemainder(dividingBy: step)
        var x = -step + slide
        while x < size.width + step {
            chop.move(to: CGPoint(x: x, y: size.height))
            chop.addLine(to: CGPoint(x: x + size.height * 0.32, y: size.height * 0.55))
            x += step
        }
        context.stroke(chop, with: .color(.white.opacity(0.05)), lineWidth: 1)
    }

    /// A drawn lattice, turning slowly: geometry rather than an element.
    private func drawLattice(_ context: GraphicsContext, _ size: CGSize, _ t: Double) {
        let spin = t.truncatingRemainder(dividingBy: 120) / 120 * 2 * .pi
        let centre = CGPoint(x: size.width * 0.5, y: size.height * 0.55)
        for layer in 0..<2 {
            let angleStep = Double.pi / 3
            let offset = spin * (layer == 0 ? 1 : -0.6)
            let gap = 44.0 + Double(layer) * 18
            var path = Path()
            for k in 0..<3 {
                let a = Double(k) * angleStep + offset
                let dx = cos(a), dy = sin(a)
                let span = max(size.width, size.height) * 1.4
                var i = -span
                while i < span {
                    let px = centre.x - dy * i, py = centre.y + dx * i
                    path.move(to: CGPoint(x: px - dx * span, y: py - dy * span))
                    path.addLine(to: CGPoint(x: px + dx * span, y: py + dy * span))
                    i += gap
                }
            }
            context.stroke(path, with: .color(.white.opacity(layer == 0 ? 0.10 : 0.06)), lineWidth: 1)
        }
    }
}

extension BuilderModel {
    /// What a theme is, for the purpose of filling a room with it.
    ///
    /// Ours know; yours gets the lattice, because guessing that a theme called
    /// "Lemon Pop" is about water would be worse than not guessing.
    func ambience(for theme: ThemeSummary?) -> AmbienceKind {
        guard let theme, theme.group != "Yours" else { return .lattice }
        switch theme.id {
        case "magma", "amber-desk", "lantern", "pulse": return .fire
        case "deep-current", "tide", "aurora", "nightshift", "sleep": return .water
        default: return .lattice
        }
    }
}
