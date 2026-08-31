import SwiftUI

/// Settings: everything that is not a theme.
///
/// Read against a pale surface — the same material, the same capsules, the
/// light half of the material sheet. What this keyboard cannot do is shown and
/// disabled rather than hidden, because a missing row reads as a missing
/// feature and a dimmed one reads as the truth.
struct SettingsView: View {
    @Environment(BuilderModel.self) private var model
    /// Which pane opens. A launch argument so the harness can photograph one
    /// that is not the default — the function row lives under Keys, and a
    /// screenshot of Touch is not a picture of it.
    @State private var pane = CommandLine.arguments.firstIndex(of: "--pane")
        .flatMap { i in i + 1 < CommandLine.arguments.count ? CommandLine.arguments[i + 1] : nil }
        ?? "Touch"

    private let panes: [(id: String, tint: Color)] = [
        ("Lighting", Color(red: 1.0, green: 0.584, blue: 0.0)),
        ("Touch", Color(red: 0.157, green: 0.749, blue: 0.298)),
        ("Keys", Color(red: 0.369, green: 0.361, blue: 0.902)),
        ("Power", Color(red: 1.0, green: 0.216, blue: 0.373)),
        ("Device", Color(red: 0.557, green: 0.557, blue: 0.576)),
    ]

    var body: some View {
        ZStack {
            LightStage()
            HStack(spacing: 0) {
                sidebar.frame(width: 226).padding(12)
                content.padding(.horizontal, 26)
            }
        }
        .frame(minWidth: 860, minHeight: 600)
        .task { await model.loadSettings() }
    }

    private var sidebar: some View {
        VStack(alignment: .leading, spacing: 3) {
            Spacer().frame(height: 36)      // the traffic lights sit on this glass
            ForEach(panes, id: \.id) { p in
                SidebarRow(title: p.id, tint: p.tint, selected: pane == p.id) { pane = p.id }
            }
            Spacer()
            VStack(alignment: .leading, spacing: 2) {
                Text(model.deviceName + (model.firmware.isEmpty ? "" : " · firmware \(model.firmware)"))
                Text(model.deviceWhere)
            }
            .font(DS.F.secondary)
            .foregroundStyle(DS.Ink.onLightDim)
            .padding(.horizontal, 10)
        }
        .padding(.horizontal, 10)
        .padding(.bottom, 12)
        .background {
            RoundedRectangle(cornerRadius: 20, style: .continuous)
                .fill(LinearGradient(colors: [.white.opacity(0.92), .white.opacity(0.74)],
                                     startPoint: .top, endPoint: .bottom))
                .shadow(color: Color(red: 0.12, green: 0.10, blue: 0.24).opacity(0.12), radius: 8, y: 4)
        }
    }

    private var content: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(pane)
                .font(DS.F.pane)
                .foregroundStyle(DS.Ink.onLight)
                .frame(height: 60, alignment: .center)
                .frame(maxWidth: .infinity, alignment: .leading)

            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 20) {
                    if pane == "Lighting" { appearance }
                    if pane == "Keys" { functionRow }
                    let groups = model.settingGroups(in: pane)
                    if groups.isEmpty {
                        SettingsNotice(state: model.settingsState, pane: pane) {
                            Task { await model.loadSettings() }
                        }
                    }
                    ForEach(groups, id: \.name) { group in
                        if !group.name.isEmpty && group.name != pane {
                            Text(group.name.uppercased())
                                .font(DS.F.sectionHeader)
                                .kerning(0.4)
                                .foregroundStyle(DS.Ink.onLightDim)
                                .padding(.horizontal, 6)
                        }
                        VStack(spacing: 0) {
                            ForEach(Array(group.items.enumerated()), id: \.element.key) { i, item in
                                if i > 0 {
                                    Rectangle().fill(.black.opacity(0.07)).frame(height: 0.5)
                                }
                                SettingRow(item: item) { value in
                                    Task { await model.setSetting(item.key, to: value) }
                                }
                            }
                        }
                        .background {
                            RoundedRectangle(cornerRadius: DS.R.group, style: .continuous)
                                .fill(LinearGradient(colors: [.white.opacity(0.92), .white.opacity(0.74)],
                                                     startPoint: .top, endPoint: .bottom))
                                .shadow(color: Color(red: 0.12, green: 0.10, blue: 0.24).opacity(0.10),
                                        radius: 8, y: 3)
                        }
                    }
                }
                .padding(.bottom, 24)
            }
        }
    }

    /// What each function key sends.
    ///
    /// The list of what a key can do is the core's, not this window's: a second
    /// table of HID usages here would be a second answer to a question
    /// `keymap.rs` already answers, and the two would drift the first time one
    /// of them gained an entry.
    @ViewBuilder private var functionRow: some View {
        if let map = model.keyMap, !map.keys.isEmpty {
            VStack(alignment: .leading, spacing: 8) {
                Text("FUNCTION ROW")
                    .font(DS.F.sectionHeader).kerning(0.4)
                    .foregroundStyle(DS.Ink.onLightDim)
                    .padding(.horizontal, 6)
                VStack(spacing: 0) {
                    ForEach(Array(map.keys.enumerated()), id: \.element.key) { i, row in
                        if i > 0 {
                            Rectangle().fill(.black.opacity(0.07)).frame(height: 0.5)
                        }
                        HStack(spacing: 12) {
                            Text(row.name)
                                .font(DS.F.bodyStrong)
                                .foregroundStyle(DS.Ink.onLight)
                                .frame(width: 44, alignment: .leading)
                            Text(row.label)
                                .font(DS.F.secondary)
                                .foregroundStyle(DS.Ink.onLightDim)
                                .lineLimit(1)
                            Spacer(minLength: 0)
                            DSPopUpButton(value: chosen(row, in: map),
                                          options: map.actions.map(\.name),
                                          light: true) { picked in
                                guard let a = map.actions.first(where: { $0.name == picked }) else { return }
                                Task { await model.setKey(row.key, to: a.id, named: a.name) }
                            }
                        }
                        .padding(.horizontal, 16)
                        .frame(minHeight: 48)
                    }
                }
                .background {
                    RoundedRectangle(cornerRadius: DS.R.group, style: .continuous)
                        .fill(LinearGradient(colors: [.white.opacity(0.92), .white.opacity(0.74)],
                                             startPoint: .top, endPoint: .bottom))
                        .shadow(color: Color(red: 0.12, green: 0.10, blue: 0.24).opacity(0.10),
                                radius: 8, y: 3)
                }
                Text("Written straight to the keyboard and read back to check. "
                     + "The vendor's app is still the one for firmware.")
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.onLightDim)
                    .padding(.horizontal, 6)
            }
        }
    }

    /// A binding the device holds that has no name here still shows what it is,
    /// rather than snapping the control to the first entry in the list.
    private func chosen(_ row: KeyMap.Row, in map: KeyMap) -> String {
        if let id = row.action, let a = map.actions.first(where: { $0.id == id }) { return a.name }
        return row.label
    }

    /// What this keyboard is made of — ours to remember, not the device's to
    /// say. Nothing on the wire distinguishes a black board from a white one,
    /// and the preview is a different picture for each.
    private var appearance: some View {
        @Bindable var model = model
        return VStack(alignment: .leading, spacing: 8) {
            Text("THIS KEYBOARD")
                .font(DS.F.sectionHeader)
                .kerning(0.4)
                .foregroundStyle(DS.Ink.onLightDim)
                .padding(.horizontal, 6)
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                    Text("Keycaps").font(DS.F.body).foregroundStyle(DS.Ink.onLight)
                    Text("So the preview draws your board, not a black one")
                        .font(DS.F.secondary).foregroundStyle(DS.Ink.onLightDim)
                }
                Spacer(minLength: 0)
                DSPopUpButton(value: model.finish.label,
                              options: KeyboardFinish.allCases.map(\.label),
                              light: true) { picked in
                    if let found = KeyboardFinish.allCases.first(where: { $0.label == picked }) {
                        model.finish = found
                    }
                }
            }
            .padding(.horizontal, 16)
            .frame(minHeight: 52)
            .background {
                RoundedRectangle(cornerRadius: DS.R.group, style: .continuous)
                    .fill(LinearGradient(colors: [.white.opacity(0.92), .white.opacity(0.74)],
                                         startPoint: .top, endPoint: .bottom))
                    .shadow(color: Color(red: 0.12, green: 0.10, blue: 0.24).opacity(0.10),
                            radius: 8, y: 3)
            }
        }
    }
}

/// The pale ground the light material is read against.
private struct LightStage: View {
    var body: some View {
        ZStack {
            LinearGradient(colors: [Color(red: 0.929, green: 0.922, blue: 0.953),
                                    Color(red: 0.906, green: 0.918, blue: 0.933),
                                    Color(red: 0.914, green: 0.898, blue: 0.886)],
                           startPoint: .topLeading, endPoint: .bottomTrailing)
            RadialGradient(colors: [Color(red: 0.863, green: 0.839, blue: 0.941).opacity(0.9), .clear],
                           center: .init(x: 0.2, y: 0.1), startRadius: 0, endRadius: 520)
        }
        .ignoresSafeArea()
    }
}

private struct SidebarRow: View {
    let title: String
    let tint: Color
    let selected: Bool
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 7, style: .continuous)
                    .fill(tint)
                    .frame(width: 22, height: 22)
                    .overlay {
                        RoundedRectangle(cornerRadius: 3, style: .continuous)
                            .fill(.white.opacity(0.95))
                            .frame(width: 11, height: 11)
                    }
                Text(title)
                    .font(selected ? DS.F.bodyStrong : DS.F.body)
                    .foregroundStyle(selected ? .white : DS.Ink.onLight)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .frame(height: DS.S.rowHeight)
            .background {
                RoundedRectangle(cornerRadius: 17, style: .continuous)
                    .fill(selected
                          ? AnyShapeStyle(LinearGradient(
                                colors: [Color(red: 0.243, green: 0.608, blue: 1.0),
                                         Color(red: 0.039, green: 0.435, blue: 0.910)],
                                startPoint: .top, endPoint: .bottom))
                          : AnyShapeStyle(Color.black.opacity(hovering ? 0.05 : 0)))
                    .shadow(color: selected ? Color(red: 0.039, green: 0.435, blue: 0.910).opacity(0.32) : .clear,
                            radius: 4, y: 2)
            }
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .animation(.easeOut(duration: 0.12), value: hovering)
    }
}

/// One setting: its name, what it is for, and the control it takes.
private struct SettingRow: View {
    let item: DeviceSetting
    let onChange: (String) -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 1) {
                Text(item.label)
                    .font(DS.F.body)
                    .foregroundStyle(item.available ? DS.Ink.onLight : DS.Ink.onLightDim.opacity(0.7))
                if let note = item.note {
                    Text(note).font(DS.F.secondary).foregroundStyle(DS.Ink.onLightDim)
                }
            }
            Spacer(minLength: 0)
            control
        }
        .padding(.horizontal, 16)
        .frame(minHeight: item.note == nil ? 40 : 48)
    }

    @ViewBuilder private var control: some View {
        switch item.kind {
        case .toggle:
            LightSwitch(isOn: item.value == "on", enabled: item.available) { on in
                onChange(on ? "on" : "off")
            }
        case .choice(let options):
            DSPopUpButton(value: item.value, options: options, light: true,
                          enabled: item.available, onChange: onChange)
        }
    }
}

/// The switch, on the light material: 40 × 24, a 20 knob.
private struct LightSwitch: View {
    let isOn: Bool
    let enabled: Bool
    let onChange: (Bool) -> Void

    var body: some View {
        Capsule()
            .fill(isOn
                  ? AnyShapeStyle(LinearGradient(colors: [Color(red: 0.204, green: 0.843, blue: 0.357),
                                                          Color(red: 0.157, green: 0.749, blue: 0.298)],
                                                 startPoint: .top, endPoint: .bottom))
                  : AnyShapeStyle(Color.black.opacity(enabled ? 0.16 : 0.08)))
            .frame(width: 40, height: 24)
            .overlay(alignment: isOn ? .trailing : .leading) {
                Circle()
                    .fill(LinearGradient(colors: [.white, Color(red: 0.949, green: 0.949, blue: 0.961)],
                                         startPoint: .top, endPoint: .bottom))
                    .frame(width: 20, height: 20)
                    .shadow(color: .black.opacity(0.28), radius: 1.5, y: 1)
                    .padding(2)
            }
            .opacity(enabled ? 1 : 0.55)
            .contentShape(.capsule)
            .onTapGesture {
                guard enabled else { return }
                withAnimation(.snappy(duration: 0.18)) { onChange(!isOn) }
            }
    }
}


/// What to say when a pane has nothing in it, which is never just "nothing".
private struct SettingsNotice: View {
    let state: LoadState
    let pane: String
    let retry: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            switch state {
            case .loading:
                HStack(spacing: 10) {
                    ProgressView().controlSize(.small)
                    Text("Reading the keyboard…")
                        .font(DS.F.body)
                        .foregroundStyle(DS.Ink.onLightDim)
                }
            case .ready:
                // The read worked; this pane simply has nothing on this
                // keyboard, which is a fact worth stating.
                Text("This keyboard has nothing to set under \(pane).")
                    .font(DS.F.body)
                    .foregroundStyle(DS.Ink.onLightDim)
            case .failed(let why):
                Text("Could not read the keyboard's settings.")
                    .font(DS.F.bodyStrong)
                    .foregroundStyle(DS.Ink.onLight)
                Text(why)
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.onLightDim)
                    .fixedSize(horizontal: false, vertical: true)
                Button("Try again", action: retry)
                    .buttonStyle(PressableButtonStyle())
                    .font(DS.F.body)
                    .foregroundStyle(Color(red: 0.039, green: 0.435, blue: 0.910))
            }
        }
        .padding(16)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background {
            RoundedRectangle(cornerRadius: DS.R.group, style: .continuous)
                .fill(.white.opacity(0.86))
        }
    }
}
