import SwiftUI

/// The keyboard, drawn as the hardware has it.
///
/// Every key sits where it sits and carries what is printed on it, and each
/// takes the colour the effect puts at its own position — so the wave crosses
/// the deck the way it crosses the desk. The touch surface and the two sliders
/// are drawn over the keys they cover, because that is where they are.
///
/// **The deck is drawn in three layers, and only two of them move.** Eighty-four
/// keycaps as eighty-four SwiftUI views meant eighty-four view bodies and
/// eighty-four text layouts thirty times a second — a third of a core to animate
/// colour. The caps are now rectangles filled in a `Canvas`; the legends are a
/// layer that never sees `phase`, because what is printed on a key does not
/// change when the light does. The clock lives here rather than in the parent so
/// that only the layers that move are rebuilt.
struct DeckView: View {
    let look: LookModel
    var selected: String
    /// Whether anything on this deck actually moves. A still look pauses the
    /// clock, and a paused clock costs nothing.
    var animated: Bool
    /// Seconds since the deck started. Each zone turns this into its own
    /// phase at its own speed — the deck does not have a phase, the zones do.
    var clock: (Date) -> Double
    /// Black keys or white ones. The device cannot say which this board is.
    var finish: KeyboardFinish = .dark
    var onSelect: (String) -> Void
    /// Dragging one zone onto another copies the light between them.
    var onCopy: (String, String) -> Void = { _, _ in }

    @State private var dragFrom: String?
    @State private var dragOver: String?
    @State private var hoverZone: String?
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    private var layout: KeyLayout { .shared }

    var body: some View {
        GeometryReader { geo in
            let deck = deckRect(in: geo.size)
            // Resolved once per look, not once per key per frame.
            let lit = Dictionary(uniqueKeysWithValues: look.zones.map {
                ($0.key, Lit($0.value, pivot: look.ranges.speedPivot))
            })
            let caps = capPlacements(in: deck, lit: lit)

            ZStack(alignment: .topLeading) {
                deckBody(in: deck)

                // Moves: the light escaping around every cap, then the caps
                // themselves in the keyboard's own plastic.
                TimelineView(.animation(minimumInterval: 1.0 / 30.0,
                                        paused: !animated || reduceMotion)) { ctx in
                    let now = clock(ctx.date)
                    let hits = typingHits(caps, at: now)
                    Canvas { g, _ in
                        bleed(caps, in: g, lit: lit, at: now, hits: hits, typed: typedColour)
                        for c in caps { cap(c, in: g) }
                    }
                    .frame(width: geo.size.width, height: geo.size.height)
                }

                // Still: what is printed on the keys, with no light behind it.
                legends(caps, ink: unlitInk, over: geo.size)

                // Moves: the same legends with the light behind them. The text
                // is laid out once and used as a mask; only the colour under it
                // is redrawn, so a glowing legend costs a fill, not a layout.
                TimelineView(.animation(minimumInterval: 1.0 / 30.0,
                                        paused: !animated || reduceMotion)) { ctx in
                    let now = clock(ctx.date)
                    let hits = typingHits(caps, at: now)
                    Canvas { g, _ in
                        for (i, c) in caps.enumerated() {
                            var colour = (lit[c.zone] ?? Lit.dark).colour(c.t, at: now)
                            // A struck key's legend takes the reactive colour,
                            // which is the whole of what a dark theme shows.
                            if let typed = typedColour, let strength = hits[i] {
                                colour = typed.opacity(strength)
                            }
                            g.fill(Path(c.rect.insetBy(dx: -1, dy: -1)), with: .color(colour))
                        }
                    }
                    .frame(width: geo.size.width, height: geo.size.height)
                }
                .mask(legends(caps, ink: .white, over: geo.size))

                // Still: the outlines that say which zone you are pointing at.
                regions(in: deck, over: geo.size)
            }
            .frame(width: geo.size.width, height: geo.size.height, alignment: .center)
            .contentShape(.rect)
            .onContinuousHover { phase in
                switch phase {
                case .active(let p): hoverZone = zone(at: p, deck: deck)
                case .ended: hoverZone = nil
                }
            }
            .gesture(dragGesture(deck: deck))
            .animation(reduceMotion ? nil : DS.M.select, value: selected)
            .animation(DS.M.hover, value: hoverZone)
            .animation(DS.M.hover, value: dragOver)
        }
        .aspectRatio(layout.aspect, contentMode: .fit)
    }

    // MARK: - Geometry

    /// The deck keeps the keyboard's own proportions and is centred in whatever
    /// room it is given.
    private func deckRect(in size: CGSize) -> CGRect {
        var w = size.width
        var h = w / layout.aspect
        if h > size.height {
            h = size.height
            w = h * layout.aspect
        }
        return CGRect(x: (size.width - w) / 2, y: (size.height - h) / 2, width: w, height: h)
    }

    private func frame(x: Double, y: Double, w: Double, h: Double, in deck: CGRect) -> CGRect {
        CGRect(x: deck.minX + x / layout.unit.width * deck.width,
               y: deck.minY + y / layout.unit.height * deck.height,
               width: w / layout.unit.width * deck.width,
               height: h / layout.unit.height * deck.height)
    }

    private func zoneRect(_ z: KeyLayout.Zone, in deck: CGRect) -> CGRect? {
        guard let x = z.x, let y = z.y, let w = z.w, let h = z.h else { return nil }
        return frame(x: x, y: y, w: w, h: h, in: deck)
    }

    /// The touch regions lie on top of the keys, so they take a click first.
    private func zone(at p: CGPoint, deck: CGRect) -> String? {
        for z in layout.zones where !z.isField {
            if let r = zoneRect(z, in: deck), r.insetBy(dx: -3, dy: -3).contains(p) { return z.id }
        }
        return deck.insetBy(dx: -6, dy: -6).contains(p) ? "keyboard" : nil
    }

    // MARK: - The keys

    /// The deck body the caps sit in, outlined when the keys are what you are
    /// editing. It does not move, so it is a view and keeps its animation.
    private func deckBody(in deck: CGRect) -> some View {
        RoundedRectangle(cornerRadius: 12, style: .continuous)
            .fill(finish.deck)
            .overlay {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .strokeBorder(selected == "keyboard" ? Color.accentColor
                                  : hoverZone == "keyboard" ? .white.opacity(0.35) : .clear,
                                  lineWidth: selected == "keyboard" ? 2.5 : 1.5)
            }
            .frame(width: deck.width + 14, height: deck.height + 14)
            .offset(x: deck.minX - 7, y: deck.minY - 7)
    }

    /// One key, ready to draw: everything about it that the light does not
    /// change, worked out once.
    private struct Cap: Identifiable {
        let id: Int
        let rect: CGRect
        let radius: Double
        /// Which zone lights this key.
        let zone: String
        /// Where along that zone's gradient this key's centre sits.
        let t: Double
        let label: String
        let sub: String?
        let point: Double
        let homing: Bool
        let showsLegend: Bool
    }

    /// Every zone that is not the whole field, in the layout's own units.
    ///
    /// **None of these is a thing drawn beside the keys.** The touch surface is
    /// a block of the key field, and each slider's rectangle is exactly the
    /// F2–F6 and F7–F11 keycaps' own — so the keys inside a zone *are* the LEDs
    /// it lights, and they take its colours, its effect and its speed. Painting
    /// them over with a bar hid five keycaps and their legends apiece and
    /// animated them as a strip rather than as the keys they are.
    private var zoneUnits: [(id: String, rect: CGRect)] {
        layout.zones.filter { !$0.isField }.compactMap { z in
            guard let x = z.x, let y = z.y, let w = z.w, let h = z.h else { return nil }
            return (z.id, CGRect(x: x, y: y, width: w, height: h))
        }
    }

    private func capPlacements(in deck: CGRect, lit: [String: Lit]) -> [Cap] {
        let zones = zoneUnits
        return layout.placedKeys.enumerated().map { i, placed in
            let (bandY, bandH, key) = placed
            let r = frame(x: key.x, y: bandY,
                          w: max(key.w - layout.unit.keyGap, 0.1),
                          h: max(bandH - layout.unit.keyGap, 0.1), in: deck)
            let base = min(r.height * 0.34, r.width * 0.42)
            let pt = max(5, min(11, key.size == "tiny" ? base * 0.62
                                  : key.size == "small" ? base * 0.74 : base))
            let centre = CGPoint(x: key.x + key.w / 2, y: bandY + bandH / 2)
            let owner = zones.first { $0.rect.contains(centre) }
            let id = owner?.id ?? "keyboard"
            let model = lit[id] ?? Lit.dark
            // Normalised inside the zone that lights it, so a zone spreads its
            // palette across itself rather than across the whole deck.
            let t: Double
            if let owner {
                t = model.axis(nx: (centre.x - owner.rect.minX) / owner.rect.width,
                               ny: (centre.y - owner.rect.minY) / owner.rect.height)
            } else {
                t = model.axis(nx: centre.x / layout.unit.width,
                               ny: centre.y / layout.unit.height)
            }
            return Cap(id: i,
                       rect: r,
                       radius: r.height < 14 ? 2 : 3,
                       zone: id,
                       t: t,
                       label: key.label,
                       sub: key.sub,
                       point: pt,
                       homing: key.isHoming,
                       showsLegend: !key.isSpace && !key.label.isEmpty && r.height >= 9)
        }
    }

    /// A legend with nothing behind it — the keyboard's own printing.
    private var unlitInk: Color { finish.legend }

    /// The light that gets out.
    ///
    /// On a real board the plastic is opaque and the LED sits under it, so what
    /// you see is a halo in the gap around the cap. Drawn as one blurred layer
    /// under all the caps, which is both how it looks and one filter rather
    /// than eighty-four.
    private func bleed(_ caps: [Cap], in g: GraphicsContext,
                       lit: [String: Lit], at seconds: Double,
                       hits: [Int: Double], typed: Color?) {
        let spread = max(1.5, caps.first.map { $0.rect.height * 0.16 } ?? 2)
        g.drawLayer { layer in
            layer.addFilter(.blur(radius: spread * 0.9))
            for c in caps {
                let colour = (lit[c.zone] ?? Lit.dark).colour(c.t, at: seconds)
                layer.fill(Path(roundedRect: c.rect.insetBy(dx: -spread, dy: -spread),
                                cornerRadius: c.radius + spread, style: .continuous),
                           with: .color(colour))
            }
            // The keys being struck, over the top — which on a dark theme is
            // the only light there is.
            if let typed {
                for (i, strength) in hits where caps.indices.contains(i) {
                    let c = caps[i]
                    let lift = spread * (1 + strength)
                    layer.fill(Path(roundedRect: c.rect.insetBy(dx: -lift, dy: -lift),
                                    cornerRadius: c.radius + lift, style: .continuous),
                               with: .color(typed.opacity(strength)))
                }
            }
        }
    }

    /// The reactive colour the keys light in, if that layer is on at all.
    private var typedColour: Color? {
        guard look.typing.enabled else { return nil }
        return Color(hex: look.typing.color)
    }

    /// Which keys are lit by typing at this moment.
    ///
    /// Only the keys — the touch area has its own layer and its own colour, and
    /// the strips answer neither.
    private func typingHits(_ caps: [Cap], at seconds: Double) -> [Int: Double] {
        guard animated, !reduceMotion, look.typing.enabled else { return [:] }
        // `amount` is 1–3, low to high, and is how long a key stays lit.
        let hold = 0.35 + Double(look.typing.amount) * 0.28
        let typing = caps.enumerated().filter { $0.element.zone == "keyboard" }
        guard !typing.isEmpty else { return [:] }
        let picked = struck(typing.map(\.element), at: seconds, every: 0.24, hold: hold)
        // Back to indices in the full list.
        return Dictionary(uniqueKeysWithValues: picked.compactMap { local, strength in
            typing.indices.contains(local) ? (typing[local].offset, strength) : nil
        })
    }

    /// Keys lighting under an imaginary pair of hands.
    ///
    /// The reactive layer is most of what a blackout theme *is* — the deck
    /// stays dark and what you touch lights up — and a still black rectangle is
    /// a poor way to show it. So the preview types: a key every so often, held
    /// for as long as the duration says, fading out.
    ///
    /// Which keys is a hash of the beat number rather than a random draw, so a
    /// frame does not depend on how many frames came before it. A render at a
    /// given moment is the same render every time, which is what makes the
    /// snapshot harness worth anything.
    private func struck(_ caps: [Cap], at seconds: Double,
                        every beat: Double, hold: Double) -> [Int: Double] {
        var out: [Int: Double] = [:]
        guard !caps.isEmpty, beat > 0, hold > 0 else { return out }
        let now = Int(floor(seconds / beat))
        let reach = max(1, Int(hold / beat) + 1)
        for n in (now - reach)...now {
            let age = seconds - Double(n) * beat
            guard age >= 0, age < hold else { continue }
            // A cheap integer hash: two different beats land on two different
            // keys without a generator whose state has to be carried anywhere.
            var h = UInt64(bitPattern: Int64(n &* 6_364_136_223_846_793_005))
            h ^= h >> 29
            let i = Int(h % UInt64(caps.count))
            // Two strikes on one key take the brighter, not the sum.
            out[i] = max(out[i] ?? 0, 1 - age / hold)
        }
        return out
    }

    /// A keycap: opaque plastic, the specular edge it catches, and the bar you
    /// find F and J by.
    private func cap(_ c: Cap, in g: GraphicsContext) {
        g.fill(Path(roundedRect: c.rect, cornerRadius: c.radius, style: .continuous),
               with: .color(finish.cap))
        g.fill(Path(CGRect(x: c.rect.minX, y: c.rect.minY, width: c.rect.width, height: 1)),
               with: .color(finish.sheen))
        if c.homing {
            let w = c.rect.width * 0.34
            g.fill(Capsule().path(in: CGRect(x: c.rect.midX - w / 2,
                                             y: c.rect.maxY - 4.5,
                                             width: w, height: 1.5)),
                   with: .color(finish.legend.opacity(0.8)))
        }
    }

    /// What is printed on the keys.
    ///
    /// Laid out once. The ink is chosen for the light the zone gives *on
    /// average* rather than for this frame's colour: picking it per frame means
    /// every legend on the deck flips between black and white as a wave passes,
    /// which is flicker, not fidelity — and it is what cost a third of a core.
    private func legends(_ caps: [Cap], ink: Color, over size: CGSize) -> some View {
        ZStack(alignment: .topLeading) {
            ForEach(caps.filter(\.showsLegend)) { c in
                VStack(spacing: 0) {
                    Text(c.label)
                    if let sub = c.sub, !sub.isEmpty { Text(sub) }
                }
                .font(.system(size: c.point, weight: .medium))
                .foregroundStyle(ink)
                .minimumScaleFactor(0.5)   // shrink to fit; a blank cap reads as a missing key
                .lineLimit(1)
                .padding(.horizontal, 1)
                .frame(width: c.rect.width, height: c.rect.height)
                .offset(x: c.rect.minX, y: c.rect.minY)
            }
        }
        // The full box, pinned to the top left. Every legend is placed by an
        // offset from that corner, so a layer that sizes itself to its content
        // — which is what a mask does — slides all of them off their keys.
        .frame(width: size.width, height: size.height, alignment: .topLeading)
        .allowsHitTesting(false)
    }

    // MARK: - The touch surface and the strips

    /// The regions are placed by offset from the top left, so they need a box
    /// of the full size to be offset *within*. A `TimelineView` sizes itself to
    /// its content and centres it, which slid every strip off its own keys.
    /// Which zone you are pointing at — an outline, and nothing more.
    ///
    /// No zone is filled here any more. The touch area used to be washed over
    /// its keys at 42% of its own colour, mixing the keys' animation into a
    /// zone that has its own light so that neither showed honestly; the sliders
    /// were worse, drawn as opaque bars over the five keycaps each one covers,
    /// which hid F2–F11 and their legends and animated them as a strip rather
    /// than as the keys they are. Every zone's light is carried by its own keys
    /// now, so all that is left to draw is the boundary.
    /// A hairline is not a marker.
    ///
    /// These outlines are the only thing saying which zone you are editing, and
    /// a thin blue line over a lit deck does not carry that: it disappears into
    /// whatever colour is under it. The selected zone is drawn thick, doubled —
    /// a dark line under a bright one, so it holds against both a black
    /// keyboard and a white one — and breathes, because a thing that moves is
    /// found by the eye without looking for it.
    private func regions(in deck: CGRect, over size: CGSize) -> some View {
        // Only the selected outline moves, so the clock runs only when there is
        // one and stops the moment there is not.
        let pulses = !reduceMotion && layout.zones.contains { !$0.isField && $0.id == selected }
        return TimelineView(.animation(minimumInterval: 1.0 / 30.0, paused: !pulses)) { ctx in
            let beat = 0.5 + 0.5 * sin(ctx.date.timeIntervalSinceReferenceDate * 2.4)
            ZStack(alignment: .topLeading) {
                ForEach(layout.zones.filter { !$0.isField }) { z in
                    if let r = zoneRect(z, in: deck) {
                        let isSelected = z.id == selected
                        let landing = dragOver == z.id && dragFrom != nil && dragOver != dragFrom
                        let hovered = hoverZone == z.id && !isSelected
                        let radius: CGFloat = z.isStrip ? 5 : 9
                        let width: CGFloat = landing ? 5 : isSelected ? 4 : hovered ? 3 : 2.5
                        ZStack {
                            // The dark liner, so the bright one reads on a pale
                            // keyboard as well as on a dark one.
                            RoundedRectangle(cornerRadius: radius, style: .continuous)
                                .strokeBorder(.black.opacity(0.55), lineWidth: width + 2)
                            RoundedRectangle(cornerRadius: radius, style: .continuous)
                                .strokeBorder(landing || isSelected ? Color.accentColor
                                              : .white.opacity(hovered ? 0.95 : 0.6),
                                              lineWidth: width)
                            // The breath, only on what you are editing.
                            if isSelected && !landing {
                                RoundedRectangle(cornerRadius: radius, style: .continuous)
                                    .strokeBorder(Color.accentColor.opacity(0.35 + 0.45 * beat),
                                                  lineWidth: width + 4 * beat)
                                    .blur(radius: 2.5)
                            }
                        }
                        .frame(width: r.width, height: r.height)
                        .offset(x: r.minX, y: r.minY)
                        // The zone a drag would land on rises to meet it.
                        .scaleEffect(landing && !reduceMotion ? 1.015 : 1)
                    }
                }
            }
            .frame(width: size.width, height: size.height, alignment: .topLeading)
        }
        .allowsHitTesting(false)
    }

    // MARK: - Selecting and copying

    private func dragGesture(deck: CGRect) -> some Gesture {
        DragGesture(minimumDistance: 0)
            .onChanged { v in
                if dragFrom == nil { dragFrom = zone(at: v.startLocation, deck: deck) }
                dragOver = zone(at: v.location, deck: deck)
            }
            .onEnded { v in
                let from = dragFrom
                let to = zone(at: v.location, deck: deck)
                dragFrom = nil
                dragOver = nil
                guard let from else { return }
                if let to, to != from, v.translation != .zero {
                    onCopy(from, to)
                } else {
                    withAnimation(reduceMotion ? nil : DS.M.select) { onSelect(from) }
                }
            }
    }
}

/// A zone's light, resolved once.
///
/// Every colour the device gives is a `#RRGGBB` string; turning one into a
/// drawable colour means a colour-space conversion, and the old code did that
/// for every key on every frame to answer a question whose answer does not
/// change between frames. Brightness and opacity are folded in here too, so the
/// sliders still redraw the deck as the thumb moves — what the device is sent
/// still has them applied in the core.
struct Lit {
    let effect: String
    /// How many times the palette repeats across the zone: stretch spreads it,
    /// crossing once at the top of the range and repeating at the bottom.
    let repeats: Double
    /// Cycles per second, taken from the period the device stores rather than
    /// from a curve invented here. Each zone keeps its own, because each zone
    /// has its own speed — one clock stepped by the fastest zone on show made
    /// the other three run at a speed nothing had been set to.
    let rate: Double
    let angle: Double
    let anglesFree: Bool
    /// The stops with brightness applied, as components — kept for the effects
    /// that vary opacity per frame.
    let base: [(r: Double, g: Double, b: Double)]
    let alpha: Double
    /// The same stops as ready-made colours, which is the common path.
    let shaded: [Color]
    /// The light this zone gives on average, which is what the legend ink is
    /// chosen for.
    let average: Color

    static let dark = Lit()

    private init() {
        effect = "solidColor"
        repeats = 1
        rate = 0
        angle = 0
        anglesFree = false
        base = []
        alpha = 1
        shaded = []
        average = .black
    }

    init(_ z: LookModel.Zone, pivot: Int?) {
        effect = z.effect
        repeats = max(1, 1000 / Double(max(z.length, 1)))
        // Worked from `speed` through the device's own relation, not from the
        // `periodMs` that came with the look: that one is a snapshot of the
        // speed at the moment it was read, so dragging the slider moved a
        // number and left the animation exactly where it was.
        let period = Double(pivot.map { max($0 - z.speed, 1) } ?? (z.periodMs ?? 3_000))
        rate = 1000 / max(period, 1)
        angle = Double(z.angle)
        anglesFree = z.anglesFree
        alpha = Double(z.opacity) / 100
        let scale = Double(z.brightness) / 100
        base = z.preview.compactMap { Color(hex: $0) }.map {
            let n = NSColor($0).usingColorSpace(.sRGB) ?? .black
            return (n.redComponent * scale, n.greenComponent * scale, n.blueComponent * scale)
        }
        let a = alpha
        shaded = base.map { Color(.sRGB, red: $0.r, green: $0.g, blue: $0.b, opacity: a) }
        if base.isEmpty {
            average = .black
        } else {
            let n = Double(base.count)
            average = Color(.sRGB,
                            red: base.reduce(0) { $0 + $1.r } / n,
                            green: base.reduce(0) { $0 + $1.g } / n,
                            blue: base.reduce(0) { $0 + $1.b } / n)
        }
    }

    /// Where along the gradient a point sits, given the zone's angle.
    ///
    /// `nx` and `ny` are normalised **within the zone**, not across the deck:
    /// the touch area and the strips cover their own patch of the keyboard and
    /// spread their palette across that patch, not across the whole board.
    ///
    /// The angle is degrees anticlockwise from "to the right", which is what
    /// the device stores and `effects.rs` encodes. The earlier version put the
    /// angle's cosine on `x` with the wrong sign, which turned every direction
    /// a quarter turn and reversed it: the control said "up" and the light
    /// crossed to the left, "right" ran to the top, and so on all the way
    /// round. `t` grows the way the light travels.
    func axis(nx: Double, ny: Double) -> Double {
        // A strip is one-dimensional: projecting onto an arbitrary angle would
        // put the whole gradient across its few pixels of thickness and show
        // one flat colour. `angle_for_zone` has already snapped it to 0 or 180.
        if !anglesFree {
            return (angle > 90 && angle <= 270) ? 1 - nx : nx
        }
        let rad = angle * .pi / 180
        return 0.5 + ((nx - 0.5) * cos(rad) - (ny - 0.5) * sin(rad))
    }

    /// How far through its cycle this zone is at a given moment.
    ///
    /// Its own speed, not the deck's: `period` milliseconds is one cycle.
    func phase(at seconds: Double) -> Double {
        let p = (seconds * rate).truncatingRemainder(dividingBy: 1)
        return p < 0 ? p + 1 : p
    }

    /// The colour this zone shows at `t` along it, at this moment.
    func colour(_ t: Double, at seconds: Double) -> Color {
        guard !shaded.isEmpty else { return .black }
        let phase = phase(at: seconds)
        switch effect {
        case "colorWave":  return shaded[index(t * repeats - phase)]
        case "colorCycle": return shaded[index(phase)]
        case "breathing":
            let breath = 0.25 + 0.75 * (0.5 + 0.5 * cos(phase * 2 * .pi))
            let c = base[0]
            return Color(.sRGB, red: c.r, green: c.g, blue: c.b, opacity: alpha * breath)
        case "aurora":     return shaded[index(sin((t * 2 + phase * 2) * .pi) * 0.5 + 0.5)]
        default:           return shaded[0]
        }
    }

    private func index(_ u: Double) -> Int {
        var x = u.truncatingRemainder(dividingBy: 1)
        if x < 0 { x += 1 }
        return min(Int(x * Double(shaded.count)), shaded.count - 1)
    }
}
