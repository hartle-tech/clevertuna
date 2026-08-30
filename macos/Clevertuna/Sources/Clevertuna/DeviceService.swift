import Foundation
import AppKit
import AVFoundation

/// Everything the app needs from the keyboard, behind one seam.
///
/// Phase 1 fulfils this by shelling out to the bundled `clevertuna` binary,
/// which is proven against the hardware. Phase 2 replaces it with a Swift
/// CoreBluetooth/HID implementation, and nothing above this protocol changes.
/// The seam exists so design work is not held behind a protocol port.
protocol DeviceService: Sendable {
    func look(random: Bool, seed: Int?) async throws -> LookModel
    /// What a named theme would put on the keyboard, without putting it there.
    /// Reaches nothing — ours are compiled in, yours are files — so a picker
    /// can show a theme moving with no keyboard attached.
    func look(of theme: String) async throws -> LookModel
    func apply(_ model: LookModel) async throws
    func themes() async throws -> [ThemeSummary]
    func applyTheme(_ id: String) async throws
    /// The schemes in the gallery — the themes that are yours rather than ours.
    /// They are files on this machine, so none of these five needs the keyboard
    /// except the one that applies.
    func profiles() async throws -> [ThemeSummary]
    func saveProfile(_ name: String, from model: LookModel) async throws
    func applyProfile(_ name: String) async throws
    func renameProfile(_ name: String, to fresh: String) async throws
    func deleteProfile(_ name: String) async throws
    func layoutJSON() async throws -> Data
    /// What a model would put on the keyboard, without writing it.
    func preview(_ path: String) async throws -> String
    /// Which keyboard this is, and how it is reached.
    func info() async throws -> DeviceInfo
    /// The five themes holding ⌃⌥1 … ⌃⌥5.
    func favourites() async throws -> [Int: String]
    func matchWallpaper() async throws
    func settings() async throws -> [DeviceSetting]
    func setSetting(_ key: String, to value: String) async throws
}

struct DeviceInfo: Sendable {
    let model: String
    let transport: String
}

struct ThemeSummary: Identifiable, Sendable, Hashable {
    let id: String
    let name: String
    let group: String
    let colours: [String]
    let note: String
}

enum DeviceError: LocalizedError {
    case noBinary
    case failed(String)

    var errorDescription: String? {
        switch self {
        case .noBinary:
            return "The clevertuna helper is missing from the app bundle."
        case .failed(let why):
            return why
        }
    }
}

/// Phase 1: the Rust core as a subprocess.
///
/// Deliberately the whole of the device layer, so the port has exactly one
/// place to land. Two rules the hardware imposes and this must not break: one
/// connection at a time, so calls are serialised; and the vendor app holds the
/// write channel when it is running, which the core already reports.
actor RustCoreBackend: DeviceService {
    private let binary: URL?

    init() {
        // "clevertuna-core", not "clevertuna": on a case-insensitive volume
        // the latter is the same file as the app binary itself.
        binary = Bundle.main.url(forAuxiliaryExecutable: "clevertuna-core")
            ?? Bundle.main.executableURL?.deletingLastPathComponent()
                .appendingPathComponent("clevertuna-core")
    }

    func run(_ args: [String]) async throws -> Data {
        guard let binary, FileManager.default.isExecutableFile(atPath: binary.path) else {
            throw DeviceError.noBinary
        }
        let task = Process()
        task.executableURL = binary
        task.arguments = ["--no-color"] + args
        let out = Pipe()
        task.standardOutput = out
        task.standardError = out
        try task.run()
        // Read before waiting: a full pipe buffer would deadlock the child.
        let data = out.fileHandleForReading.readDataToEndOfFile()
        task.waitUntilExit()
        guard task.terminationStatus == 0 else {
            throw DeviceError.failed(String(data: data, encoding: .utf8) ?? "the keyboard did not answer")
        }
        return data
    }

    func look(random: Bool = false, seed: Int? = nil) async throws -> LookModel {
        var args = ["look"]
        if random { args.append("random") }
        if let seed { args += ["--seed", String(seed)] }
        return try JSONDecoder().decode(LookModel.self, from: try await run(args))
    }

    func look(of theme: String) async throws -> LookModel {
        try JSONDecoder().decode(LookModel.self, from: try await run(["look", "of", theme]))
    }

    func apply(_ model: LookModel) async throws {
        // The core owns the arithmetic to the device's numbers, so the model
        // goes back exactly the way it came. `look apply` reads the same shape
        // `look` prints and ignores the fields it does not need.
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent("clevertuna-look-\(UUID().uuidString).json")
        defer { try? FileManager.default.removeItem(at: url) }
        try JSONEncoder().encode(model).write(to: url)
        _ = try await run(["look", "apply", url.path])
    }

    func themes() async throws -> [ThemeSummary] {
        let text = String(data: try await run(["theme", "list"]), encoding: .utf8) ?? ""
        var group = ""
        var out: [ThemeSummary] = []
        for line in text.split(separator: "\n") {
            let parts = line.split(separator: " ", omittingEmptySubsequences: true)
            guard let kind = parts.first else { continue }
            if kind == "GROUP" {
                group = parts.dropFirst().joined(separator: " ")
            } else if kind == "THEME", parts.count > 2 {
                let id = String(parts[1])
                let rest = parts.dropFirst(2)
                let colours = rest.filter { $0.hasPrefix("#") }.map(String.init)
                let note = rest.filter { !$0.hasPrefix("#") }.joined(separator: " ")
                out.append(ThemeSummary(id: id, name: id.replacingOccurrences(of: "-", with: " ").capitalized,
                                        group: group, colours: colours, note: note))
            }
        }
        return out
    }

    func applyTheme(_ id: String) async throws {
        _ = try await run(["theme", id])
    }

    /// Read as JSON rather than parsed out of the printed table: a profile's
    /// name is whatever a person typed, spaces and all, so there is no column
    /// to split on that a name cannot contain.
    func profiles() async throws -> [ThemeSummary] {
        struct Row: Decodable {
            let name: String
            let zones: [String]
            let colors: [String]
            let shadowed: Bool
        }
        let rows = try JSONDecoder().decode([Row].self, from: try await run(["--json", "profile", "list"]))
        return rows.map { row in
            let covers = row.zones.map { zoneNames[$0] ?? $0 }.joined(separator: ", ").lowercased()
            return ThemeSummary(
                id: row.name,
                name: row.name,
                group: "Yours",
                colours: row.colors,
                note: row.shadowed
                    ? "shares its name with a theme we ship — rename it"
                    : covers)
        }
    }

    /// Saved from the model rather than from the keyboard, because the builder
    /// promises that nothing is written until you apply — and a Save that first
    /// had to write the look to the hardware would break that promise to keep
    /// a copy of it. The core turns the model into the scheme it would send,
    /// and that is what goes in the gallery.
    func saveProfile(_ name: String, from model: LookModel) async throws {
        let dir = FileManager.default.temporaryDirectory
        let modelURL = dir.appendingPathComponent("clevertuna-save-\(UUID().uuidString).json")
        let schemeURL = dir.appendingPathComponent("clevertuna-scheme-\(UUID().uuidString).json")
        defer {
            try? FileManager.default.removeItem(at: modelURL)
            try? FileManager.default.removeItem(at: schemeURL)
        }
        try JSONEncoder().encode(model).write(to: modelURL)
        try await run(["look", "preview", modelURL.path]).write(to: schemeURL)
        _ = try await run(["profile", "save", name, "--from", schemeURL.path])
    }

    func applyProfile(_ name: String) async throws {
        _ = try await run(["profile", "apply", name])
    }

    func renameProfile(_ name: String, to fresh: String) async throws {
        _ = try await run(["profile", "rename", name, fresh])
    }

    func deleteProfile(_ name: String) async throws {
        _ = try await run(["profile", "delete", name])
    }

    func layoutJSON() async throws -> Data {
        try await run(["layout"])
    }

    func preview(_ path: String) async throws -> String {
        String(data: try await run(["look", "preview", path]), encoding: .utf8) ?? ""
    }

    func info() async throws -> DeviceInfo {
        let text = String(data: try await run(["info"]), encoding: .utf8) ?? ""
        var model = "Keyboard", transport = "Connected"
        for line in text.split(separator: "\n") {
            let parts = line.split(separator: " ", maxSplits: 1).map(String.init)
            guard parts.count == 2 else { continue }
            if parts[0] == "model" { model = parts[1] }
            if parts[0] == "transport" {
                transport = parts[1] == "bluetooth" ? "Connected over Bluetooth" : "Connected over USB"
            }
        }
        return DeviceInfo(model: model, transport: transport)
    }

    func favourites() async throws -> [Int: String] {
        let text = String(data: try await run(["favourites"]), encoding: .utf8) ?? ""
        var out: [Int: String] = [:]
        // "KEY  ⌃⌥1  theme:magma", and "not set" where nothing is bound.
        for line in text.split(separator: "\n") {
            let parts = line.split(separator: " ", omittingEmptySubsequences: true).map(String.init)
            guard parts.count >= 3, parts[0] == "KEY",
                  let n = Int(parts[1].filter(\.isNumber)), (1...5).contains(n) else { continue }
            let value = parts[2...].joined(separator: " ")
            guard value != "not set" else { continue }
            // The gallery and the built-ins share one vocabulary; the prefix is
            // how the core tells them apart, and the tile only needs the name.
            out[n] = value.hasPrefix("theme:") ? String(value.dropFirst("theme:".count)) : value
        }
        return out
    }

    /// The desktop picture is resolved here, not by the core.
    ///
    /// The core has to ask AppleScript, and AppleScript cannot answer: for a
    /// wallpaper macOS supplies itself `System Events` returns `missing value`
    /// and the Finder fallback fails with -1700, so `match-wallpaper` came back
    /// "could not find the current wallpaper" — which, with no error surface in
    /// the helper menu, looked exactly like the button doing nothing. An app
    /// has `NSWorkspace`, which answers for every wallpaper including the ones
    /// that are not a file the person chose, so the app hands the core a path.
    func matchWallpaper() async throws {
        if let path = await Desktop.picturePath() {
            _ = try await run(["match-wallpaper", "--wallpaper", path])
        } else {
            _ = try await run(["match-wallpaper"])
        }
    }
}

/// What the desktop is showing.
@MainActor
enum Desktop {
    /// The picture to take the colours from, as a file the core can read.
    static func picturePath() async -> String? {
        guard let screen = NSScreen.main ?? NSScreen.screens.first else { return nil }
        guard let url = NSWorkspace.shared.desktopImageURL(for: screen) else { return nil }
        return await readable(url)
    }

    /// The same picture as something the core can open.
    ///
    /// Split out so it can be pointed at a file and checked — the desktop can
    /// only be set to one wallpaper at a time, which is a poor way to test the
    /// handling of the other kinds.
    static func readable(_ url: URL) async -> String? {
        guard FileManager.default.fileExists(atPath: url.path) else { return nil }
        guard isMovie(url) else { return url.path }
        return await still(from: url)
    }

    /// Apple's Landscape wallpapers are looping video.
    ///
    /// `sips` will not read a frame out of one, so `match-wallpaper` answered
    /// "is not a PNG this build can read" for every desktop set to one of them.
    private static func isMovie(_ url: URL) -> Bool {
        ["mov", "mp4", "m4v"].contains(url.pathExtension.lowercased())
    }

    /// One frame, written where the core can pick it up.
    ///
    /// A third of the way in rather than the first frame: these loops often
    /// open on a fade from black, and black is the one thing a palette must not
    /// be built from.
    private static func still(from url: URL) async -> String? {
        let asset = AVURLAsset(url: url)
        let generator = AVAssetImageGenerator(asset: asset)
        generator.appliesPreferredTrackTransform = true
        // Any nearby frame will do, and asking for an exact one makes it decode
        // from the last keyframe forward.
        generator.requestedTimeToleranceBefore = CMTime(seconds: 1, preferredTimescale: 600)
        generator.requestedTimeToleranceAfter = CMTime(seconds: 1, preferredTimescale: 600)

        let seconds = (try? await asset.load(.duration).seconds) ?? 0
        let at = CMTime(seconds: seconds.isFinite && seconds > 0 ? seconds / 3 : 0,
                        preferredTimescale: 600)
        guard let frame = try? await generator.image(at: at).image else { return nil }

        let out = FileManager.default.temporaryDirectory
            .appendingPathComponent("clevertuna-wallpaper-\(UUID().uuidString).png")
        let rep = NSBitmapImageRep(cgImage: frame)
        guard let png = rep.representation(using: .png, properties: [:]),
              (try? png.write(to: out)) != nil else { return nil }
        return out.path
    }
}

/// One device setting, as the core describes it.
struct DeviceSetting: Sendable, Identifiable, Hashable {
    enum Kind: Sendable, Hashable {
        case toggle
        case choice([String])
    }

    let key: String
    let label: String
    let value: String
    let group: String
    let kind: Kind
    /// A row this keyboard does not carry is shown and dimmed rather than
    /// hidden: a missing row reads as a missing feature, a dimmed one reads as
    /// the truth.
    let available: Bool
    var note: String?

    var id: String { key }
}

extension RustCoreBackend {
    func settings() async throws -> [DeviceSetting] {
        let text = String(data: try await run(["settings"]), encoding: .utf8) ?? ""
        var group = ""
        var out: [DeviceSetting] = []
        for line in text.split(separator: "\n") {
            // The core prints columns padded with runs of spaces, so split on
            // those. Guessing where the value ends by looking for a capital
            // letter turns "Medium  Left slider sensitivity" into a row named
            // "Medium Left slider sensitivity" with no value at all.
            let parts = line
                .components(separatedBy: "  ")
                .map { $0.trimmingCharacters(in: .whitespaces) }
                .filter { !$0.isEmpty }
            guard let kind = parts.first else { continue }
            if kind == "GROUP" {
                group = parts.dropFirst().joined(separator: " ")
                continue
            }
            guard kind == "SET" || kind == "OFF", parts.count >= 4 else { continue }

            let value = parts[2]
            let label = parts[3]
            let isToggle = value == "on" || value == "off"
            out.append(DeviceSetting(
                key: parts[1],
                label: label.isEmpty ? parts[1] : label,
                value: value,
                group: group,
                kind: isToggle ? .toggle : .choice(Self.choices(for: parts[1], current: value)),
                available: kind == "SET"))
        }
        return out
    }

    /// The values a setting takes. The core refuses anything else, so the
    /// pop-up must not offer it — a published option list is not the range the
    /// firmware accepts.
    private static func choices(for key: String, current: String) -> [String] {
        switch key {
        case "dominant-hand": return ["Left", "Right"]
        case "left-slider-sensitivity", "right-slider-sensitivity":
            return ["Lowest", "Low", "Medium", "High"]
        case "backlight-timeout":
            return ["never", "5 minutes", "10 minutes", "30 minutes", "1 hour"]
        case "idle-timeout":
            return ["never", "30 seconds", "1 minute", "3 minutes", "5 minutes", "10 minutes", "30 minutes"]
        default: return [current]
        }
    }

    func setSetting(_ key: String, to value: String) async throws {
        _ = try await run(["settings", key, value])
    }
}
