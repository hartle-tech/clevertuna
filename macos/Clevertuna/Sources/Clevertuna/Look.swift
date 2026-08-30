import SwiftUI

/// What the keyboard is showing, as the builder edits it.
///
/// This is the shape `clevertuna look` prints, field for field. It is written
/// against the real output rather than against an idea of it — the first
/// version guessed `colors` where the core says `stops`, and the app quietly
/// fell back to a sample look while reporting "no keyboard", which reads as a
/// dead Bluetooth connection and is nothing of the sort.
struct LookModel: Codable, Sendable {
    struct Ranges: Codable, Sendable {
        let speed: [Int]
        let length: [Int]
        let angle: [Int]
        let brightness: [Int]
        let opacity: [Int]
        /// How many colour stops a zone will hold.
        let markers: Int
        /// `period = speedPivot - speed`, in milliseconds — the device's own
        /// relation, sent over so the preview can follow the speed slider
        /// rather than animating at whatever the last read implied.
        var speedPivot: Int?
    }

    /// One colour along the zone, where it sits, and how strong it is there.
    struct Stop: Codable, Sendable, Hashable {
        var color: String
        var position: Int
        var opacity: Int
    }

    /// What an effect can be given. A solid colour has no speed to set and no
    /// gradient to spread, and showing those controls anyway invites a person
    /// to change a number the keyboard will ignore.
    struct Offer: Codable, Sendable, Hashable {
        let key: String
        let label: String
        let animated: Bool
        let colours: Bool
        let gradient: Bool
        let length: Bool
        let speed: Bool
    }

    struct Zone: Codable, Sendable {
        var effect: String
        var stops: [Stop]
        /// The stop colours, in order.
        var swatch: [String]
        /// Resolved colours across the zone at full strength — brightness and
        /// opacity are the window's to apply, so the sliders redraw the deck as
        /// the thumb moves rather than waiting on a round trip.
        var preview: [String]
        var brightness: Int
        var opacity: Int
        var speed: Int
        /// One cycle of the animation, in milliseconds, as the device stores
        /// it. The preview runs at this rate rather than at a rate invented
        /// here, so what the deck shows is what the keyboard does. Optional
        /// only so a model written by an older core still decodes.
        var periodMs: Int?
        var length: Int
        /// Degrees, `0 = to the right`, counting anticlockwise — the
        /// convention `docs/PROTOCOL.md` §7 records and `effects.rs` encodes
        /// from. It is *not* a compass bearing, and reading it as one is what
        /// made every direction in the builder ninety degrees out.
        var angle: Int
        /// A touch strip runs one way or the other; only the areas take an angle.
        let anglesFree: Bool
        let offers: [Offer]

        var offer: Offer? { offers.first { $0.key == effect } }
    }

    /// Lighting the key you press, and trailing your finger on the touch area.
    ///
    /// These are a layer *over* whatever the zone is doing, which is what makes
    /// a blackout keyboard light up under your fingers and nowhere else. The
    /// core has always read and written them; they were simply never on screen.
    struct Reactive: Codable, Sendable {
        var enabled: Bool
        var color: String
        /// How long a key stays lit, or how far a trail follows a finger.
        var amount: Int
        let label: String
        let min: Int
        let max: Int
    }

    let ranges: Ranges
    var zones: [String: Zone]
    var typing: Reactive
    var gesture: Reactive

    /// Whether anything in this look actually moves.
    ///
    /// A still look pauses the deck's clock, and a paused clock costs nothing —
    /// redrawing eighty-four keys thirty times a second to show a picture that
    /// never changes cost a third of a core.
    /// A reactive layer counts: a blackout deck is a still picture until you
    /// remember that what it does is light up under your hands, which is the
    /// whole of that theme and the one thing worth showing about it.
    var isAnimated: Bool {
        if typing.enabled || gesture.enabled { return true }
        return zoneOrder.contains { zones[$0]?.offer?.animated ?? false }
    }
}

/// What the keyboard itself is made of.
///
/// **A backlit keycap is not a lit rectangle.** The plastic is opaque; the light
/// comes through the printed legend and out of the gap around the cap, and the
/// cap stays the colour of the keyboard. Painting whole keycaps in the light
/// made every theme look like a screen showing a picture of a keyboard.
///
/// Which finish this particular board is cannot be read from the device — the
/// protocol carries no such field and the model name is the same either way —
/// so it is the one thing here a person tells us.
enum KeyboardFinish: String, CaseIterable, Sendable {
    case dark, light

    var label: String { self == .dark ? "Black" : "White" }

    /// The tray the keys sit in.
    var deck: Color { self == .dark ? Color(white: 0.09) : Color(white: 0.80) }
    /// The keycap's own plastic.
    var cap: Color { self == .dark ? Color(white: 0.17) : Color(white: 0.93) }
    /// A legend with no light behind it.
    var legend: Color { self == .dark ? Color(white: 0.52) : Color(white: 0.28) }
    /// The specular line along a cap's top edge.
    var sheen: Color { self == .dark ? .white.opacity(0.16) : .white.opacity(0.75) }
}

/// Display order. A map has no order, and the four zones read in one particular
/// order on the hardware: the keys, then the surface over them, then the strips.
let zoneOrder = ["keyboard", "touchpad", "leftSlider", "rightSlider"]
let zoneShort = ["keyboard": "Keys", "touchpad": "Pad",
                 "leftSlider": "Left", "rightSlider": "Right"]
let zoneNames = ["keyboard": "Keys", "touchpad": "Touchpad",
                 "leftSlider": "Left slider", "rightSlider": "Right slider"]

extension Color {
    /// `#RRGGBB`, which is what the device speaks.
    init?(hex: String) {
        var s = hex.trimmingCharacters(in: .whitespaces)
        if s.hasPrefix("#") { s.removeFirst() }
        guard s.count == 6, let v = UInt32(s, radix: 16) else { return nil }
        self.init(.sRGB,
                  red: Double((v >> 16) & 0xFF) / 255,
                  green: Double((v >> 8) & 0xFF) / 255,
                  blue: Double(v & 0xFF) / 255)
    }

    var hexString: String {
        let c = NSColor(self).usingColorSpace(.sRGB) ?? .black
        return String(format: "#%02X%02X%02X",
                      Int((c.redComponent * 255).rounded()),
                      Int((c.greenComponent * 255).rounded()),
                      Int((c.blueComponent * 255).rounded()))
    }

    /// Legend ink that stays readable on a key of any colour — a lit keycap
    /// runs from near-black to near-white, and one fixed ink disappears into
    /// half of that range.
    static func ink(on background: Color) -> Color {
        let c = NSColor(background).usingColorSpace(.sRGB) ?? .white
        let luma = 0.2126 * c.redComponent + 0.7152 * c.greenComponent + 0.0722 * c.blueComponent
        return luma > 0.55 ? .black.opacity(0.78) : .white.opacity(0.92)
    }
}

/// Everything an effect is called, in one place.
func effectLabel(_ key: String) -> String {
    switch key {
    case "colorWave": return "Colour wave"
    case "colorCycle": return "Colour cycle"
    case "breathing": return "Breathing"
    case "aurora": return "Aurora"
    case "solidColor": return "Solid colour"
    default: return key
    }
}
