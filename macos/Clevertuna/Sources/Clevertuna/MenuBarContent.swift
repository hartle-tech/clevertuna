import SwiftUI

/// The helper menu: the quick surface that dispenses with a window.
///
/// This is a *window*, not an `NSMenu`. A menu can only be a list of words, and
/// the handoff is not a list of words — it is a device tile, a brightness
/// slider you drag, six themes as tiles you can see, and two rows that act at
/// once. Every row acts on the keyboard straight away; only Builder and
/// Settings open anything, which is why they sit apart at the foot.
struct MenuBarContent: View {
    let model: BuilderModel

    var body: some View {
        VStack(spacing: 10) {
            device
            themes
            smart
            if let failure = model.failure { problem(failure) }
            windows
        }
        .padding(12)
        .frame(width: 376)
        .animation(DS.M.surface, value: model.failure)
        .animation(DS.M.hover, value: model.busy)
        .task { await model.loadMenu() }
    }

    /// What went wrong, where the thing that went wrong was pressed.
    ///
    /// This menu had no way at all to say a row had failed: `status` is only
    /// drawn in the builder, so `match-wallpaper` finding no wallpaper wrote a
    /// sentence into a field nobody could see and the row read as inert.
    private func problem(_ why: String) -> some View {
        HStack(alignment: .top, spacing: 9) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 13))
                .foregroundStyle(Color(red: 1.0, green: 0.84, blue: 0.25))
            Text(why)
                .font(DS.F.secondary)
                .foregroundStyle(DS.Ink.primary)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
        .padding(.horizontal, 12)
        .padding(.vertical, 10)
        .background {
            RoundedRectangle(cornerRadius: DS.R.groupTight, style: .continuous)
                .fill(Color(red: 1.0, green: 0.84, blue: 0.25).opacity(0.12))
                .overlay {
                    RoundedRectangle(cornerRadius: DS.R.groupTight, style: .continuous)
                        .strokeBorder(Color(red: 1.0, green: 0.84, blue: 0.25).opacity(0.28),
                                      lineWidth: 0.5)
                }
        }
        .transition(.opacity.combined(with: .move(edge: .top)))
    }

    // MARK: - The device, and the one control worth having without a window

    private var device: some View {
        VStack(spacing: 13) {
            HStack(spacing: 11) {
                RoundedRectangle(cornerRadius: 11, style: .continuous)
                    .fill(LinearGradient(colors: [.white.opacity(0.26), .white.opacity(0.10)],
                                         startPoint: .top, endPoint: .bottom))
                    .frame(width: 34, height: 34)
                    .overlay {
                        Image(systemName: "keyboard")
                            .font(.system(size: 17, weight: .regular))
                            .foregroundStyle(.white)
                    }
                VStack(alignment: .leading, spacing: 1) {
                    Text(model.deviceName).font(DS.F.bodyStrong).foregroundStyle(DS.Ink.primary)
                    // While the keyboard is being talked to, the tile says what
                    // is being said rather than where it is connected.
                    Text(model.busy?.what ?? model.deviceWhere)
                        .font(DS.F.secondary)
                        .foregroundStyle(DS.Ink.dim)
                        .lineLimit(1)
                }
                Spacer(minLength: 0)
                if let charge = model.battery, model.busy == nil {
                    HStack(spacing: 4) {
                        Image(systemName: batteryGlyph(charge))
                            .font(.system(size: 15))
                            .foregroundStyle(charge <= 15
                                             ? Color(red: 1.0, green: 0.42, blue: 0.35)
                                             : DS.Ink.dim)
                        Text("\(charge)%")
                            .font(DS.F.secondary.monospacedDigit())
                            .foregroundStyle(DS.Ink.dim)
                    }
                    .padding(.trailing, 2)
                }
                if model.busy != nil {
                    ProgressView()
                        .controlSize(.small)
                        .scaleEffect(0.7)
                        .frame(width: 12, height: 12)
                } else {
                    Circle()
                        .fill(model.connected ? Color(red: 0.188, green: 0.820, blue: 0.345) : .orange)
                        .frame(width: 8, height: 8)
                        .shadow(color: (model.connected ? Color.green : .orange).opacity(0.8), radius: 5)
                }
            }

            HStack(spacing: 10) {
                Image(systemName: "sun.max")
                    .font(.system(size: 15))
                    .foregroundStyle(DS.Ink.primary.opacity(0.7))
                BrightnessBar(value: model.menuBrightness,
                              writing: model.busy?.source == "brightness") { v in
                    Task { await model.setBrightness(v) }
                }
            }
        }
        .padding(.horizontal, 14)
        .padding(.vertical, 13)
        .quietGlass(DS.R.group)
    }

    /// The battery glyph macOS uses, at the nearest quarter.
    private func batteryGlyph(_ percent: Int) -> String {
        switch percent {
        case ..<13: return "battery.0percent"
        case ..<38: return "battery.25percent"
        case ..<63: return "battery.50percent"
        case ..<88: return "battery.75percent"
        default: return "battery.100percent"
        }
    }

    // MARK: - Six themes, as tiles

    private var themes: some View {
        VStack(spacing: 10) {
            HStack(alignment: .firstTextBaseline) {
                Text("Themes")
                    .font(DS.F.sectionHeader)
                    .kerning(0.33)
                    .foregroundStyle(DS.Ink.dim)
                Spacer()
                Button("Show All") { Windows.shared.show(.themes) }
                    .buttonStyle(.plain)
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.link)
            }
            LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 8), count: 3),
                      spacing: 8) {
                ForEach(model.quickThemes) { theme in
                    DSTile(name: theme.name,
                           key: model.key(for: theme.id),
                           colours: theme.colours.compactMap { Color(hex: $0) },
                           selected: model.currentTheme == theme.id,
                           working: model.busy?.source == theme.id,
                           waiting: model.busy != nil && model.busy?.source != theme.id) {
                        Task { await model.applyTheme(theme.id) }
                    }
                }
            }
        }
        .padding(12)
        .quietGlass(DS.R.group)
    }

    // MARK: - The two that are worked out rather than chosen

    private var smart: some View {
        VStack(spacing: 0) {
            SmartRow(title: "Random",
                     note: "A different theme, right now",
                     key: model.key(for: "random"),
                     // The gradient itself, not a circle drawn on it: every
                     // other icon in this menu is the rounded square its slot
                     // clips, and a disc inside one reads as a different kind
                     // of thing.
                     icon: AnyView(AngularGradient(
                        colors: [Color(hex: "#FF375F")!, Color(hex: "#FF9F0A")!,
                                 Color(hex: "#30D158")!, Color(hex: "#0A84FF")!,
                                 Color(hex: "#BF5AF2")!, Color(hex: "#FF375F")!],
                        center: .center)),
                     working: model.busy?.source == "random",
                     waiting: model.busy != nil && model.busy?.source != "random") {
                Task { await model.rollAndApply() }
            }
            Rectangle().fill(.white.opacity(0.12)).frame(height: 0.5).padding(.horizontal, 8)
            SmartRow(title: "Wallpaper",
                     note: "Follows your desktop picture",
                     key: model.key(for: "wallpaper"),
                     icon: AnyView(LinearGradient(
                        colors: [Color(hex: "#FFAF00")!, Color(hex: "#D22900")!,
                                 Color(hex: "#DFE600")!, Color(hex: "#61B723")!],
                        startPoint: .topLeading, endPoint: .bottomTrailing)),
                     working: model.busy?.source == "wallpaper",
                     waiting: model.busy != nil && model.busy?.source != "wallpaper") {
                Task { await model.matchWallpaper() }
            }
        }
        .padding(6)
        .quietGlass(DS.R.group)
    }

    // MARK: - The two things that open a window

    private var windows: some View {
        HStack(spacing: 10) {
            MenuWindowButton(title: "Builder", systemImage: "slider.horizontal.3") {
                Windows.shared.show(.builder)
            }
            MenuWindowButton(title: "Settings", systemImage: "gearshape") {
                Windows.shared.show(.settings)
            }
        }
    }
}

/// The brightness bar: 28 tall, radius 14, filled in white — the one control
/// worth having without opening anything.
private struct BrightnessBar: View {
    let value: Double
    /// The keyboard is still taking the last drag.
    var writing = false
    let onChange: (Double) -> Void
    @State private var live: Double?

    var body: some View {
        GeometryReader { geo in
            let shown = live ?? value
            ZStack(alignment: .leading) {
                Capsule().fill(.white.opacity(0.14))
                Capsule()
                    .fill(LinearGradient(colors: [.white, Color(red: 0.902, green: 0.902, blue: 0.918)],
                                         startPoint: .top, endPoint: .bottom))
                    .frame(width: max(14, geo.size.width * shown / 100))
            }
            .frame(height: 28)
            .contentShape(.capsule)
            .gesture(
                DragGesture(minimumDistance: 0)
                    .onChanged { g in
                        live = min(max(g.location.x / geo.size.width * 100, 0), 100)
                    }
                    .onEnded { _ in
                        if let live { onChange(live) }
                    }
            )
            // Held until the device has actually answered, rather than for a
            // guessed six-tenths of a second — a timer that expired mid-write
            // snapped the bar back to the old value and then forward again.
            .onChange(of: writing) { was, now in
                if was && !now { live = nil }
            }
        }
        .frame(height: 28)
        .shimmering(writing, radius: 14)
    }
}

/// A row that acts at once, with the key that does the same without opening
/// anything.
private struct SmartRow: View {
    let title: String
    let note: String
    let key: String
    let icon: AnyView
    var working = false
    var waiting = false
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 11) {
                icon
                    .frame(width: 32, height: 32)
                    .clipShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
                    .overlay {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .strokeBorder(.white.opacity(0.4), lineWidth: 0.5)
                    }
                VStack(alignment: .leading, spacing: 1) {
                    Text(title).font(DS.F.bodyMedium).foregroundStyle(DS.Ink.primary)
                    Text(note).font(DS.F.secondary).foregroundStyle(DS.Ink.dim)
                }
                Spacer(minLength: 0)
                if working {
                    ProgressView().controlSize(.small).scaleEffect(0.7)
                } else {
                    Text(key).font(DS.F.secondary).foregroundStyle(DS.Ink.dim)
                }
            }
            .padding(8)
            .background {
                RoundedRectangle(cornerRadius: 12, style: .continuous)
                    .fill(.white.opacity(hovering ? 0.10 : 0))
            }
        }
        .buttonStyle(.plain)
        .shimmering(working, radius: 12)
        .focusEffectDisabled()
        .disabled(waiting)
        .opacity(waiting ? 0.55 : 1)
        .onHover { hovering = $0 && !waiting }
    }
}

/// Builder and Settings: the only two rows that open anything.
private struct MenuWindowButton: View {
    let title: String
    let systemImage: String
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 9) {
                Image(systemName: systemImage).font(.system(size: 15))
                Text(title).font(DS.F.bodyMedium)
                Spacer(minLength: 0)
            }
            .foregroundStyle(DS.Ink.primary)
            .padding(.horizontal, 13)
            .padding(.vertical, 11)
            .background {
                RoundedRectangle(cornerRadius: 16, style: .continuous)
                    .fill(.white.opacity(hovering ? 0.16 : 0.09))
                    .overlay {
                        RoundedRectangle(cornerRadius: 16, style: .continuous)
                            .strokeBorder(.white.opacity(0.14), lineWidth: 0.5)
                    }
            }
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .animation(.easeOut(duration: 0.12), value: hovering)
    }
}
