import SwiftUI
import AppKit

/// Render the builder and write it to a PNG.
///
/// A menu bar app cannot be photographed the ordinary way — there is no dock
/// icon to click and the status item is not screenshottable — and the standing
/// rule is to check UI changes without asking a person for a screenshot.
///
/// It has to be a **real window**, captured through the window server.
/// `ImageRenderer` rasterises a view tree on its own, which means it never
/// sees the compositor: `glassEffect` comes out as a flat fill, and the whole
/// point of this app is the glass. So the window is built, shown, and captured
/// with `CGWindowListCreateImage`, which photographs what was actually drawn.
enum Snapshot {
    /// Which surface was asked for, needed in two places.
    static func surfaceName(_ args: [String]) -> String {
        args.firstIndex(of: "--surface").flatMap { i -> String? in
            i + 1 < args.count ? args[i + 1] : nil
        } ?? "builder"
    }

    @MainActor
    static func write(to path: String, args: [String]) {
        let device = RustCoreBackend()
        let model = BuilderModel(device: device)

        // `--zone` renders it opened on a particular zone, which is the only
        // way to check the controls a touch slider gets without clicking one.
        if let z = args.firstIndex(of: "--zone"), z + 1 < args.count {
            let named = ["keys": "keyboard", "pad": "touchpad", "touchpad": "touchpad",
                         "left": "leftSlider", "right": "rightSlider"]
            model.selectedZone = named[args[z + 1].lowercased()] ?? args[z + 1]
        }
        // `--theme` opens the Themes window on a particular one, which is the
        // only way to check what one of your own saved themes looks like there
        // without clicking it.
        if let t = args.firstIndex(of: "--theme"), t + 1 < args.count {
            model.currentTheme = args[t + 1]
        }
        let random = args.contains("--roll")
        let seed = args.firstIndex(of: "--seed").flatMap { i -> Int? in
            i + 1 < args.count ? Int(args[i + 1]) : nil
        }

        Task { @MainActor in
            // No fallback on purpose. An earlier version quietly substituted a
            // sample look when the read failed, which turned a decoding bug —
            // the model asked for `colors` where the core says `stops` — into
            // the words "no keyboard", and sent the search after Bluetooth
            // permissions for an hour. A harness that hides its own failure is
            // worse than no harness.
            do {
                // `--theme` on the builder renders it wearing a named theme
                // rather than a roll of the dice. A screenshot is a picture of
                // the product, and letting chance pick its palette is how a
                // page ends up advertising a colour scheme nobody chose.
                if let t = args.firstIndex(of: "--theme"), t + 1 < args.count,
                   surfaceName(args) == "builder" {
                    model.look = try await device.look(of: args[t + 1])
                    model.pinned = true
                } else {
                    model.look = try await device.look(random: random, seed: seed)
                    // A rolled look is the subject of the render: hold it
                    // against the read the builder does when it appears, or a
                    // render "at seed 7" is really a render of whatever the
                    // keyboard happens to be showing.
                    model.pinned = random || seed != nil
                }
            } catch {
                FileHandle.standardError.write(
                    Data("could not read the keyboard: \(error.localizedDescription)\n".utf8))
                // A sheet draws no keyboard, so a sheet does not need one — and
                // refusing to photograph a dialog because a keyboard is asleep
                // is a harness getting in its own way. Every other surface still
                // exits: those *do* draw the look, and a render that quietly
                // substituted a sample once cost an hour chasing Bluetooth
                // permissions that were never the problem.
                guard surfaceName(args) == "prompt" else { exit(4) }
                FileHandle.standardError.write(
                    Data("continuing anyway: a sheet does not draw the keyboard\n".utf8))
            }

            // Which surface, and how big it is in the handoff.
            let surface = surfaceName(args)
            // Which sheet, needed before the window is sized: the slot copy
            // asks about four zones and a switch, and a window cut to the
            // height of the shortest prompt hides the buttons on the tallest.
            let kind = args.firstIndex(of: "--prompt").flatMap { i -> String? in
                i + 1 < args.count ? args[i + 1] : nil
            } ?? "save"
            let size: NSSize
            switch surface {
            // Exactly the sheet, which is 380 wide. A window any wider leaves
            // black margins down both sides of the capture, and the traffic
            // lights sit in the left one — a sheet photographed inside a window
            // it does not fill looks like a mistake, because it is one.
            case "prompt": size = NSSize(width: 380, height: kind == "slot" ? 540 : 250)
            case "menu": size = NSSize(width: 400, height: 700)
            case "themes": size = NSSize(width: 900, height: 620)
            case "settings": size = NSSize(width: 900, height: 640)
            default: size = NSSize(width: 1080, height: 760)
            }
            // A sheet is not a window: it has no title bar of its own, so
            // capturing one inside a titled window photographs chrome that
            // does not exist in the app.
            let bare = surface == "prompt"
            let window = NSWindow(
                contentRect: NSRect(origin: .zero, size: size),
                styleMask: bare ? [.borderless] : [.titled, .closable, .fullSizeContentView],
                backing: .buffered, defer: false)
            window.titlebarAppearsTransparent = true
            window.titleVisibility = .hidden
            window.backgroundColor = bare ? .clear : .black
            window.isOpaque = !bare
            window.hasShadow = !bare
            let root: AnyView
            switch surface {
            case "prompt":
                // The sheets are only reachable by clicking, and the rule here
                // is to check a UI change without asking a person to look — so
                // each one can be shown on its own.
                let asked: Prompt
                switch kind {
                case "rename": asked = .rename("Lemon Pop")
                case "remove": asked = .remove("Lemon Pop")
                case "copy": asked = .copyZone
                case "slot": asked = .copySlot
                default: asked = .save
                }
                root = AnyView(PromptSheet(prompt: asked) {}
                    .environment(model)
                    .frame(width: size.width, height: size.height)
                    // Its own corners, since there is no window drawing them.
                    .clipShape(RoundedRectangle(cornerRadius: 14, style: .continuous)))
            case "menu":
                root = AnyView(MenuBarContent(model: model)
                    .environment(model)
                    .frame(width: size.width, height: size.height, alignment: .top)
                    .background(Color(white: 0.13)))
            case "themes":
                root = AnyView(ThemesView().environment(model)
                    .frame(width: size.width, height: size.height))
            case "settings":
                root = AnyView(SettingsView().environment(model)
                    .frame(width: size.width, height: size.height))
            default:
                root = AnyView(BuilderView().environment(model)
                    .frame(width: size.width, height: size.height))
            }
            window.contentView = NSHostingView(rootView: root)

            // On screen, because glass is drawn by the compositor and a window
            // that never displays is never composited. Placed at the origin so
            // it is fully on the display and nothing is clipped.
            window.setFrameOrigin(NSPoint(x: 40, y: 40))
            window.orderFrontRegardless()
            window.displayIfNeeded()

            // Two beats: one for layout, one for the glass to settle.
            await model.loadMenu()
            await model.loadSettings()
            try? await Task.sleep(for: .milliseconds(1200))

            // `--bench <seconds>`: what this window costs to keep on screen.
            //
            // The deck's cost is a claim that has to be measured, and measuring
            // it by eye on Activity Monitor is neither repeatable nor something
            // a later session can re-run. `look random --seed` needs no
            // keyboard, so this works with the CLVX unplugged.
            let bench = args.firstIndex(of: "--bench").flatMap { i -> Double? in
                i + 1 < args.count ? Double(args[i + 1]) : nil
            }
            if bench != nil, !model.isAnimated {
                // A still deck pauses the clock and costs nothing, so a bench
                // run over one would print a number that looks like a win and
                // measures nothing. Roll a seed whose keys actually move.
                FileHandle.standardError.write(Data(
                    "this look does not animate — a bench over it measures nothing\n".utf8))
                exit(5)
            }

            // `screencapture -l` does the photographing. CGWindowListCreateImage
            // is gone in macOS 26 and ScreenCaptureKit wants an async session
            // with its own permission; the command line tool already has the
            // grant and captures the same composited pixels.
            let shot = Process()
            shot.executableURL = URL(fileURLWithPath: "/usr/sbin/screencapture")
            shot.arguments = ["-l", String(window.windowNumber), "-o", "-x", path]
            do {
                try shot.run()
                shot.waitUntilExit()
            } catch {
                FileHandle.standardError.write(Data("could not run screencapture: \(error)\n".utf8))
                exit(4)
            }
            guard shot.terminationStatus == 0,
                  FileManager.default.fileExists(atPath: path) else {
                FileHandle.standardError.write(Data("screencapture wrote nothing\n".utf8))
                exit(4)
            }
            print("wrote \(path)")
            if let bench { await measure(seconds: bench) }
            exit(0)
        }
    }

    /// Hold the window up and report what it burns.
    ///
    /// `getrusage` counts every thread of this process, which is what
    /// `ps -Ao pcpu` reports too, so the figure is comparable with the 33% the
    /// handoff recorded. The sleep is `async`, so the run loop keeps drawing.
    @MainActor
    private static func measure(seconds: Double) async {
        func cpuSeconds() -> Double {
            var u = rusage()
            getrusage(RUSAGE_SELF, &u)
            return Double(u.ru_utime.tv_sec) + Double(u.ru_utime.tv_usec) / 1e6
                 + Double(u.ru_stime.tv_sec) + Double(u.ru_stime.tv_usec) / 1e6
        }
        // A beat first: launch, the first read and the capture all cost, and
        // none of them is what is being measured.
        try? await Task.sleep(for: .milliseconds(500))
        let cpu0 = cpuSeconds()
        let wall0 = Date()
        try? await Task.sleep(for: .seconds(seconds))
        let used = cpuSeconds() - cpu0
        let wall = Date().timeIntervalSince(wall0)
        print(String(format: "bench  %.1f%% of a core over %.1fs (%.2fs cpu)",
                     used / wall * 100, wall, used))
    }
}
