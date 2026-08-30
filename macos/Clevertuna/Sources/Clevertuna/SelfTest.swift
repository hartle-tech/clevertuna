import Foundation
import AppKit

/// Checks the app against the core, with the keyboard attached.
///
/// This exists because of a specific failure: the model was written against an
/// idea of the core's output rather than the output, asked for `colors` where
/// the core says `stops`, and the decode error surfaced as the words "no
/// keyboard". Codec vectors would not have caught it — a test that shares its
/// constants with the code under test cannot fail. So every check here runs
/// against the live device and the real binary.
///
/// Read-only apart from the last one, which writes the brightness it just read
/// back — the value does not change, but the write path is exercised.
enum SelfTest {
    @MainActor
    static func run() async -> Never {
        let device = RustCoreBackend()
        var failures = 0

        func check(_ name: String, _ body: () async throws -> String?) async {
            do {
                if let why = try await body() {
                    print("FAIL  \(name)\n      \(why)")
                    failures += 1
                } else {
                    print("ok    \(name)")
                }
            } catch {
                print("FAIL  \(name)\n      \(error.localizedDescription)")
                failures += 1
            }
        }

        // 1. The shape the core prints is the shape this app decodes.
        var live: LookModel?
        await check("the keyboard's current look decodes") {
            let m = try await device.look(random: false, seed: nil)
            live = m
            guard m.zones.count == 4 else { return "expected four zones, got \(m.zones.count)" }
            for id in zoneOrder where m.zones[id] == nil { return "no zone named \(id)" }
            return nil
        }

        // 2. Every zone says what it offers, and its effect is one of them.
        await check("each zone's effect is one it offers") {
            guard let m = live else { return "nothing to check" }
            for (id, z) in m.zones {
                guard !z.offers.isEmpty else { return "\(id) offers no effects" }
                guard z.offers.contains(where: { $0.key == z.effect }) else {
                    return "\(id) is set to \(z.effect), which it does not offer"
                }
            }
            return nil
        }

        // 3. The strips do not take an angle, and the areas do. Getting this
        //    wrong puts a gradient across eight pixels of thickness.
        await check("only the areas take an angle") {
            guard let m = live else { return "nothing to check" }
            if m.zones["leftSlider"]?.anglesFree != false { return "the left slider claims a free angle" }
            if m.zones["rightSlider"]?.anglesFree != false { return "the right slider claims a free angle" }
            if m.zones["keyboard"]?.anglesFree != true { return "the keys do not take an angle" }
            return nil
        }

        // 4. The deck can be drawn: the layout parses and covers the hardware.
        await check("the keyboard layout loads and fits together") {
            let l = KeyLayout.shared
            // 81, counted off the photographs: six rows, no navigation column,
            // and the arrow cluster's up and down sharing one column. A number
            // rather than a range because the only way this changes is if the
            // table is edited, and then it should be looked at again.
            let keys = l.rows.reduce(0) { $0 + $1.keys.count }
            guard keys == 81 else { return "expected 81 keys, found \(keys)" }
            guard l.rows.count == 6 else { return "expected six rows, found \(l.rows.count)" }
            // The board ends at the right shift; there is no home/page column.
            guard !l.rows.contains(where: { $0.keys.contains { $0.label.hasPrefix("pgup") } })
            else { return "the navigation column is back" }
            for row in l.rows {
                guard row.y + row.h <= l.unit.height + 0.01 else { return "a row runs off the deck" }
                for k in row.keys where k.x + k.w > l.unit.width + 0.01 {
                    return "\(k.label) runs off the right edge"
                }
                // A key may sit in part of its row's band — the stacked arrows —
                // but never outside it.
                for k in row.keys {
                    let top = k.y ?? row.y
                    let bottom = top + (k.h ?? row.h)
                    guard top >= row.y - 0.01, bottom <= row.y + row.h + 0.01 else {
                        return "\(k.label) hangs outside its row"
                    }
                }
            }
            // The whole point: the touch surface is over the keys.
            guard let pad = l.zone("touchpad"), let y = pad.y, let h = pad.h,
                  y > l.rows[0].y, y + h <= l.unit.height + 0.01 else {
                return "the touch surface is not over the key field"
            }
            return nil
        }

        // 5. What this app encodes, the core can read back. This is the seam,
        //    and the one that broke.
        await check("a look this app writes is one the core can read") {
            guard let m = live else { return "nothing to check" }
            let url = FileManager.default.temporaryDirectory
                .appendingPathComponent("clevertuna-selftest-\(UUID().uuidString).json")
            defer { try? FileManager.default.removeItem(at: url) }
            try JSONEncoder().encode(m).write(to: url)
            let out = try await device.preview(url.path)
            guard out.contains("keyboard") || out.contains("Keys") else {
                return "the core did not recognise the model: \(out.prefix(120))"
            }
            return nil
        }

        // 6. The write path, with the value it already has: nothing changes on
        //    the keyboard, and a failure is still a failure.
        await check("applying the current look succeeds") {
            guard let m = live else { return "nothing to check" }
            try await device.apply(m)
            return nil
        }

        // 7. And it is still the same afterwards.
        await check("the keyboard reads back the same look") {
            guard let before = live else { return "nothing to check" }
            let after = try await device.look(random: false, seed: nil)
            for id in zoneOrder {
                guard let a = before.zones[id], let b = after.zones[id] else { return "\(id) vanished" }
                guard a.effect == b.effect else {
                    return "\(id) changed effect: \(a.effect) → \(b.effect)"
                }
                guard a.brightness == b.brightness, a.opacity == b.opacity else {
                    return "\(id) changed light: \(a.brightness)/\(a.opacity) → \(b.brightness)/\(b.opacity)"
                }
                guard a.swatch == b.swatch else {
                    return "\(id) changed colour: \(a.swatch) → \(b.swatch)"
                }
            }
            return nil
        }

        // 8. The gallery round trip. Saving goes through the core twice — the
        //    model becomes the scheme it would write, and that scheme is what
        //    is filed — so this is the check that the two hops agree. It uses
        //    a throwaway name and takes it away again, and it never writes to
        //    the keyboard: saving a look must not mean applying it.
        await check("a theme of your own can be saved, applied, renamed and removed") {
            guard let m = live else { return "nothing to check" }
            let name = "Selftest \(UUID().uuidString.prefix(8))"
            let renamed = name + " renamed"
            try await device.saveProfile(name, from: m)
            guard try await device.profiles().contains(where: { $0.id == name }) else {
                return "\(name) was saved but the gallery does not list it"
            }
            // Applying it is safe because it is a copy of what the keyboard is
            // already showing — the same reason check 6 can write. It is here
            // because a theme of yours takes a different route to the device
            // (`profile apply`, not `theme`) and that branch is the one where a
            // wrong turn writes the wrong thing to the hardware.
            try await device.applyProfile(name)
            try await device.renameProfile(name, to: renamed)
            let after = try await device.profiles()
            guard after.contains(where: { $0.id == renamed }) else {
                return "\(renamed) is not in the gallery after the rename"
            }
            guard let entry = after.first(where: { $0.id == renamed }), !entry.colours.isEmpty else {
                return "the saved theme has no colours, so a picker would show a grey row"
            }
            try await device.deleteProfile(renamed)
            guard try await !device.profiles().contains(where: { $0.id == renamed }) else {
                return "\(renamed) is still there after being removed"
            }
            return nil
        }

        // 9. And the model sends each theme down the right road. The device
        //    layer above proves both roads work; this proves the fork.
        await check("the model knows whose theme is whose") {
            let m = BuilderModel(device: device)
            await m.loadThemes()
            guard !m.allThemes.isEmpty else { return "no themes at all" }
            guard !m.isYours("deep-current") else { return "a theme we ship was taken for one of yours" }
            for t in m.allThemes where (t.group == "Yours") != m.isYours(t.id) {
                return "\(t.id) is in \(t.group) but routed the other way"
            }
            return nil
        }

        // 10. A window brings a menu bar with it.
        //
        //     An `LSUIElement` app contributes no menu bar when it activates
        //     unless it has a main menu, so opening the builder left the bar
        //     blank — indistinguishable from Clevertuna having dismissed the
        //     system's own. This asserts the two halves of the fix directly,
        //     because the visible half only shows on a Mac whose menu bar is
        //     not set to auto-hide, and that is the user's setting to make.
        await check("a window brings a menu bar, and takes it away again") {
            // Held for the whole check: `Windows` keeps the model weakly, and a
            // temporary is gone before `show` reads it — which made the window
            // silently never appear.
            let owner = BuilderModel(device: device)
            Windows.shared.attach(owner)
            guard NSApp.activationPolicy() == .accessory else {
                return "a keyboard app idles in the menu bar, not the dock"
            }
            Windows.shared.show(.builder)
            guard Windows.shared.isUp(.builder) else { return "the window never appeared" }
            guard NSApp.activationPolicy() == .regular else {
                return "an accessory app contributes no menu bar; the window would leave it blank"
            }
            guard let bar = NSApp.mainMenu else { return "no main menu at all" }
            let titles = bar.items.compactMap { $0.submenu?.title }
            for wanted in ["Clevertuna", "Keyboard", "Edit", "Window"] where !titles.contains(wanted) {
                return "the menu bar has no \(wanted) menu — only \(titles.joined(separator: ", "))"
            }
            // ⌘Q and ⌘W were unreachable for the whole life of the app before
            // this menu existed, which is its own bug.
            let keys = bar.items.flatMap { $0.submenu?.items ?? [] }.map(\.keyEquivalent)
            for wanted in ["q", "w", "v"] where !keys.contains(wanted) {
                return "no ⌘\(wanted.uppercased())"
            }
            Windows.shared.close(.builder)
            // The close is delivered on a later turn of the run loop, and the
            // policy change lands after that, so this waits rather than
            // sampling once.
            for _ in 0..<40 where NSApp.activationPolicy() != .accessory {
                try await Task.sleep(for: .milliseconds(50))
            }
            guard NSApp.activationPolicy() == .accessory else {
                return "the dock icon outlived the last window"
                     + (Windows.shared.isUp(.builder) ? " (the window never closed)" : "")
            }
            return nil
        }

        print(failures == 0 ? "\nall checks passed" : "\n\(failures) failed")
        exit(failures == 0 ? 0 : 1)
    }
}
