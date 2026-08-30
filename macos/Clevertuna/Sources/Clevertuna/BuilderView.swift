import SwiftUI

/// The Theme Builder, built from the handoff rather than from memory of it.
///
/// Panes of glass floating over the content: a toolbar 48 tall at the top, an
/// inspector 306 wide down the right, the deck on a stage between them, and an
/// action bar at the foot where the one control that writes to the keyboard is
/// the only one wearing the accent.
///
/// The controls are the ones in `Design.swift`, which are the ones the material
/// sheet draws. Stock `Picker`, `Slider` and `ColorPicker` are not those, and
/// using them made this window look like a settings pane from any other app.
struct BuilderView: View {
    @Environment(BuilderModel.self) private var model
    @State private var prompt: Prompt?

    var body: some View {
        @Bindable var model = model
        ZStack {
            Stage(bloom: model.bloom)

            GlassEffectContainer(spacing: 20) {
                VStack(spacing: 0) {
                    // Flush to the top, full width: this *is* the title bar,
                    // and the traffic lights sit on it. It used to be a second
                    // floating bar inset below the real one, which read as two
                    // title bars stacked.
                    toolbar
                    HStack(alignment: .top, spacing: DS.S.windowInset + 4) {
                        VStack(alignment: .leading, spacing: 14) {
                            zoneChip
                            deck
                            actions
                        }
                        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
                        inspector.frame(width: DS.S.inspectorWidth)
                    }
                    .padding(DS.S.windowInset)
                    .frame(maxHeight: .infinity)
                }
            }
            .ignoresSafeArea(edges: .top)
        }
        .frame(minWidth: 1000, minHeight: 720)
        .task { await model.load() }
        .sheet(item: $prompt) { p in
            PromptSheet(prompt: p) { prompt = nil }
                .environment(model)
        }
    }

    // MARK: - Toolbar

    /// The window's title bar.
    ///
    /// Everything it offers is a button you can see. The three that used to be
    /// folded behind a glyph — copying a zone's light, Themes, Settings — were
    /// three of the five things this window does, and a menu is where a control
    /// goes when there is no room for it, not where the main ones live.
    private var toolbar: some View {
        HStack(spacing: 10) {
            Text("Theme Builder").font(DS.F.toolbarTitle).foregroundStyle(DS.Ink.primary)
            Spacer(minLength: 0)
            DSButton(title: "Roll", systemImage: "die.face.5") { Task { await model.roll() } }
            DSButton(title: "Read", systemImage: "arrow.clockwise") { Task { await model.load() } }
            DSButton(title: "Copy zone", systemImage: "square.on.square") { prompt = .copyZone }
            DSButton(title: "To slot", systemImage: "arrow.left.arrow.right") { prompt = .copySlot }
            DSButton(title: "Themes", systemImage: "swatchpalette") { Windows.shared.show(.themes) }
            DSButton(title: "Settings", systemImage: "gearshape") { Windows.shared.show(.settings) }
        }
        .padding(.leading, DS.S.trafficLightInset)
        .padding(.trailing, 14)
        .frame(maxWidth: .infinity)
        .frame(height: DS.S.titlebarHeight)
        .background {
            ZStack {
                Rectangle().fill(.black.opacity(0.28))
                Rectangle().fill(.white.opacity(0.05))
            }
            .overlay(alignment: .bottom) {
                Rectangle().fill(.white.opacity(0.10)).frame(height: 0.5)
            }
        }
        // A title bar is what you drag a window by — and now the only thing,
        // since the window no longer moves by its background. Dragging a
        // colour slider used to carry the whole window with it.
        .gesture(WindowDragGesture())
    }

    // MARK: - The deck

    /// The chip the design floats above the deck: a dot in the zone's own light,
    /// then what that zone is doing.
    private var zoneChip: some View {
        HStack(spacing: 8) {
            Circle()
                .fill(model.zoneTint)
                .frame(width: 7, height: 7)
            Text(model.zoneCaption).font(DS.F.secondaryMedium).foregroundStyle(DS.Ink.primary)
        }
        .padding(.horizontal, 12)
        .frame(height: 26)
        .quietGlass(DS.R.popup)
    }

    private var deck: some View {
        Group {
            if let look = model.look {
                // The clock is the deck's own, and it only drives the layers
                // that move: pushing `phase` through from here rebuilt every
                // key, its legend and every region thirty times a second.
                DeckView(look: look,
                         selected: model.selectedZone,
                         animated: model.isAnimated,
                         clock: { model.elapsed(at: $0) },
                         finish: model.finish,
                         onSelect: { model.selectedZone = $0 },
                         onCopy: { from, to in Task { await model.copy(from: from, to: to) } })
            } else {
                ProgressView()
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }

    // MARK: - Actions

    private var actions: some View {
        HStack(spacing: 10) {
            DSButton(title: "Save as Theme…") { prompt = .save }
            DSButton(title: "Apply to Keyboard", prominent: true) { Task { await model.apply() } }
                .shimmering(model.busy != nil, radius: 15)
                .disabled(model.busy != nil)
            HStack(spacing: 7) {
                if model.busy != nil {
                    ProgressView().controlSize(.small).scaleEffect(0.65)
                }
                Text(model.status)
                    .font(DS.F.secondary)
                    .foregroundStyle(model.failure == nil ? DS.Ink.dim
                                     : Color(red: 1.0, green: 0.84, blue: 0.25))
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.leading, 6)
            .animation(DS.M.hover, value: model.busy)
            Spacer(minLength: 0)
        }
    }

    // MARK: - Inspector

    private var inspector: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: DS.S.groupGap) {
            zoneHeader

            DSSegmented(selection: $model.selectedZone,
                        options: zoneOrder.map { ($0, zoneShort[$0] ?? $0) })

            if let zone = model.zone {
                let offer = zone.offer
                let tint = model.zoneTint

                DSGroup(title: "Effect") {
                    DSRow(label: "Style") {
                        Spacer(minLength: 0)
                        DSPopUpButton(value: effectLabel(zone.effect),
                                      options: zone.offers.map(\.label)) { label in
                            if let key = zone.offers.first(where: { $0.label == label })?.key {
                                model.setEffect(key)
                            }
                        }
                    }
                }

                // Only what this effect honours. A solid colour has no speed to
                // set and no gradient to spread, and offering those invites a
                // change the keyboard will ignore.
                if offer?.colours ?? true {
                    DSGroup(title: zone.stops.count > 1 ? "Colours" : "Colour") {
                        ForEach(Array(zone.stops.enumerated()), id: \.offset) { i, stop in
                            DSRow(label: nil) {
                                DSColourPill(colour: Color(hex: stop.color) ?? .black)
                                    .onTapGesture { model.pickColour(i) }
                                if offer?.gradient == true {
                                    DSSlider(value: model.stopPosition(i), range: 0...100, tint: tint)
                                } else {
                                    Spacer(minLength: 0)
                                }
                                if zone.stops.count > 1 {
                                    Button { model.removeStop(i) } label: {
                                        Image(systemName: "minus.circle.fill")
                                            .foregroundStyle(DS.Ink.dim)
                                    }
                                    .buttonStyle(.plain)
                                }
                            }
                        }
                        if offer?.gradient == true, zone.stops.count < (model.look?.ranges.markers ?? 5) {
                            DSRow(label: nil) {
                                Button { model.addStop() } label: {
                                    Label("Add a colour", systemImage: "plus.circle.fill")
                                        .font(DS.F.body)
                                        .foregroundStyle(DS.Ink.link)
                                }
                                .buttonStyle(.plain)
                                Spacer(minLength: 0)
                            }
                        }
                    }
                }

                DSGroup(title: "Light") {
                    DSSliderRow(name: "Brightness", value: model.brightnessBinding,
                                range: model.range(\.brightness), suffix: "%", tint: tint)
                    DSSliderRow(name: "Opacity", value: model.opacityBinding,
                                range: model.range(\.opacity), suffix: "%", tint: tint)
                }

                if offer?.animated == true || offer?.length == true || offer?.gradient == true {
                    DSGroup(title: "Movement") {
                        if offer?.speed == true {
                            DSSliderRow(name: "Speed", value: model.speedBinding,
                                        range: model.range(\.speed), tint: tint)
                        }
                        if offer?.length == true {
                            DSSliderRow(name: "Stretch", value: model.lengthBinding,
                                        range: model.range(\.length), tint: tint)
                        }
                        if offer?.gradient == true {
                            DirectionRow(angle: model.angleBinding, free: zone.anglesFree, tint: tint)
                        }
                    }
                }

                // The layer over the top, which is the whole point of a
                // blackout: the keys stay dark until you touch them.
                if let reactive = model.reactive {
                    DSGroup(title: model.reactiveTitle) {
                        DSRow(label: reactive.label == "Trace" ? "Trail" : "Light up") {
                            Spacer(minLength: 0)
                            DSSwitch(isOn: model.reactiveOn)
                        }
                        if reactive.enabled {
                            DSRow(label: "Colour") {
                                Spacer(minLength: 0)
                                DSColourPill(colour: Color(hex: reactive.color) ?? .white)
                                    .onTapGesture { model.pickReactiveColour() }
                            }
                            DSSliderRow(name: reactive.label,
                                        value: model.reactiveAmount,
                                        range: model.reactiveRange,
                                        tint: Color(hex: reactive.color) ?? tint)
                        }
                    }
                    Text(model.reactiveNote)
                        .font(DS.F.secondary)
                        .foregroundStyle(DS.Ink.faint)
                        .fixedSize(horizontal: false, vertical: true)
                        .padding(.horizontal, 2)
                }
            }
            Spacer(minLength: 0)
        }
        .padding(DS.S.inspectorPad)
        .frame(maxWidth: .infinity, alignment: .leading)
        .modifier(ScrollIfTall())
        .frame(maxHeight: .infinity, alignment: .top)
        .glassEffect(.regular, in: .rect(cornerRadius: DS.R.panel))
    }

    /// The pane you are editing, lit by what you are editing. A row of grey
    /// chrome would say which zone is selected; this shows what it is doing.
    private var zoneHeader: some View {
        let colours = (model.zone?.preview ?? []).compactMap { Color(hex: $0) }
        return HStack {
            Text(zoneNames[model.selectedZone] ?? model.selectedZone)
                .font(DS.F.bodyStrong)
            Spacer()
            Text(effectLabel(model.zone?.effect ?? ""))
                .font(DS.F.body)
                .foregroundStyle(.white.opacity(0.9))
        }
        .foregroundStyle(.white)
        .padding(.horizontal, 14)
        .frame(height: 34)
        .background {
            RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                .fill(colours.count > 1
                      ? AnyShapeStyle(LinearGradient(colors: colours, startPoint: .leading, endPoint: .trailing))
                      : AnyShapeStyle(colours.first ?? .gray))
        }
    }
}

/// The stage the panes float over, lit by whatever the deck is showing.
struct Stage: View {
    let bloom: Color

    var body: some View {
        ZStack {
            Color(white: 0.055)
            RadialGradient(colors: [Color(white: 0.17), Color(white: 0.055)],
                           center: .init(x: 0.5, y: 0.2), startRadius: 0, endRadius: 760)
            RadialGradient(colors: [bloom.opacity(0.30), bloom.opacity(0.12), .clear],
                           center: .init(x: 0.40, y: 0.62), startRadius: 0, endRadius: 560)
        }
        .ignoresSafeArea()
    }
}

/// Direction: a label, what it reads as in words, and a wheel.
///
/// The row is 78 tall in the handoff because the dial is 60 — and the wheel is
/// there so the direction can be *seen* rather than read as a number.
///
/// **The angle is not a compass bearing.** The device counts it from "to the
/// right", anticlockwise — `docs/PROTOCOL.md` §7 — so 0° is right, 90° is up,
/// 180° left, 270° down. Reading it as a bearing made the needle, the words and
/// the deck three different answers: the row said "left" at 270° while the
/// light ran top to bottom, and "up" at 0° while it ran to the left. The needle
/// points where the light goes, and the word underneath names the same thing.
struct DirectionRow: View {
    @Binding var angle: Double
    let free: Bool
    var tint: Color

    private let dial: Double = 60

    var body: some View {
        HStack {
            VStack(alignment: .leading, spacing: 1) {
                Text("Direction").font(DS.F.body).foregroundStyle(DS.Ink.primary)
                Text(free ? "\(Int(angle))° · \(compass)" : (isLeft ? "left" : "right"))
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.dim)
            }
            Spacer()
            ZStack {
                Circle().fill(.white.opacity(0.10))
                Circle().strokeBorder(.white.opacity(0.16), lineWidth: 0.5)
                Capsule()
                    .fill(tint)
                    .frame(width: 2, height: 22)
                    .offset(y: -11)
                    // The needle is drawn pointing up and turned clockwise, so
                    // an anticlockwise-from-right angle turns by 90 − angle.
                    .rotationEffect(.degrees(90 - angle))
                Circle().fill(.white).frame(width: 8, height: 8)
            }
            .frame(width: dial, height: dial)
            .contentShape(.circle)
            .gesture(
                DragGesture(minimumDistance: 0).onChanged { v in
                    let c = dial / 2
                    guard free else {
                        // A strip runs one way or the other; there is no angle
                        // between them to pick. 0 is right, 180 is left.
                        angle = v.location.x < c ? 180 : 0
                        return
                    }
                    // Anticlockwise from "to the right", and the screen's y
                    // grows downward, so it is the y term that is negated.
                    var deg = atan2(c - v.location.y, v.location.x - c) * 180 / .pi
                    if deg < 0 { deg += 360 }
                    angle = deg.rounded()
                }
            )
        }
        .frame(minHeight: 78)
    }

    private var isLeft: Bool { angle > 90 && angle <= 270 }

    /// The word for where the light goes, on the device's own zero.
    private var compass: String {
        switch Int(((angle + 22.5).truncatingRemainder(dividingBy: 360)) / 45) {
        case 0: return "right"
        case 1: return "up and right"
        case 2: return "up"
        case 3: return "up and left"
        case 4: return "left"
        case 5: return "down and left"
        case 6: return "down"
        default: return "down and right"
        }
    }
}


/// The inspector is as tall as the effect it is editing, so it scrolls when
/// that is taller than the window rather than pushing the action bar out.
private struct ScrollIfTall: ViewModifier {
    func body(content: Content) -> some View {
        ScrollView(.vertical, showsIndicators: false) { content }
            .scrollBounceBehavior(.basedOnSize)
    }
}
