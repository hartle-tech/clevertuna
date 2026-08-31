import SwiftUI

/// The four things a surface has to ask a person before it can act.
///
/// They are one type because they are one sheet: a title, a line saying what
/// will happen, one control, and the button that does it. Four separate sheets
/// would be four separate ideas of what this app's dialogs look like.
/// One of the connections the keyboard keeps a separate lighting scheme on.
///
/// There are four: the cable, and three Bluetooth channels the board prints on
/// F2, F3 and F4. **Which one is live is chosen on the keyboard**, with `fn` and
/// that key — there is no operation on the wire that switches it, so nothing
/// here can move between them or even tell which is current. A picker that
/// promised otherwise would be a picker that lies; this one names them and asks.
struct KeyboardSlot: Sendable {
    let id: String
    let name: String
    /// How a person puts the keyboard on it.
    let how: String

    static let all: [KeyboardSlot] = [
        KeyboardSlot(id: "usb", name: "Cable", how: "plug it in"),
        KeyboardSlot(id: "ble1", name: "Bluetooth 1", how: "fn + F2"),
        KeyboardSlot(id: "ble2", name: "Bluetooth 2", how: "fn + F3"),
        KeyboardSlot(id: "ble3", name: "Bluetooth 3", how: "fn + F4"),
    ]
}

enum Prompt: Identifiable {
    /// Keep the look you have, under a name.
    case save
    /// Give one of yours a better name.
    case rename(String)
    /// Remove one of yours. The only thing here that asks first.
    case remove(String)
    /// Copy one zone's light onto another.
    case copyZone
    /// Copy the whole lighting onto another connection's slot.
    case copySlot

    var id: String {
        switch self {
        case .save: return "save"
        case .rename(let name): return "rename:\(name)"
        case .remove(let name): return "remove:\(name)"
        case .copyZone: return "copy"
        case .copySlot: return "copy-slot"
        }
    }
}

/// One sheet, in the app's own material rather than the system's.
///
/// A `.alert` here would be the one place in Clevertuna that looks like every
/// other app, and it is the place where the app asks for something — which is
/// exactly where it should still look like itself.
struct PromptSheet: View {
    let prompt: Prompt
    @Environment(BuilderModel.self) private var model
    var onClose: () -> Void

    @State private var text = ""
    @State private var from = "keyboard"
    @State private var to = "touchpad"
    /// The scheme taken off the first slot, waiting for the rest.
    @State private var held: LookModel?
    @State private var zones: Set<String> = Set(zoneOrder)
    @State private var withReactive = true
    /// Which slots it is going to, and which are still to do.
    @State private var slots: Set<String> = []
    @State private var done: Set<String> = []
    @FocusState private var typing: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                Text(title).font(DS.F.pane).foregroundStyle(DS.Ink.primary)
                // Wraps rather than truncating: the sheet is 380 wide and
                // every one of these sentences is longer than that.
                Text(note)
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.dim)
                    .fixedSize(horizontal: false, vertical: true)
            }

            switch prompt {
            case .save, .rename:
                TextField("", text: $text)
                    .textFieldStyle(.plain)
                    .font(DS.F.body)
                    .foregroundStyle(DS.Ink.primary)
                    .focused($typing)
                    .onSubmit { if canConfirm { act() } }
                    .padding(.horizontal, 11)
                    .frame(height: 30)
                    .quietGlass(DS.R.control)

            case .copyZone:
                VStack(alignment: .leading, spacing: 8) {
                    DSSegmented(selection: $from, options: zoneOrder.map { ($0, zoneShort[$0] ?? $0) })
                    Image(systemName: "arrow.down")
                        .font(.system(size: 11))
                        .foregroundStyle(DS.Ink.dim)
                        .frame(maxWidth: .infinity)
                    DSSegmented(selection: $to, options: zoneOrder.map { ($0, zoneShort[$0] ?? $0) })
                }

            case .copySlot:
                copySlotBody

            case .remove:
                EmptyView()
            }

            HStack(spacing: 10) {
                Spacer(minLength: 0)
                DSButton(title: "Cancel") { onClose() }
                DSButton(title: confirmTitle, prominent: true) { act() }
                    .opacity(canConfirm ? 1 : 0.45)
                    .disabled(!canConfirm)
            }
        }
        .padding(20)
        .frame(width: 380)
        .background(Color(white: 0.11))
        .onAppear {
            switch prompt {
            case .save: text = suggestedName
            case .rename(let name): text = name
            default: break
            }
            typing = true
        }
    }

    // MARK: - Copying a whole scheme to the other slot

    /// The keyboard keeps its lighting per connection, and grants one
    /// connection at a time — so this cannot be one button. It takes a copy of
    /// what is on the slot you are on, waits while you move the keyboard to the
    /// other one, and writes it there.
    @ViewBuilder private var copySlotBody: some View {
        if held == nil {
            VStack(alignment: .leading, spacing: 10) {
                Text("ONTO WHICH SLOTS")
                    .font(DS.F.sectionHeader).kerning(0.33)
                    .foregroundStyle(DS.Ink.faint)
                ForEach(KeyboardSlot.all, id: \.id) { slot in
                    HStack {
                        VStack(alignment: .leading, spacing: 1) {
                            Text(slot.name)
                                .font(DS.F.body).foregroundStyle(DS.Ink.primary)
                            Text(slot.how)
                                .font(DS.F.secondary).foregroundStyle(DS.Ink.dim)
                        }
                        Spacer(minLength: 0)
                        DSSwitch(isOn: Binding(
                            get: { slots.contains(slot.id) },
                            set: { on in
                                if on { slots.insert(slot.id) } else { slots.remove(slot.id) }
                            }))
                    }
                    .frame(minHeight: 34)
                }
                Text("WHAT TO COPY")
                    .font(DS.F.sectionHeader).kerning(0.33)
                    .foregroundStyle(DS.Ink.faint)
                    .padding(.top, 4)
                ForEach(zoneOrder, id: \.self) { id in
                    HStack {
                        Text(zoneNames[id] ?? id)
                            .font(DS.F.body).foregroundStyle(DS.Ink.primary)
                        Spacer(minLength: 0)
                        DSSwitch(isOn: Binding(
                            get: { zones.contains(id) },
                            set: { on in
                                if on { zones.insert(id) } else { zones.remove(id) }
                            }))
                    }
                    .frame(height: 30)
                }
                HStack {
                    Text("Typing and touch colours")
                        .font(DS.F.body).foregroundStyle(DS.Ink.primary)
                    Spacer(minLength: 0)
                    DSSwitch(isOn: $withReactive)
                }
                .frame(height: 30)
            }
        } else {
            VStack(alignment: .leading, spacing: 10) {
                Label("Copy taken", systemImage: "checkmark.circle.fill")
                    .font(DS.F.body)
                    .foregroundStyle(Color(red: 0.188, green: 0.820, blue: 0.345))
                if let next = queue.first {
                    Text("Put the keyboard on **\(next.name)** — \(next.how) — then write.")
                        .font(DS.F.secondary)
                        .foregroundStyle(DS.Ink.primary)
                        .fixedSize(horizontal: false, vertical: true)
                }
                // Nothing here can tell which slot the keyboard is actually on:
                // the channel is chosen on the keyboard, not over the wire. So
                // this says which one it is waiting for and trusts the answer.
                Text(model.connected ? model.deviceWhere : "Looking for the keyboard…")
                    .font(DS.F.secondary)
                    .foregroundStyle(DS.Ink.faint)
                ForEach(KeyboardSlot.all.filter { slots.contains($0.id) }, id: \.id) { slot in
                    HStack(spacing: 8) {
                        Image(systemName: done.contains(slot.id)
                              ? "checkmark.circle.fill" : "circle")
                            .font(.system(size: 12))
                            .foregroundStyle(done.contains(slot.id)
                                             ? Color(red: 0.188, green: 0.820, blue: 0.345)
                                             : DS.Ink.faint)
                        Text(slot.name)
                            .font(DS.F.secondary)
                            .foregroundStyle(done.contains(slot.id) ? DS.Ink.dim : DS.Ink.primary)
                        Spacer(minLength: 0)
                    }
                }
            }
        }
    }

    /// The slots still to be written, in order.
    private var queue: [KeyboardSlot] {
        KeyboardSlot.all.filter { slots.contains($0.id) && !done.contains($0.id) }
    }

    // MARK: - What it says

    private var title: String {
        switch prompt {
        case .save: return "Save as a theme"
        case .rename: return "Rename"
        case .remove(let name): return "Remove \(name)?"
        case .copyZone: return "Copy one zone's light"
        case .copySlot: return held == nil ? "Apply to other slots" : "Switch the keyboard over"
        }
    }

    private var note: String {
        switch prompt {
        case .save:
            return "Kept on this Mac, alongside the ones we ship. The keyboard is not written to."
        case .rename:
            return "Letters, numbers, spaces, - and _."
        case .remove:
            return "Your saved themes are files on this Mac. This one goes for good."
        case .copyZone:
            return "Takes the colours, the effect and the light. Nothing is written until you apply."
        case .copySlot:
            return held == nil
                ? "The keyboard keeps its lighting per connection. This takes what is on the slot you are on now."
                : "Nothing has been written yet. Press Write when the keyboard is on the other slot."
        }
    }

    private var confirmTitle: String {
        switch prompt {
        case .save: return "Save"
        case .rename: return "Rename"
        case .remove: return "Remove"
        case .copyZone: return "Copy"
        case .copySlot:
            if held == nil { return "Take a copy" }
            return queue.count > 1 ? "Write, then next" : "Write to this slot"
        }
    }

    private var canConfirm: Bool {
        switch prompt {
        case .save, .rename: return !text.trimmingCharacters(in: .whitespaces).isEmpty
        case .copyZone: return from != to
        case .copySlot:
            return held == nil
                ? (!zones.isEmpty && !slots.isEmpty)
                : (model.connected && !queue.isEmpty)
        case .remove: return true
        }
    }

    /// A name that is already reasonable, so the common case is one keystroke.
    private var suggestedName: String {
        let f = DateFormatter()
        f.dateFormat = "yyyy-MM-dd HHmm"
        return "Look \(f.string(from: Date()))"
    }

    // MARK: - What it does

    private func act() {
        switch prompt {
        case .save:
            Task { await model.saveLookAsTheme(named: text) }
        case .rename(let name):
            Task { await model.renameTheme(name, to: text) }
        case .remove(let name):
            Task { await model.deleteTheme(name) }
        case .copyZone:
            Task { await model.copy(from: from, to: to) }
        case .copySlot:
            // The sheet stays open across the whole run: the keyboard is
            // switched over by hand between slots, and a dialog that closed
            // after the first write would leave the copy nowhere.
            guard let held else {
                held = model.slotCopy(zones: zones, reactive: withReactive)
                return
            }
            guard let target = queue.first else { break }
            done.insert(target.id)
            let last = queue.isEmpty
            Task { await model.writeToSlot(held, named: target.name) }
            if !last { return }
        }
        onClose()
    }
}
