import SwiftUI
import AppKit

/// Clevertuna for macOS.
///
/// A menu bar app first — the helper menu is the quick surface, and most of
/// what this app does should never need a window. Builder, Themes and Settings
/// are the three things that do, and they are made in `Windows` rather than as
/// SwiftUI scenes: a scene can only be opened from a `View`, and the delegate
/// is where a URL and a launch argument arrive.
@main
struct ClevertunaApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var delegate

    var body: some Scene {
        // The quick surface: brightness and themes without opening anything.
        //
        // `.window`, not `.menu`. A menu can only be a list of words, and the
        // handoff is a device tile, a slider you drag and six themes you can
        // see. Shipping it as a menu was shipping a different design.
        MenuBarExtra("Clevertuna", systemImage: "keyboard") {
            MenuBarContent(model: AppState.shared.model)
                .environment(AppState.shared.model)
        }
        .menuBarExtraStyle(.window)
    }
}

/// One model, one device, for the life of the app.
///
/// The keyboard grants a single connection, so a second model would mean a
/// second conversation with it — and the windows all show the same keyboard,
/// so they should all be showing the same reading of it.
@MainActor
final class AppState {
    static let shared = AppState()
    let model = BuilderModel(device: RustCoreBackend())
    private init() {}
}

/// Launch arguments, URLs, and the offscreen render used to check the windows
/// against the design without a person taking a screenshot.
final class AppDelegate: NSObject, NSApplicationDelegate, AppCommands {
    func applicationDidFinishLaunching(_ note: Notification) {
        // A keyboard app has no dock icon; it lives in the menu bar. It gains
        // one for as long as a window is open — see `Windows.show` — because a
        // window with no application behind it is what left the menu bar blank.
        NSApp.setActivationPolicy(.accessory)

        let args = CommandLine.arguments

        // Writes the model exactly as the app would send it, so the file can be
        // inspected and fed to the core by hand.
        if let i = args.firstIndex(of: "--dump-look"), i + 1 < args.count {
            let out = args[i + 1]
            Task { @MainActor in
                do {
                    let m = try await RustCoreBackend().look(random: false, seed: nil)
                    let enc = JSONEncoder()
                    enc.outputFormatting = [.prettyPrinted, .sortedKeys]
                    try enc.encode(m).write(to: URL(fileURLWithPath: out))
                    print("wrote \(out)")
                    exit(0)
                } catch {
                    FileHandle.standardError.write(Data("\(error)\n".utf8))
                    exit(4)
                }
            }
            return
        }
        // What the wallpaper matcher would read, and for which file.
        //
        // With no argument: the desktop's own picture. With one: that file put
        // through the same handling, which is the only way to check a kind of
        // wallpaper the desktop is not currently set to.
        if let i = args.firstIndex(of: "--desktop-picture") {
            let given = i + 1 < args.count && !args[i + 1].hasPrefix("--") ? args[i + 1] : nil
            Task { @MainActor in
                let resolved = given.map { URL(fileURLWithPath: $0) }
                let path = resolved == nil
                    ? await Desktop.picturePath()
                    : await Desktop.readable(resolved!)
                print(path ?? "— nothing readable —")
                exit(path == nil ? 4 : 0)
            }
            return
        }
        // Whether this keyboard answers the standard Battery Service at all.
        if args.contains("--battery") {
            Task { @MainActor in
                let level = await Battery.shared.read()
                print(level.map { "\($0)%" } ?? "— this keyboard does not report a battery —")
                exit(level == nil ? 4 : 0)
            }
            return
        }
        if args.contains("--selftest") {
            Task { await SelfTest.run() }
            return
        }
        // What the app actually waits for, in milliseconds.
        //
        // The affordance shown while a control is working is supposed to last
        // as long as the wait — and "it feels like a moment" is not a number.
        // Every call here needs no keyboard, so this measures the floor: the
        // subprocess itself. Anything above it on the real hardware is the
        // Bluetooth round trip.
        if args.contains("--timings") {
            Task { @MainActor in
                // Whether the wallpaper feature can see the desktop at all.
                // AppleScript cannot, and used to fail silently; this says so
                // out loud rather than leaving it to a button that does nothing.
                print("desktop picture       " + (await Desktop.picturePath() ?? "— not found —"))
                let device = RustCoreBackend()
                var rows: [(String, Double)] = []
                var mark = Date()
                func lap(_ name: String) {
                    rows.append((name, Date().timeIntervalSince(mark) * 1000))
                    mark = Date()
                }
                _ = try? await device.look(random: true, seed: 1);        lap("look random")
                _ = try? await device.look(of: "magma");                  lap("look of magma")
                _ = try? await device.themes();                           lap("theme list")
                _ = try? await device.profiles();                         lap("profile list")
                _ = try? await device.favourites();                       lap("favourites")
                _ = try? await device.info();                             lap("info · keyboard")
                _ = try? await device.look(random: false, seed: nil);     lap("look · keyboard")
                for (name, ms) in rows {
                    print(name.padding(toLength: 22, withPad: " ", startingAt: 0)
                          + String(format: "%8.1f ms", ms))
                }
                exit(0)
            }
            return
        }
        if let i = args.firstIndex(of: "--print-builder"), i + 1 < args.count {
            Snapshot.write(to: args[i + 1], args: args)
            return
        }

        Task { @MainActor in
            Windows.shared.attach(AppState.shared.model)
            // One read at launch, so the menu has something to show the moment
            // it is opened rather than a frame of nothing.
            await AppState.shared.model.loadMenu()
            if args.contains("--builder") { Windows.shared.show(.builder) }
        }
    }

    // MARK: - What the menu bar's own items do

    func openBuilder(_ sender: Any?) { open(.builder) }
    func openThemes(_ sender: Any?) { open(.themes) }
    func openSettings(_ sender: Any?) { open(.settings) }

    func reread(_ sender: Any?) {
        Task { @MainActor in await AppState.shared.model.load() }
    }

    private func open(_ surface: Windows.Surface) {
        Task { @MainActor in
            Windows.shared.attach(AppState.shared.model)
            Windows.shared.show(surface)
        }
    }

    /// `clevertuna://builder`, `://themes`, `://settings`.
    ///
    /// macOS hands a URL to the instance already running; launch arguments are
    /// dropped for an app that is already up, which for a menu bar app is
    /// nearly always.
    func application(_ application: NSApplication, open urls: [URL]) {
        for url in urls where url.scheme == "clevertuna" {
            let what = (url.host ?? "") + url.path.replacingOccurrences(of: "/", with: "")
            let surface = Windows.Surface(rawValue: what) ?? .builder
            Task { @MainActor in
                Windows.shared.attach(AppState.shared.model)
                Windows.shared.show(surface)
            }
        }
    }
}
