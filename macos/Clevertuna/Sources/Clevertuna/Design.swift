import SwiftUI

/// The design, as values.
///
/// Every number here is lifted from the handoff in `other/design/` — the Kit
/// artboard is the material sheet, and the other four are it in use. Nothing in
/// this file is invented: if a value is wrong it is wrong against a specific
/// line of a specific artboard, which is the point of writing it down rather
/// than approximating it with stock controls.
///
/// The earlier version of this app used `Picker(.segmented)`, `Slider` and
/// `ColorPicker` and looked like a settings pane from any other app. The design
/// asks for a 4pt track with an 18pt knob, a capsule pop-up 26 tall, a
/// segmented capsule 30 tall with a 13-radius thumb. So those are built.
enum DS {

    // MARK: - Radii, concentric

    enum R {
        static let window: CGFloat = 26
        static let panel: CGFloat = 22
        static let group: CGFloat = 18
        /// The inspector's own groups sit tighter than a standalone group.
        static let groupTight: CGFloat = 14
        static let tile: CGFloat = 13
        static let control: CGFloat = 15
        static let popup: CGFloat = 13
    }

    // MARK: - Type

    enum F {
        /// 30 / 680 — what the pane is about.
        static let subject = Font.system(size: 30, weight: .heavy)
        /// 17 / 640 — the name of a pane.
        static let pane = Font.system(size: 17, weight: .semibold)
        /// 13 / 400 — body, and every control label.
        static let body = Font.system(size: 13)
        static let bodyMedium = Font.system(size: 13, weight: .medium)
        static let bodyStrong = Font.system(size: 13, weight: .semibold)
        /// 11 / 400 — secondary, and the keys beside a theme.
        static let secondary = Font.system(size: 11)
        static let secondaryMedium = Font.system(size: 11, weight: .medium)
        /// 11 / 590, letterspaced, upper — a group's name.
        static let sectionHeader = Font.system(size: 11, weight: .semibold)
        static let toolbarTitle = Font.system(size: 14, weight: .semibold)
        static let value = Font.system(size: 13).monospacedDigit()
    }

    // MARK: - Ink

    enum Ink {
        static let primary = Color(red: 0.961, green: 0.961, blue: 0.969)   // #F5F5F7
        static let dim = Color(red: 0.961, green: 0.961, blue: 0.969).opacity(0.62)
        static let faint = Color(red: 0.961, green: 0.961, blue: 0.969).opacity(0.50)
        static let onLight = Color(red: 0.114, green: 0.114, blue: 0.122)   // #1D1D1F
        static let onLightDim = Color(red: 0.525, green: 0.525, blue: 0.545) // #86868B
        static let link = Color(red: 0.392, green: 0.710, blue: 1.0)        // #64B5FF
    }

    // MARK: - Motion
    //
    // One vocabulary, so nothing in the app moves in a way nothing else does.
    // Every one of these is short: this is a utility that sits behind your work,
    // and a flourish you wait for is a flourish you resent by the tenth time.

    enum M {
        /// A control acknowledging a press.
        static let press = Animation.spring(response: 0.22, dampingFraction: 0.7)
        /// Something arriving or leaving under the pointer.
        static let hover = Animation.easeOut(duration: 0.12)
        /// A selection moving from one place to another.
        static let select = Animation.spring(response: 0.32, dampingFraction: 0.78)
        /// A whole surface changing what it is showing.
        static let surface = Animation.easeInOut(duration: 0.28)
    }

    // MARK: - Spacing the artboards use

    enum S {
        static let windowInset: CGFloat = 12
        static let toolbarHeight: CGFloat = 48
        /// The window's own title bar, which the builder's toolbar *is* rather
        /// than sits below: tall enough to carry a 30pt control either side of
        /// the traffic lights.
        static let titlebarHeight: CGFloat = 52
        /// Clear of the traffic lights.
        static let trafficLightInset: CGFloat = 78
        static let inspectorWidth: CGFloat = 306
        static let inspectorPad: CGFloat = 16
        static let rowHeight: CGFloat = 34
        static let groupGap: CGFloat = 18
    }
}

// MARK: - The material

/// Quiet glass: the inner surface a group or a control sits on.
///
/// Not `glassEffect` — the design distinguishes the floating panes, which
/// refract what is behind them, from the surfaces *inside* a pane, which are a
/// flat lift off it. Making everything refract turns an inspector into soup.
struct QuietGlass: ViewModifier {
    var radius: CGFloat = DS.R.groupTight

    func body(content: Content) -> some View {
        content.background {
            RoundedRectangle(cornerRadius: radius, style: .continuous)
                .fill(LinearGradient(colors: [.white.opacity(0.15), .white.opacity(0.07)],
                                     startPoint: .top, endPoint: .bottom))
                .overlay {
                    RoundedRectangle(cornerRadius: radius, style: .continuous)
                        .strokeBorder(.white.opacity(0.12), lineWidth: 0.5)
                }
                .overlay(alignment: .top) {
                    // The specular rim, brightest along the top edge.
                    RoundedRectangle(cornerRadius: radius, style: .continuous)
                        .strokeBorder(.white.opacity(0.32), lineWidth: 0.5)
                        .mask(LinearGradient(colors: [.white, .clear],
                                             startPoint: .top, endPoint: .center))
                }
        }
    }
}

extension View {
    func quietGlass(_ radius: CGFloat = DS.R.groupTight) -> some View {
        modifier(QuietGlass(radius: radius))
    }
}

// MARK: - Waiting

/// A light travelling across something that is working.
///
/// Every action in this app goes out to a keyboard over Bluetooth, and a flash
/// write takes long enough to read as a dead button — which gets pressed again,
/// which queues a second write behind the first. There is no shortening the
/// wait past what the hardware takes, so the wait is *shown*: the control that
/// is acting carries a sweep for exactly as long as it is acting, and stops the
/// instant the keyboard answers.
///
/// Driven from a `TimelineView`'s clock rather than a repeating animation on
/// state, so it neither invalidates a view tree thirty times a second nor
/// leaves an animation running after the thing it described has finished.
struct Shimmer: ViewModifier {
    var active: Bool
    var radius: CGFloat = DS.R.tile
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    func body(content: Content) -> some View {
        content.overlay {
            if active {
                if reduceMotion {
                    // The same promise without the movement: still says the
                    // control is busy, which is the part that matters.
                    RoundedRectangle(cornerRadius: radius, style: .continuous)
                        .fill(.white.opacity(0.14))
                } else {
                    TimelineView(.animation(minimumInterval: 1.0 / 30.0)) { ctx in
                        let t = ctx.date.timeIntervalSinceReferenceDate
                        let p = (t * 0.9).truncatingRemainder(dividingBy: 1)
                        GeometryReader { geo in
                            LinearGradient(
                                colors: [.white.opacity(0), .white.opacity(0.34), .white.opacity(0)],
                                startPoint: .leading, endPoint: .trailing)
                                .frame(width: geo.size.width * 0.55)
                                // From just off one edge to just off the other.
                                .offset(x: -geo.size.width * 0.55 + p * geo.size.width * 1.55)
                        }
                    }
                    .clipShape(RoundedRectangle(cornerRadius: radius, style: .continuous))
                    .allowsHitTesting(false)
                }
            }
        }
    }
}

extension View {
    /// Shown as working, for as long as it is.
    func shimmering(_ active: Bool, radius: CGFloat = DS.R.tile) -> some View {
        modifier(Shimmer(active: active, radius: radius))
    }
}

// MARK: - Controls the design specifies

/// A group: a name in small caps, then its rows on quiet glass.
struct DSGroup<Content: View>: View {
    let title: String?
    @ViewBuilder var content: Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            if let title {
                Text(title.uppercased())
                    .font(DS.F.sectionHeader)
                    .kerning(0.4)
                    .foregroundStyle(DS.Ink.faint)
            }
            VStack(spacing: 0) { content }
                .padding(.horizontal, 12)
                .padding(.vertical, 4)
                .quietGlass()
        }
    }
}

/// A row inside a group: 34 tall, label left, control right.
struct DSRow<Content: View>: View {
    let label: String?
    var minHeight: CGFloat = DS.S.rowHeight
    @ViewBuilder var content: Content

    var body: some View {
        HStack(spacing: 10) {
            if let label {
                Text(label).font(DS.F.body).foregroundStyle(DS.Ink.primary)
            }
            content
        }
        .frame(minHeight: minHeight)
    }
}

/// The slider the design draws: a 4pt track, a fill that runs from the light's
/// own colour to white, and an 18pt knob. `Slider` gives none of that.
struct DSSlider: View {
    @Binding var value: Double
    var range: ClosedRange<Double>
    /// The colour the fill starts from — the zone's own light.
    var tint: Color = Color(red: 0.039, green: 0.518, blue: 1.0)

    @State private var dragging = false
    @State private var hovering = false

    private let track: CGFloat = 4
    private let knob: CGFloat = 18

    var body: some View {
        GeometryReader { geo in
            let span = max(range.upperBound - range.lowerBound, 0.0001)
            let t = min(max((value - range.lowerBound) / span, 0), 1)
            let usable = max(geo.size.width - knob, 1)
            let x = usable * t

            ZStack(alignment: .leading) {
                Capsule().fill(.white.opacity(0.16))
                    .frame(height: track)
                Capsule()
                    .fill(LinearGradient(colors: [tint, .white],
                                         startPoint: .leading, endPoint: .trailing))
                    .frame(width: x + knob / 2, height: track)
                Circle()
                    .fill(LinearGradient(colors: [.white, Color(red: 0.914, green: 0.914, blue: 0.925)],
                                         startPoint: .top, endPoint: .bottom))
                    .frame(width: knob, height: knob)
                    .shadow(color: .black.opacity(dragging ? 0.5 : 0.4),
                            radius: dragging ? 4 : 2, y: 1)
                    // The knob takes hold under the finger, and lets go after.
                    .scaleEffect(dragging ? 1.12 : hovering ? 1.05 : 1)
                    .offset(x: x)
            }
            .frame(height: geo.size.height, alignment: .center)
            .contentShape(.rect)
            .onHover { hovering = $0 }
            .animation(DS.M.press, value: dragging)
            .animation(DS.M.hover, value: hovering)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { g in
                        dragging = true
                        let p = min(max((g.location.x - knob / 2) / usable, 0), 1)
                        value = range.lowerBound + p * span
                    }
                    .onEnded { _ in dragging = false }
            )
        }
        .frame(height: knob)
    }
}

/// A labelled slider row: name at 74 wide, track, value right-aligned.
struct DSSliderRow: View {
    let name: String
    @Binding var value: Double
    var range: ClosedRange<Double>
    var suffix: String = ""
    var tint: Color = Color(red: 0.039, green: 0.518, blue: 1.0)
    var enabled: Bool = true

    var body: some View {
        HStack(spacing: 10) {
            Text(name)
                .font(DS.F.body)
                .foregroundStyle(enabled ? DS.Ink.primary : DS.Ink.faint)
                .frame(width: 74, alignment: .leading)
            DSSlider(value: $value, range: range, tint: tint)
                .disabled(!enabled)
                .opacity(enabled ? 1 : 0.45)
            Text("\(Int(value.rounded()))\(suffix)")
                .font(DS.F.secondary.monospacedDigit())
                .foregroundStyle(DS.Ink.dim)
                .frame(width: 40, alignment: .trailing)
        }
        .frame(minHeight: DS.S.rowHeight)
    }
}

/// The segmented capsule: 30 tall, radius 15, a 13-radius thumb behind the
/// selection.
struct DSSegmented: View {
    @Binding var selection: String
    let options: [(id: String, label: String)]

    @Namespace private var thumb
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        HStack(spacing: 0) {
            ForEach(options, id: \.id) { option in
                let on = option.id == selection
                Text(option.label)
                    .font(on ? DS.F.bodyStrong : DS.F.body)
                    .foregroundStyle(on ? DS.Ink.primary : DS.Ink.dim)
                    .frame(maxWidth: .infinity)
                    .frame(height: 26)
                    .background {
                        // The thumb travels to the segment you picked, so the
                        // eye follows the selection instead of hunting for it.
                        if on {
                            RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                                .fill(.white.opacity(0.24))
                                .shadow(color: .black.opacity(0.2), radius: 1.5, y: 1)
                                .matchedGeometryEffect(id: "thumb", in: thumb)
                        }
                    }
                    .contentShape(.rect)
                    .onTapGesture {
                        withAnimation(reduceMotion ? nil : DS.M.select) { selection = option.id }
                    }
            }
        }
        .padding(2)
        .frame(height: 30)
        .quietGlass(DS.R.control)
    }
}

/// The up/down pair the design draws on every pop-up, 11 × 14.
struct Chevrons: View {
    var dark = false

    var body: some View {
        Path { p in
            p.move(to: CGPoint(x: 3, y: 5.5)); p.addLine(to: CGPoint(x: 5.5, y: 3)); p.addLine(to: CGPoint(x: 8, y: 5.5))
            p.move(to: CGPoint(x: 3, y: 8.5)); p.addLine(to: CGPoint(x: 5.5, y: 11)); p.addLine(to: CGPoint(x: 8, y: 8.5))
        }
        .stroke((dark ? Color.black : DS.Ink.primary).opacity(0.6),
                style: .init(lineWidth: 1.5, lineCap: .round, lineJoin: .round))
        .frame(width: 11, height: 14)
    }
}

/// A capsule button: 30 tall, radius 15. Quiet by default; the one control that
/// writes to the keyboard wears the accent.
struct DSButton: View {
    let title: String
    var systemImage: String?
    var prominent = false
    var action: () -> Void

    @State private var hovering = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button(action: action) {
            HStack(spacing: 7) {
                if let systemImage { Image(systemName: systemImage).font(.system(size: 13)) }
                Text(title).font(prominent ? DS.F.bodyStrong : DS.F.body)
            }
            .foregroundStyle(prominent ? Color.white : DS.Ink.primary)
            .padding(.horizontal, 15)
            .frame(height: 30)
            .background {
                if prominent {
                    Capsule()
                        .fill(LinearGradient(
                            colors: [Color(red: 0.227, green: 0.627, blue: 1.0),
                                     Color(red: 0.039, green: 0.518, blue: 1.0)],
                            startPoint: .top, endPoint: .bottom))
                        .shadow(color: Color(red: 0.039, green: 0.518, blue: 1.0).opacity(0.40),
                                radius: 10, y: 4)
                }
            }
            .modifier(QuietCapsule(active: !prominent))
            .brightness(hovering ? 0.06 : 0)
        }
        .buttonStyle(PressableButtonStyle(reduceMotion: reduceMotion))
        // A capsule wearing a square blue system focus ring is not this design,
        // and it appeared on every toolbar button the moment the window took
        // key. The press and hover states already say what is under the finger.
        .focusEffectDisabled()
        .onHover { hovering = $0 }
        .animation(DS.M.hover, value: hovering)
    }
}

/// Everything clickable answers the click: a small give under the finger, and
/// nothing at all when Reduce Motion is on.
struct PressableButtonStyle: ButtonStyle {
    var reduceMotion = false
    var scale: CGFloat = 0.97

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .scaleEffect(configuration.isPressed && !reduceMotion ? scale : 1)
            .opacity(configuration.isPressed ? 0.85 : 1)
            .animation(DS.M.press, value: configuration.isPressed)
    }
}

private struct QuietCapsule: ViewModifier {
    let active: Bool
    func body(content: Content) -> some View {
        if active { content.quietGlass(DS.R.control) } else { content }
    }
}

/// A colour, shown as the pill the design uses: 36 × 22, radius 11.
struct DSColourPill: View {
    let colour: Color
    var width: CGFloat = 36
    var height: CGFloat = 22

    var body: some View {
        RoundedRectangle(cornerRadius: height / 2, style: .continuous)
            .fill(colour)
            .frame(width: width, height: height)
            .overlay {
                RoundedRectangle(cornerRadius: height / 2, style: .continuous)
                    .strokeBorder(.white.opacity(0.35), lineWidth: 0.5)
            }
    }
}

/// A theme tile: 74 wide, radius 13, a 30-tall swatch, its name and its key.
struct DSTile: View {
    let name: String
    let key: String
    let colours: [Color]
    var selected = false
    /// This tile is the one the keyboard is busy with.
    var working = false
    /// Something else is, so this one will not act until it is done.
    var waiting = false
    var action: () -> Void

    @State private var hovering = false
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    var body: some View {
        Button(action: action) {
            VStack(alignment: .leading, spacing: 6) {
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .fill(colours.count > 1
                          ? AnyShapeStyle(LinearGradient(colors: colours, startPoint: .leading, endPoint: .trailing))
                          : AnyShapeStyle(colours.first ?? .gray))
                    .frame(height: 30)
                    .overlay {
                        RoundedRectangle(cornerRadius: 9, style: .continuous)
                            .strokeBorder(.white.opacity(0.35), lineWidth: 0.5)
                    }
                HStack(alignment: .firstTextBaseline, spacing: 4) {
                    Text(name)
                        .font(DS.F.secondaryMedium)
                        .foregroundStyle(DS.Ink.primary)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Spacer(minLength: 0)
                    Text(key).font(DS.F.secondary).foregroundStyle(DS.Ink.dim)
                }
            }
            .padding(7)
            .background {
                RoundedRectangle(cornerRadius: DS.R.tile, style: .continuous)
                    .fill(.white.opacity(selected ? 0.20 : hovering ? 0.13 : 0.07))
                    .overlay {
                        RoundedRectangle(cornerRadius: DS.R.tile, style: .continuous)
                            .strokeBorder(.white.opacity(selected ? 0.30 : 0.20), lineWidth: selected ? 1 : 0.5)
                    }
            }
        }
        .buttonStyle(PressableButtonStyle(reduceMotion: reduceMotion, scale: 0.96))
        .shimmering(working)
        .focusEffectDisabled()
        // A press that cannot act is refused rather than queued: the keyboard
        // takes one conversation at a time, and stacking writes on it is what
        // made the second tap slower than the first.
        .disabled(waiting)
        .opacity(waiting ? 0.55 : 1)
        .onHover { hovering = $0 && !waiting }
        .animation(DS.M.hover, value: hovering)
        .animation(DS.M.select, value: selected)
        .animation(DS.M.hover, value: waiting)
    }
}

/// The switch: 40 × 24, a 20 knob, green when on.
struct DSSwitch: View {
    @Binding var isOn: Bool
    var enabled = true

    var body: some View {
        Capsule()
            .fill(isOn
                  ? AnyShapeStyle(LinearGradient(colors: [Color(red: 0.204, green: 0.843, blue: 0.357),
                                                          Color(red: 0.157, green: 0.749, blue: 0.298)],
                                                 startPoint: .top, endPoint: .bottom))
                  : AnyShapeStyle(Color.white.opacity(0.18)))
            .frame(width: 40, height: 24)
            .overlay(alignment: isOn ? .trailing : .leading) {
                Circle()
                    .fill(LinearGradient(colors: [.white, Color(red: 0.949, green: 0.949, blue: 0.961)],
                                         startPoint: .top, endPoint: .bottom))
                    .frame(width: 20, height: 20)
                    .shadow(color: .black.opacity(0.28), radius: 1.5, y: 1)
                    .padding(2)
            }
            .opacity(enabled ? 1 : 0.5)
            .contentShape(.capsule)
            .onTapGesture { if enabled { withAnimation(.snappy(duration: 0.18)) { isOn.toggle() } } }
    }
}

// MARK: - A pop-up that actually looks like the design

/// SwiftUI's `Menu` will not let its label carry a background: the capsule the
/// handoff draws simply does not paint, and the control comes out as bare text.
/// So the menu is `NSMenu`, popped from an anchor, and the label is ours.
///
/// **The anchor opens its own menu.** The first version handed the `NSView`
/// back to SwiftUI through a `@State` written from `makeNSView` — that is a
/// state mutation during a view update, so the binding never settled and the
/// tap gesture called `present` on a `nil` anchor. The control looked right,
/// hovered, and did nothing at all when clicked. Nothing here writes SwiftUI
/// state from an update any more: the view takes the click, the hover and the
/// selection itself, and hands back only events.
struct MenuAnchor: NSViewRepresentable {
    final class Anchor: NSView {
        var items: [(title: String, run: () -> Void)] = []
        var selected = ""
        var enabled = true
        var onHover: (Bool) -> Void = { _ in }

        /// Fixed rather than inherited, so "under the control" is one
        /// arithmetic in one coordinate system.
        override var isFlipped: Bool { true }

        override func updateTrackingAreas() {
            super.updateTrackingAreas()
            for area in trackingAreas { removeTrackingArea(area) }
            addTrackingArea(NSTrackingArea(
                rect: bounds,
                options: [.mouseEnteredAndExited, .activeInActiveApp, .inVisibleRect],
                owner: self))
        }

        override func mouseEntered(with event: NSEvent) { onHover(enabled) }
        override func mouseExited(with event: NSEvent) { onHover(false) }

        override func mouseDown(with event: NSEvent) {
            guard enabled, !items.isEmpty else { return }
            let menu = NSMenu()
            // The target is ours and always answers; leaving AppKit to work
            // that out for itself greys the whole menu if it decides otherwise.
            menu.autoenablesItems = false
            for item in items {
                let row = NSMenuItem(title: item.title, action: #selector(Handler.fire(_:)), keyEquivalent: "")
                row.target = handler
                row.representedObject = Handler.Box(item.run)
                row.state = item.title == selected ? .on : .off
                menu.addItem(row)
            }
            // Under the control, left-aligned, the way a pop-up opens.
            menu.popUp(positioning: nil, at: NSPoint(x: 0, y: bounds.height + 4), in: self)
        }

        private let handler = Handler()

        final class Handler: NSObject {
            final class Box: NSObject {
                let run: () -> Void
                init(_ run: @escaping () -> Void) { self.run = run }
            }
            @objc func fire(_ sender: NSMenuItem) {
                (sender.representedObject as? Box)?.run()
            }
        }
    }

    let items: [(title: String, run: () -> Void)]
    let selected: String
    var enabled = true
    let onHover: (Bool) -> Void

    func makeNSView(context: Context) -> Anchor { apply(to: Anchor()) }

    func updateNSView(_ nsView: Anchor, context: Context) { _ = apply(to: nsView) }

    private func apply(to view: Anchor) -> Anchor {
        view.items = items
        view.selected = selected
        view.enabled = enabled
        view.onHover = onHover
        return view
    }
}

/// The pop-up the design draws: the value, a chevron pair, on a capsule.
struct DSPopUpButton: View {
    let value: String
    let options: [String]
    var light = false
    var enabled = true
    let onChange: (String) -> Void

    @State private var hovering = false

    var body: some View {
        HStack(spacing: 7) {
            Text(value)
                .font(DS.F.body)
                .foregroundStyle(light ? DS.Ink.onLight : DS.Ink.primary)
            Chevrons(dark: light)
        }
        .padding(.leading, light ? 13 : 12)
        .padding(.trailing, 8)
        .frame(height: light ? 28 : 26)
        .background {
            if light {
                RoundedRectangle(cornerRadius: 14, style: .continuous)
                    .fill(LinearGradient(colors: [.white, Color(red: 0.965, green: 0.965, blue: 0.973)],
                                         startPoint: .top, endPoint: .bottom))
                    .overlay {
                        RoundedRectangle(cornerRadius: 14, style: .continuous)
                            .strokeBorder(.black.opacity(hovering ? 0.20 : 0.12), lineWidth: 0.5)
                    }
                    .shadow(color: .black.opacity(0.07), radius: 1, y: 1)
            } else {
                RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                    .fill(.white.opacity(hovering ? 0.18 : 0.11))
                    .overlay {
                        RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                            .strokeBorder(.white.opacity(0.14), lineWidth: 0.5)
                    }
            }
        }
        // The anchor is the click target: it sits over the pill, opens the
        // menu itself, and reports hover back. Nothing routes a press through
        // SwiftUI state that an update would have to settle first.
        .overlay {
            MenuAnchor(items: options.map { o in (o, { onChange(o) }) },
                       selected: value,
                       enabled: enabled) { hovering = $0 }
        }
        .opacity(enabled ? 1 : 0.55)
        .animation(.easeOut(duration: 0.1), value: hovering)
    }
}
