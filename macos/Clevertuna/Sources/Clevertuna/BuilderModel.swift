import SwiftUI
import Observation

/// What the builder is editing, and the only thing that talks to the device.
///
/// Edits change the model and the preview immediately; the keyboard is written
/// only when Apply is pressed. Nothing here writes on a slider drag — the
/// promise printed next to the button is a promise the code keeps.
/// What a surface is doing, so it can say so rather than showing a blank.
enum LoadState: Equatable {
    case loading
    case ready
    case failed(String)
}

@MainActor
@Observable
final class BuilderModel {
    let device: any DeviceService

    var look: LookModel?
    /// The look a theme in the picker would put on the keyboard, and which
    /// theme it belongs to — so a reply that arrives after you have moved on
    /// does not paint the deck with the previous theme.
    var previewLook: LookModel?
    var previewOf: String?
    var selectedZone = "keyboard"
    var status = "Nothing is written until you apply"

    var showThemes = false
    var showSettings = false

    /// What the helper menu shows about the keyboard itself.
    var deviceName = "Keyboard"
    var deviceWhere = "Looking…"
    var connected = false
    /// Charge left, when the keyboard will say — over Bluetooth it answers the
    /// standard Battery Service; on the cable there is nothing to ask.
    var battery: Int?
    var allThemes: [ThemeSummary] = []
    var favourites: [Int: String] = [:]
    var currentTheme: String?
    var menuBrightness: Double = 100
    var settings: [DeviceSetting] = []
    var settingsState: LoadState = .loading
    var firmware = ""
    var showExtras = false

    /// The look was handed in and must not be replaced by a reading.
    ///
    /// The snapshot harness rolls a look by seed so a render is repeatable, and
    /// the builder reads the keyboard when it appears — so with a keyboard
    /// attached the roll was overwritten a beat later and `--seed` quietly did
    /// nothing. A flag that says "this is the look" is the difference between a
    /// harness and a coincidence.
    var pinned = false

    /// What the app is waiting on the keyboard for, and nil when it is not.
    ///
    /// Every one of these goes out to a keyboard over Bluetooth, and a flash
    /// write is not instant. With nothing on screen saying so, a theme tile
    /// read as a dead button and got pressed again — which queued a second
    /// write behind the first and made it slower still. A surface that is
    /// working says it, and refuses the second press rather than stacking it.
    var busy: Busy?

    /// The last thing that went wrong, kept until something else happens.
    ///
    /// `status` is only on screen in the builder, so a failure in the helper
    /// menu — `match-wallpaper` finding no wallpaper, say — was written to a
    /// string nobody could see. Silence is the one thing a failure must not be.
    var failure: String?

    struct Busy: Equatable {
        /// What is being done, in words, for the surface to show.
        let what: String
        /// The tile or row that started it, so only that one shows a spinner.
        let source: String?
    }

    /// How long the last few calls to the keyboard actually took.
    ///
    /// Measured rather than guessed: the affordance is supposed to last as long
    /// as the wait does, and "it feels like a moment" is not a number. Printed
    /// by `--timings`.
    private(set) var timings: [(String, Double)] = []

    private var epoch = Date()

    /// Black keys or white ones.
    ///
    /// Nothing on the wire says which this board is — the protocol has no such
    /// field and the model name is the same either way — so it is remembered
    /// here rather than read, and the deck draws the plastic accordingly.
    var finish: KeyboardFinish = .dark {
        didSet { UserDefaults.standard.set(finish.rawValue, forKey: Self.finishKey) }
    }
    private static let finishKey = "keyboardFinish"

    init(device: any DeviceService) {
        self.device = device
        if let saved = UserDefaults.standard.string(forKey: Self.finishKey),
           let known = KeyboardFinish(rawValue: saved) {
            finish = known
        }
    }

    var zone: LookModel.Zone? { look?.zones[selectedZone] }

    var zoneCaption: String {
        let name = zoneNames[selectedZone] ?? selectedZone
        guard let e = zone?.effect else { return name }
        return "\(name) · \(effectLabel(e))"
    }

    /// A range straight from the device, so a slider cannot offer a number the
    /// keyboard will refuse.
    func range(_ key: KeyPath<LookModel.Ranges, [Int]>) -> ClosedRange<Double> {
        guard let r = look?.ranges[keyPath: key], r.count == 2 else { return 0...100 }
        return Double(r[0])...Double(max(r[1], r[0] + 1))
    }

    /// The colour the selected zone is showing, used to tint its own controls —
    /// a slider for the keys should be filled with the keys' light.
    var zoneTint: Color {
        Color(hex: zone?.preview.first ?? zone?.swatch.first ?? "#0A84FF") ?? .accentColor
    }

    /// The colour well the design puts on a stop. AppKit's picker is the one
    /// place a system control is right: it is the colour picker people know.
    func pickColour(_ i: Int) {
        guard let hex = zone?.stops[safe: i]?.color else { return }
        openColourPanel(.stop(i), from: hex)
    }

    /// The colour the reactive layer lights in.
    func pickReactiveColour() {
        guard let current = reactive?.color else { return }
        openColourPanel(.reactive, from: current)
    }

    private func openColourPanel(_ target: ColourTarget, from hex: String) {
        guard let start = Color(hex: hex) else { return }
        let panel = NSColorPanel.shared
        panel.color = NSColor(start)
        panel.showsAlpha = false
        colourTarget = target
        panel.setTarget(colourSink)
        panel.setAction(#selector(ColourSink.changed(_:)))
        panel.makeKeyAndOrderFront(nil)
    }

    /// Which well the shared colour panel is currently editing.
    enum ColourTarget {
        case stop(Int)
        case reactive
    }

    @ObservationIgnored private var colourTarget: ColourTarget?
    @ObservationIgnored private var colourSinkStore: ColourSink?

    /// Built on first use: `@Observable` rewrites stored properties, and a
    /// `lazy` one referring to `self` cannot survive that.
    private var colourSink: ColourSink {
        if let colourSinkStore { return colourSinkStore }
        let sink = ColourSink { [weak self] colour in
            guard let self, let target = self.colourTarget else { return }
            switch target {
            case .stop(let i):
                self.mutate { z in
                    guard z.stops.indices.contains(i) else { return }
                    z.stops[i].color = colour.hexString
                    z.swatch = z.stops.map(\.color)
                    z.preview = z.swatch
                }
            case .reactive:
                self.mutateReactive { $0.color = colour.hexString }
            }
        }
        colourSinkStore = sink
        return sink
    }

    /// The light the deck is casting, which is what tints the glass.
    var bloom: Color {
        let swatch = (look?.zones["keyboard"]?.preview ?? []).compactMap { NSColor(Color(hex: $0) ?? .blue) }
        guard !swatch.isEmpty else { return .blue }
        var r = 0.0, g = 0.0, b = 0.0
        for c in swatch {
            let s = c.usingColorSpace(.sRGB) ?? c
            r += s.redComponent; g += s.greenComponent; b += s.blueComponent
        }
        let n = Double(swatch.count)
        return Color(.sRGB, red: r / n, green: g / n, blue: b / n)
    }

    /// Whether anything on the deck actually moves.
    var isAnimated: Bool { look?.isAnimated ?? false }

    /// Seconds since the deck started.
    ///
    /// Not a phase. One clock stepped by the fastest zone on show made every
    /// other zone run at a speed nothing had been set to, and the rate it
    /// stepped at — 0.06 to 0.96 cycles a second — was a curve chosen to look
    /// busy, not the rate the keyboard runs at. Each zone now turns this into
    /// its own phase from the period the device actually stores; see `Lit`.
    func elapsed(at date: Date) -> Double {
        date.timeIntervalSince(epoch)
    }

    // MARK: - Reading and writing

    /// Run something that reaches the keyboard, and say so while it runs.
    ///
    /// One place, so no surface can forget: it marks the app busy, refuses a
    /// second press while the first is in flight, times the call, and turns a
    /// throw into something a person can read rather than a string in a field
    /// that surface does not draw.
    @discardableResult
    func working<T>(_ what: String,
                    from source: String? = nil,
                    _ body: () async throws -> T) async -> T? {
        guard busy == nil else { return nil }
        busy = Busy(what: what, source: source)
        failure = nil
        status = what + "…"
        let started = Date()
        defer {
            timings.append((what, Date().timeIntervalSince(started)))
            if timings.count > 20 { timings.removeFirst(timings.count - 20) }
            busy = nil
        }
        do {
            return try await body()
        } catch {
            let why = error.localizedDescription.trimmingCharacters(in: .whitespacesAndNewlines)
            failure = why.isEmpty ? "The keyboard did not answer." : why
            status = failure ?? ""
            return nil
        }
    }

    /// Read the keyboard, and say so.
    ///
    /// This looked broken and was not: the read took the better part of a
    /// second with nothing on screen to say it was happening, and then set the
    /// status line to the same sentence it already held — so a read that had
    /// worked was indistinguishable from a button that did nothing. It goes
    /// through `working` now like every other thing that reaches the keyboard,
    /// and says what it did afterwards.
    func load() async {
        guard !pinned else { return }
        let fresh = await working("Reading the keyboard", from: "read") {
            try await device.look(random: false, seed: nil)
        }
        guard let fresh else { return }
        look = fresh
        menuBrightness = Double(fresh.zones["keyboard"]?.brightness ?? 100)
        currentTheme = nil
        status = "Read from the keyboard — nothing is written until you apply"
    }

    func roll() async {
        let rolled = await working("Rolling a look", from: "roll") {
            try await device.look(random: true, seed: nil)
        }
        guard let rolled else { return }
        look = rolled
        currentTheme = nil
        status = "Rolled — nothing is written until you apply"
    }

    func apply() async {
        guard let look else { return }
        let done = await working("Writing to the keyboard", from: "apply") {
            try await device.apply(look)
        }
        if done != nil { status = "Applied to the keyboard" }
    }

    /// Copying by dragging one zone onto another.
    ///
    /// Zones do not offer the same things, so what the destination cannot do is
    /// converted rather than refused: an effect it does not have stays as it
    /// was, and a diagonal angle becomes the nearer of left or right on a strip
    /// that only runs two ways.
    func copy(from: String, to: String) async {
        guard var look, let source = look.zones[from], var dest = look.zones[to] else { return }
        if dest.offers.contains(where: { $0.key == source.effect }) {
            dest.effect = source.effect
        }
        dest.stops = source.stops
        dest.swatch = source.swatch
        dest.preview = source.preview
        dest.brightness = source.brightness
        dest.opacity = source.opacity
        dest.speed = source.speed
        dest.length = source.length
        dest.angle = dest.anglesFree
            ? source.angle
            : ((source.angle > 90 && source.angle <= 270) ? 180 : 0)
        look.zones[to] = dest
        self.look = look
        status = "Copied \(zoneNames[from] ?? from) to \(zoneNames[to] ?? to) — not written yet"
    }

    // MARK: - Moving a scheme between the keyboard's own slots

    /// What is on this slot, trimmed to what was asked for.
    ///
    /// A copy rather than a reference: the keyboard is about to be switched
    /// over, and the model will be re-read the moment it reconnects.
    func slotCopy(zones wanted: Set<String>, reactive: Bool) -> LookModel? {
        guard var copy = look else { return nil }
        // A zone left out keeps whatever the destination already has, which is
        // what "copy only the keys" has to mean — there is no third state.
        for id in zoneOrder where !wanted.contains(id) {
            copy.zones[id] = nil
        }
        if !reactive {
            copy.typing.enabled = false
            copy.gesture.enabled = false
        }
        return copy
    }

    /// Write a held copy onto whatever slot is connected now.
    ///
    /// The core's `copy --from --to` cannot be used from a window: it stops and
    /// waits on standard input for the keyboard to be switched over, which is
    /// a prompt with nowhere to appear. This is the same two steps with the
    /// waiting done by the sheet, and it writes through the verified path.
    func writeToSlot(_ held: LookModel, named slot: String = "this slot") async {
        var toWrite = held
        // Only the zones that were taken; the rest of this slot is left alone.
        if let current = look {
            for id in zoneOrder where held.zones[id] == nil {
                toWrite.zones[id] = current.zones[id]
            }
        }
        let written = await working("Copying to \(slot)", from: "slot") {
            try await device.apply(toWrite)
        }
        guard written != nil else { return }
        await reread()
        status = "Copied onto \(slot)"
    }

    // MARK: - Editing

    func mutate(_ change: (inout LookModel.Zone) -> Void) {
        guard var look, var z = look.zones[selectedZone] else { return }
        change(&z)
        look.zones[selectedZone] = z
        self.look = look
    }

    func setEffect(_ effect: String) {
        mutate { $0.effect = effect }
    }

    // MARK: - The layer that answers your fingers

    /// The reactive layer this zone carries, if it carries one.
    ///
    /// Only two do: the keys light what you press, and the touch area trails
    /// what you drag. The strips have neither, and the device refuses one.
    var reactive: LookModel.Reactive? {
        switch selectedZone {
        case "keyboard": return look?.typing
        case "touchpad": return look?.gesture
        default: return nil
        }
    }

    /// What this zone's reactive layer is called on this zone.
    var reactiveTitle: String {
        selectedZone == "touchpad" ? "When you touch" : "When you type"
    }

    var reactiveNote: String {
        selectedZone == "touchpad"
            ? "A trail that follows your finger across the keys"
            : "The key you press lights, over whatever else this zone is doing"
    }

    func mutateReactive(_ change: (inout LookModel.Reactive) -> Void) {
        guard var look else { return }
        switch selectedZone {
        case "keyboard":
            var r = look.typing; change(&r); look.typing = r
        case "touchpad":
            var r = look.gesture; change(&r); look.gesture = r
        default:
            return
        }
        self.look = look
    }

    var reactiveOn: Binding<Bool> {
        Binding(get: { self.reactive?.enabled ?? false },
                set: { v in self.mutateReactive { $0.enabled = v } })
    }

    var reactiveAmount: Binding<Double> {
        Binding(get: { Double(self.reactive?.amount ?? 1) },
                set: { v in self.mutateReactive { $0.amount = Int(v.rounded()) } })
    }

    var reactiveRange: ClosedRange<Double> {
        guard let r = reactive else { return 1...3 }
        return Double(r.min)...Double(max(r.max, r.min + 1))
    }

    var brightnessBinding: Binding<Double> {
        Binding(get: { Double(self.zone?.brightness ?? 100) },
                set: { v in self.mutate { $0.brightness = Int(v) } })
    }
    var opacityBinding: Binding<Double> {
        Binding(get: { Double(self.zone?.opacity ?? 100) },
                set: { v in self.mutate { $0.opacity = Int(v) } })
    }
    var speedBinding: Binding<Double> {
        Binding(get: { Double(self.zone?.speed ?? 5000) },
                set: { v in self.mutate { $0.speed = Int(v) } })
    }
    var lengthBinding: Binding<Double> {
        Binding(get: { Double(self.zone?.length ?? 500) },
                set: { v in self.mutate { $0.length = Int(v) } })
    }
    var angleBinding: Binding<Double> {
        Binding(get: { Double(self.zone?.angle ?? 0) },
                set: { v in self.mutate { $0.angle = Int(v) } })
    }

    /// A colour stop. The well edits the colour itself; the preview follows the
    /// stops until the next read from the device settles it.
    func stopColour(_ i: Int) -> Binding<Color> {
        Binding(
            get: { Color(hex: self.zone?.stops[safe: i]?.color ?? "#000000") ?? .black },
            set: { c in
                self.mutate { z in
                    guard z.stops.indices.contains(i) else { return }
                    z.stops[i].color = c.hexString
                    z.swatch = z.stops.map(\.color)
                    z.preview = z.swatch
                }
            })
    }

    /// Where a stop sits along the zone.
    func stopPosition(_ i: Int) -> Binding<Double> {
        Binding(
            get: { Double(self.zone?.stops[safe: i]?.position ?? 0) },
            set: { v in self.mutate { z in
                guard z.stops.indices.contains(i) else { return }
                z.stops[i].position = Int(v)
            } })
    }

    func addStop() {
        mutate { z in
            guard z.stops.count < (self.look?.ranges.markers ?? 5) else { return }
            let last = z.stops.last
            z.stops.append(LookModel.Stop(color: last?.color ?? "#00C8FF",
                                          position: min(100, (last?.position ?? 0) + 25),
                                          opacity: 100))
            z.swatch = z.stops.map(\.color)
            z.preview = z.swatch
        }
    }

    func removeStop(_ i: Int) {
        mutate { z in
            guard z.stops.count > 1, z.stops.indices.contains(i) else { return }
            z.stops.remove(at: i)
            z.swatch = z.stops.map(\.color)
            z.preview = z.swatch
        }
    }
}

extension Array {
    subscript(safe i: Int) -> Element? { indices.contains(i) ? self[i] : nil }
}

// MARK: - What the helper menu needs

extension BuilderModel {
    func themeList() async throws -> [ThemeSummary] {
        try await device.themes()
    }

    /// The menu opens often and must never look empty while it thinks.
    func loadMenu() async {
        if allThemes.isEmpty { await loadThemes() }
        if favourites.isEmpty { favourites = (try? await device.favourites()) ?? [:] }
        if look == nil { await load() }
        if let info = try? await device.info() {
            deviceName = info.model
            deviceWhere = info.transport
            connected = true
        } else {
            connected = false
        }
        // Last, and allowed to fail: the menu is already drawn by now, and a
        // keyboard that will not say has nothing to show rather than an error.
        battery = await Battery.shared.read()
    }

    /// Six tiles: what the design shows without opening anything.
    ///
    /// The ones with a key first, so a tile and its shortcut agree, then a
    /// spread across the families. Taking the first six in order fills the grid
    /// with Blackout and Typing Only — two black rectangles, which is a poor
    /// advertisement for a keyboard that lights up.
    var quickThemes: [ThemeSummary] {
        var chosen = zoneOrderedFavourites
        var seen = Set(chosen.map(\.id))

        // One from each family in turn, so the grid shows the range.
        var byGroup: [String: [ThemeSummary]] = [:]
        for t in allThemes where !seen.contains(t.id) && !isDark(t) {
            byGroup[t.group, default: []].append(t)
        }
        let groups = byGroup.keys.sorted()
        var round = 0
        while chosen.count < 6 {
            var added = false
            for g in groups where chosen.count < 6 {
                if let t = byGroup[g]?[safe: round], !seen.contains(t.id) {
                    chosen.append(t); seen.insert(t.id); added = true
                }
            }
            if !added { break }
            round += 1
        }
        // Only if the range runs out do the dark ones fill the grid.
        for t in allThemes where chosen.count < 6 && !seen.contains(t.id) {
            chosen.append(t); seen.insert(t.id)
        }
        return Array(chosen.prefix(6))
    }

    /// A theme that shows as a black rectangle says nothing in a tile.
    private func isDark(_ t: ThemeSummary) -> Bool {
        let colours = t.colours.compactMap { Color(hex: $0) }
        guard !colours.isEmpty else { return true }
        return colours.allSatisfy { c in
            let n = NSColor(c).usingColorSpace(.sRGB) ?? .black
            return (0.2126 * n.redComponent + 0.7152 * n.greenComponent + 0.0722 * n.blueComponent) < 0.06
        }
    }

    private var zoneOrderedFavourites: [ThemeSummary] {
        (1...5).compactMap { n in favourites[n].flatMap { id in allThemes.first { $0.id == id } } }
    }

    func key(for id: String) -> String {
        if let n = favourites.first(where: { $0.value == id })?.key { return "⌃⌥\(n)" }
        if id == "random" { return "⌃⌥1" }
        if id == "wallpaper" { return "⌃⌥2" }
        return ""
    }

    /// Brightness, everywhere at once — the one thing worth changing without
    /// opening a window, and it writes straight away.
    func setBrightness(_ percent: Double) async {
        guard var m = look else { return }
        for (id, var z) in m.zones {
            z.brightness = Int(percent.rounded())
            m.zones[id] = z
        }
        // The bar moves with the finger; the keyboard catches up.
        look = m
        menuBrightness = percent
        let sent = m
        let done = await working("Setting brightness", from: "brightness") {
            try await device.apply(sent)
        }
        if done != nil { status = "Brightness \(Int(percent.rounded()))%" }
    }

    func matchWallpaper() async {
        let done = await working("Reading your desktop picture", from: "wallpaper") {
            try await device.matchWallpaper()
        }
        guard done != nil else { return }
        await reread()
        status = "Matched your desktop picture"
        currentTheme = nil
    }

    /// Read the keyboard back without the busy affordance.
    ///
    /// Used after something that already showed one: a second spinner for the
    /// read that follows a write is the same wait counted twice.
    private func reread() async {
        if let fresh = try? await device.look(random: false, seed: nil) {
            look = fresh
            menuBrightness = Double(fresh.zones["keyboard"]?.brightness ?? 100)
        }
    }

    /// The themes we ship and the themes you saved, in one list.
    ///
    /// Two sources, because they are two different things: ours are compiled
    /// in, yours are files in the gallery. Neither read needs the keyboard, so
    /// the list is there even when nothing is plugged in.
    func loadThemes() async {
        let built = (try? await device.themes()) ?? []
        let mine = (try? await device.profiles()) ?? []
        allThemes = built + mine
    }

    /// Whether a theme is one of yours, which is what says how to apply it and
    /// whether it can be renamed or removed at all.
    func isYours(_ id: String) -> Bool {
        allThemes.first { $0.id == id }?.group == "Yours"
    }

    /// The menu acts on the keyboard straight away — that is what makes it the
    /// quick surface. The builder is the place where nothing is written until
    /// you say so; these two promises are different on purpose.
    func applyTheme(_ id: String) async {
        let name = allThemes.first { $0.id == id }?.name ?? id
        let mine = isYours(id)
        let done = await working("Applying \(name)", from: id) {
            if mine {
                try await device.applyProfile(id)
            } else {
                try await device.applyTheme(id)
            }
        }
        guard done != nil else { return }
        currentTheme = id
        status = "Applied \(name)"
        // The read that follows the write is not part of the wait: the theme is
        // already on the keyboard, and the deck catching up is the app's
        // business, not something to hold a spinner up for.
        await reread()
    }

    func rollAndApply() async {
        let done = await working("Rolling a theme", from: "random") { () -> LookModel in
            let rolled = try await device.look(random: true, seed: nil)
            try await device.apply(rolled)
            return rolled
        }
        guard let rolled = done else { return }
        look = rolled
        currentTheme = nil
        status = "Rolled and applied"
    }

    // MARK: - Showing a theme without wearing it

    func preview(theme id: String) async {
        guard previewOf != id else { return }
        previewOf = id
        previewLook = nil
        var fresh = try? await device.look(of: id)
        // A theme that sets no reactive layer does not clear the keyboard's —
        // it leaves it alone, which is why Blackout still lights what you touch
        // and in whatever colour was last set. The preview shows the keyboard's
        // own layer in that case, because that is what you would see.
        if var carried = fresh, !carried.typing.enabled, let live = look {
            carried.typing = live.typing
            carried.gesture = live.gesture
            fresh = carried
        }
        // Still the one being shown? A slow reply for a theme the person has
        // already scrolled past belongs nowhere.
        guard previewOf == id else { return }
        previewLook = fresh
    }
}

// MARK: - Settings, and the gallery

extension BuilderModel {
    func loadSettings() async {
        settingsState = .loading
        if allThemes.isEmpty { await loadThemes() }
        do {
            let read = try await device.settings()
            settings = read
            // An empty read is not success. The keyboard always has settings,
            // so nothing back means the reply was not understood — and a blank
            // pane with no explanation is the worst way to say that.
            settingsState = read.isEmpty
                ? .failed("The keyboard answered, but with no settings in it.")
                : .ready
        } catch {
            settingsState = .failed(error.localizedDescription)
        }
        if let info = try? await device.info() {
            deviceName = info.model
            deviceWhere = info.transport
        }
    }

    /// The panes the sidebar offers, and which of the core's groups belong to
    /// each. The core groups by what the setting *is*; the design groups by
    /// what a person came to change.
    func settingGroups(in pane: String) -> [(name: String, items: [DeviceSetting])] {
        let wanted: [String]
        switch pane {
        case "Touch": wanted = ["Touch", "Multi-touch"]
        case "Keys": wanted = ["Keyboard"]
        case "Power": wanted = ["Power"]
        case "Lighting": wanted = ["Lighting"]
        default: wanted = ["Device"]
        }
        return wanted.compactMap { name in
            let items = settings.filter { $0.group == name }
            return items.isEmpty ? nil : (name, items)
        }
    }

    func setSetting(_ key: String, to value: String) async {
        do {
            try await device.setSetting(key, to: value)
            settings = (try? await device.settings()) ?? settings
            status = "\(key) is now \(value)"
        } catch {
            status = error.localizedDescription
        }
    }

    /// Keep the look you have, under a name, in your own gallery.
    ///
    /// It saves the *model* — what the builder is showing, applied or not —
    /// rather than reading the keyboard back, so saving does not quietly write
    /// to it. Names are the core's to judge: it refuses one that would collide
    /// with a theme we ship or that cannot be a filename, and what it says is
    /// what the surface shows.
    func saveLookAsTheme(named name: String) async {
        let wanted = name.trimmingCharacters(in: .whitespaces)
        guard let look else {
            status = "There is no look to save yet"
            return
        }
        do {
            try await device.saveProfile(wanted, from: look)
            await loadThemes()
            currentTheme = wanted
            status = "Saved as \(wanted)"
        } catch {
            status = error.localizedDescription
        }
    }

    func renameTheme(_ id: String, to name: String) async {
        let wanted = name.trimmingCharacters(in: .whitespaces)
        guard isYours(id) else {
            status = "Only your own themes can be renamed"
            return
        }
        do {
            try await device.renameProfile(id, to: wanted)
            await loadThemes()
            if currentTheme == id { currentTheme = wanted }
            status = "Renamed to \(wanted)"
        } catch {
            status = error.localizedDescription
        }
    }

    /// The one thing in this app that cannot be undone by doing it again, which
    /// is why it is the one thing that asks first.
    func deleteTheme(_ id: String) async {
        guard isYours(id) else {
            status = "Only your own themes can be removed"
            return
        }
        do {
            try await device.deleteProfile(id)
            await loadThemes()
            if currentTheme == id { currentTheme = nil }
            status = "Removed \(id)"
        } catch {
            status = error.localizedDescription
        }
    }
}
