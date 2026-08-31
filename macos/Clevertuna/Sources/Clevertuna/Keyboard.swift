import Foundation

/// The physical keyboard, as data.
///
/// The Clevetura CLVX S is a compact US ANSI board: a half-height function row,
/// five letter rows, a six-key column down the right edge and an inverted-T
/// arrow cluster.
///
/// The part every drawing of it gets wrong: **there is no separate touchpad.**
/// The touch surface is a region of the key field itself, and the two sliders
/// are strips running along the F2–F6 and F7–F11 keycaps. A pad drawn beside
/// the keys is a picture of a keyboard that does not exist.
///
/// The table is `assets/clvx-s-layout.json`, shared with the Rust core and the
/// design. A second copy is how a drawing drifts out of step with the hardware.
struct KeyLayout: Decodable, Sendable {
    struct Unit: Decodable, Sendable {
        let width: Double
        let height: Double
        let keyGap: Double
    }

    struct Key: Decodable, Sendable, Identifiable {
        let x: Double
        let w: Double
        let label: String
        let sub: String?
        let size: String?
        let homing: Bool?
        let led: Bool?
        /// Set only where a key does not fill its row's band.
        ///
        /// The arrow cluster is an inverted-T squeezed into one column: up and
        /// down are half-height keys stacked in the space of one. A row with a
        /// single `y` and `h` cannot say that, and drawing them full height put
        /// two keys on top of each other.
        let y: Double?
        let h: Double?
        private let spaceFlag: Bool?

        /// Stable within a row; rows compose it with their own index.
        var id: String { "\(x)-\(y ?? -1)-\(label)" }
        var isSpace: Bool { spaceFlag ?? false }
        var isHoming: Bool { homing ?? false }
        var hasLED: Bool { led ?? false }

        enum CodingKeys: String, CodingKey {
            case x, w, label, sub, size, homing, led, y, h
            case spaceFlag = "space"
        }
    }

    struct Row: Decodable, Sendable, Identifiable {
        let y: Double
        let h: Double
        let keys: [Key]
        var id: Double { y }
    }

    struct Zone: Decodable, Sendable, Identifiable {
        let id: String
        let name: String
        let shape: String
        let x: Double?
        let y: Double?
        let w: Double?
        let h: Double?

        /// The key field is the whole deck; the rest are regions drawn over it.
        var isField: Bool { shape == "field" }
        var isStrip: Bool { shape == "strip" }
    }

    let unit: Unit
    let rows: [Row]
    let zones: [Zone]

    var aspect: Double { unit.width / unit.height }

    /// Every key with the band it actually occupies, for a single pass over the
    /// deck. A key may override its row's `y` and `h`; most do not.
    var placedKeys: [(y: Double, h: Double, key: Key)] {
        rows.flatMap { row in
            row.keys.map { (($0.y ?? row.y), ($0.h ?? row.h), $0) }
        }
    }

    func zone(_ id: String) -> Zone? { zones.first { $0.id == id } }

    /// Loaded from the app bundle. The layout is not optional to the product —
    /// a keyboard app that cannot draw the keyboard has nothing to show — so a
    /// missing or malformed table is a build error, not a runtime fallback.
    static let shared: KeyLayout = {
        guard let url = Bundle.main.url(forResource: "clvx-s-layout", withExtension: "json"),
              let data = try? Data(contentsOf: url),
              let layout = try? JSONDecoder().decode(KeyLayout.self, from: data)
        else {
            fatalError("clvx-s-layout.json is missing from the bundle")
        }
        return layout
    }()
}
