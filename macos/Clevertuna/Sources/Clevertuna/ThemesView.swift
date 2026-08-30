import SwiftUI

/// Themes.
///
/// The selected theme *is* the window's backdrop, so every pane of glass on top
/// of it is tinted by the light it describes. The card carries only what you
/// cannot see by looking — where it came from, what it does, and the key that
/// applies it from anywhere.
///
/// The room the controls leave is given to the theme itself: one we ship knows
/// what it is, so magma burns and the cyan ones move like water, while a theme
/// you built gets a drawn lattice rather than a picture of something it is not.
struct ThemesView: View {
    @Environment(BuilderModel.self) private var model
    @State private var selected: String?
    @State private var search = ""
    @State private var prompt: Prompt?

    private var chosen: ThemeSummary? {
        model.allThemes.first { $0.id == (selected ?? model.currentTheme) } ?? model.allThemes.first
    }

    /// Whether the theme on show is one of yours, which is the only kind that
    /// can be renamed or removed — ours are compiled into the binary.
    private var mine: Bool { chosen?.group == "Yours" }

    var body: some View {
        ZStack {
            backdrop
            HStack(spacing: 0) {
                sourceList
                    .frame(width: 258)
                    .padding(12)
                detail
                    .padding(.vertical, 12)
                    .padding(.trailing, 12)
            }
        }
        .frame(minWidth: 860, minHeight: 580)
        .task { await model.loadMenu() }
        .sheet(item: $prompt) { p in
            PromptSheet(prompt: p) { prompt = nil }
                .environment(model)
        }
    }

    // MARK: - The theme, as the window's own light

    private var backdrop: some View {
        let colours = (chosen?.colours ?? []).compactMap { Color(hex: $0) }
        return ZStack {
            Color(white: 0.04)
            if colours.count > 1 {
                LinearGradient(colors: colours, startPoint: .topLeading, endPoint: .bottomTrailing)
            } else {
                (colours.first ?? .gray)
            }
            RadialGradient(colors: [.black.opacity(0.10), .black.opacity(0.52), .black.opacity(0.74)],
                           center: .init(x: 0.3, y: 0), startRadius: 0, endRadius: 900)
        }
        .animation(.easeInOut(duration: 0.35), value: chosen?.id)
        .ignoresSafeArea()
    }

    // MARK: - The list

    private var sourceList: some View {
        VStack(alignment: .leading, spacing: 0) {
            Spacer().frame(height: 34)      // the traffic lights sit on this glass

            HStack(spacing: 7) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 12))
                    .foregroundStyle(DS.Ink.dim)
                TextField("Search", text: $search)
                    .textFieldStyle(.plain)
                    .font(DS.F.body)
                    .foregroundStyle(DS.Ink.primary)
            }
            .padding(.horizontal, 11)
            .frame(height: 28)
            .quietGlass(14)
            .padding(.horizontal, 2)
            .padding(.bottom, 4)

            // Scrolled to the theme on show. Yours are at the foot of a list
            // longer than the window, so opening this on one of your own used
            // to select a row nobody could see.
            ScrollViewReader { list in
            ScrollView(.vertical, showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    ForEach(groups, id: \.self) { group in
                        Text(group)
                            .font(DS.F.sectionHeader)
                            .kerning(0.33)
                            .foregroundStyle(DS.Ink.faint)
                            .padding(.horizontal, 10)
                            .padding(.top, 12)
                            .padding(.bottom, 5)
                        ForEach(shown.filter { $0.group == group }) { theme in
                            ThemeRow(theme: theme,
                                     key: model.key(for: theme.id),
                                     selected: theme.id == chosen?.id) {
                                selected = theme.id
                            } onApply: {
                                Task { await model.applyTheme(theme.id) }
                            }
                            .id(theme.id)
                        }
                    }
                }
                .padding(.horizontal, 2)
            }
            .onChange(of: chosen?.id, initial: true) { _, id in
                guard let id else { return }
                list.scrollTo(id, anchor: .center)
            }
            }

            HStack(spacing: 4) {
                CircleGlyph(systemImage: "plus") { prompt = .save }
                // Dimmed rather than hidden on one of ours: a control that
                // disappears reads as a missing feature, and the reason this
                // one cannot act is worth showing.
                CircleGlyph(systemImage: "minus", enabled: mine) {
                    if let id = chosen?.id { prompt = .remove(id) }
                }
                Spacer()
            }
            .padding(.top, 8)
            .padding(.horizontal, 6)
            .overlay(alignment: .top) {
                Rectangle().fill(.white.opacity(0.12)).frame(height: 0.5)
            }
        }
        .padding(.horizontal, 8)
        .padding(.bottom, 10)
        .glassEffect(.regular, in: .rect(cornerRadius: DS.R.panel))
    }

    private var shown: [ThemeSummary] {
        search.isEmpty ? model.allThemes
            : model.allThemes.filter { $0.name.localizedCaseInsensitiveContains(search) }
    }

    private var groups: [String] {
        var seen: [String] = []
        for t in shown where !seen.contains(t.group) { seen.append(t.group) }
        return seen
    }

    // MARK: - The theme itself

    private var detail: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(alignment: .top, spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(chosen?.name ?? "—")
                        .font(DS.F.subject)
                        .kerning(-0.6)
                        .foregroundStyle(.white)
                        .shadow(color: .black.opacity(0.45), radius: 7, y: 2)
                    Text(chosen?.note ?? "")
                        .font(DS.F.body)
                        .foregroundStyle(.white.opacity(0.80))
                        .shadow(color: .black.opacity(0.4), radius: 4, y: 1)
                }
                Spacer()
                // Dimmed, not hidden: a button that vanishes on half the
                // themes reads as a layout bug, and the reason this one cannot
                // act — the theme is one of ours — is worth showing.
                DSButton(title: "Rename…") {
                    if let id = chosen?.id { prompt = .rename(id) }
                }
                .opacity(mine ? 1 : 0.4)
                .disabled(!mine)
                .help(mine ? "" : "The themes we ship keep their names")
                DSButton(title: "Apply", prominent: true) {
                    if let id = chosen?.id { Task { await model.applyTheme(id) } }
                }
            }

            // The room the controls leave, given to the keyboard wearing it.
            //
            // A row of swatches asks a person to imagine what a theme does; the
            // deck shows it, moving, at the speed and in the direction the
            // keyboard will run it. Nothing is written to reach this — the
            // core describes a theme without applying it.
            stage
                .frame(maxWidth: .infinity, maxHeight: .infinity)

            card
        }
    }

    /// The keyboard, wearing the theme on show.
    ///
    /// Ambience is what fills the room until the look arrives — a beat, not a
    /// substitute — so the pane is never a hole while the core is asked.
    private var stage: some View {
        ZStack {
            if let look = model.previewLook, model.previewOf == chosen?.id {
                DeckView(look: look,
                         selected: "",
                         animated: look.isAnimated,
                         clock: { model.elapsed(at: $0) },
                         finish: model.finish,
                         onSelect: { _ in })
                    .padding(18)
                    .allowsHitTesting(false)   // a picker, not an editor
                    .transition(.opacity)
            } else {
                Ambience(kind: model.ambience(for: chosen))
            }
        }
        .animation(.easeInOut(duration: 0.25), value: model.previewLook == nil)
        .task(id: chosen?.id) {
            if let id = chosen?.id { await model.preview(theme: id) }
        }
    }

    private var card: some View {
        VStack(spacing: 0) {
            HStack(spacing: 8) {
                ForEach(Array((chosen?.colours ?? []).enumerated()), id: \.offset) { _, hex in
                    VStack(spacing: 6) {
                        RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                            .fill(Color(hex: hex) ?? .black)
                            .frame(height: 26)
                            .overlay {
                                RoundedRectangle(cornerRadius: DS.R.popup, style: .continuous)
                                    .strokeBorder(.white.opacity(0.4), lineWidth: 0.5)
                            }
                        Text(hex)
                            .font(DS.F.secondary.monospacedDigit())
                            .foregroundStyle(DS.Ink.dim)
                    }
                }
            }
            .padding(.bottom, 14)

            MetaRow(label: "Where from",
                    value: chosen.map { $0.group == "Yours" ? "Yours · saved on this Mac" : "Built in · \($0.group)" } ?? "")
            // Ours describe what they do; yours are described by the line
            // under the title already, and saying it twice is not a card.
            if !mine {
                MetaRow(label: "Effect", value: chosen?.note ?? "")
            }
            MetaRow(label: "Shortcut", value: nil) {
                if let id = chosen?.id, !model.key(for: id).isEmpty {
                    HStack(spacing: 12) {
                        Text(model.key(for: id))
                            .font(DS.F.body.monospacedDigit())
                            .padding(.leading, 12)
                            .padding(.trailing, 8)
                            .frame(height: 26)
                            .quietGlass(DS.R.popup)
                        Text("Works from any app")
                            .font(DS.F.secondary)
                            .foregroundStyle(DS.Ink.dim)
                    }
                } else {
                    Text("No key yet").font(DS.F.body).foregroundStyle(DS.Ink.dim)
                }
            }
        }
        .padding(.horizontal, 18)
        .padding(.vertical, 14)
        .glassEffect(.regular, in: .rect(cornerRadius: DS.R.panel))
    }
}

/// One theme in the list: a swatch, its name, and the key that applies it.
private struct ThemeRow: View {
    let theme: ThemeSummary
    let key: String
    let selected: Bool
    let onSelect: () -> Void
    let onApply: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: onSelect) {
            HStack(spacing: 10) {
                RoundedRectangle(cornerRadius: 10, style: .continuous)
                    .fill(swatch)
                    .frame(width: 30, height: 20)
                    .overlay {
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .strokeBorder(.white.opacity(0.35), lineWidth: 0.5)
                    }
                Text(theme.name)
                    .font(selected ? DS.F.bodyStrong : DS.F.body)
                    .foregroundStyle(DS.Ink.primary)
                    .lineLimit(1)
                Spacer(minLength: 0)
                Text(key).font(DS.F.secondary).foregroundStyle(DS.Ink.dim)
            }
            .padding(.horizontal, 10)
            .frame(height: DS.S.rowHeight)
            .background {
                RoundedRectangle(cornerRadius: 17, style: .continuous)
                    .fill(.white.opacity(selected ? 0.24 : hovering ? 0.10 : 0))
                    .overlay {
                        RoundedRectangle(cornerRadius: 17, style: .continuous)
                            .strokeBorder(.white.opacity(selected ? 0.22 : 0), lineWidth: 1)
                    }
            }
        }
        .buttonStyle(.plain)
        .onHover { hovering = $0 }
        .simultaneousGesture(TapGesture(count: 2).onEnded { onApply() })
        .padding(.bottom, 2)
    }

    private var swatch: AnyShapeStyle {
        let colours = theme.colours.compactMap { Color(hex: $0) }
        return colours.count > 1
            ? AnyShapeStyle(LinearGradient(colors: colours, startPoint: .leading, endPoint: .trailing))
            : AnyShapeStyle(colours.first ?? .gray)
    }
}

private struct MetaRow<Content: View>: View {
    let label: String
    var value: String?
    @ViewBuilder var content: Content

    init(label: String, value: String?, @ViewBuilder content: () -> Content = { EmptyView() }) {
        self.label = label
        self.value = value
        self.content = content()
    }

    var body: some View {
        HStack(alignment: .firstTextBaseline, spacing: 14) {
            Text(label)
                .font(DS.F.body)
                .foregroundStyle(DS.Ink.dim)
                .frame(width: 92, alignment: .trailing)
            if let value {
                Text(value).font(DS.F.body).foregroundStyle(DS.Ink.primary)
            }
            content
            Spacer(minLength: 0)
        }
        .padding(.vertical, 10)
        .overlay(alignment: .bottom) {
            Rectangle().fill(.white.opacity(0.12)).frame(height: 0.5)
        }
    }
}

private struct CircleGlyph: View {
    let systemImage: String
    var enabled = true
    let action: () -> Void
    @State private var hovering = false

    var body: some View {
        Button(action: action) {
            Image(systemName: systemImage)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(DS.Ink.primary)
                .frame(width: 26, height: 26)
                .background {
                    Circle().fill(.white.opacity(hovering && enabled ? 0.20 : 0.10))
                }
        }
        .buttonStyle(.plain)
        .opacity(enabled ? 1 : 0.4)
        .disabled(!enabled)
        .onHover { hovering = $0 }
    }
}
