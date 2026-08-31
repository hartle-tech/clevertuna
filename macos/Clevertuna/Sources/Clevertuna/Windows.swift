import SwiftUI
import AppKit

/// The app's windows, made and shown by hand.
///
/// SwiftUI's `Window` scenes are created lazily by `openWindow(id:)`, which is
/// only reachable from a `View` — so the delegate, which is where a URL and a
/// launch argument arrive, had no way to open one. The workaround was to open
/// `clevertuna://builder-open`, which the delegate handled by asking for the
/// window again, which opened the URL again: a loop that made the screen flash
/// and the builder unusable.
///
/// Owning the windows removes the whole class of problem. There is one place a
/// window is made, one place it is shown, and nothing recursive between them.
@MainActor
final class Windows {
    static let shared = Windows()

    enum Surface: String, CaseIterable {
        case builder, themes, settings

        var title: String {
            switch self {
            case .builder: return "Theme Builder"
            case .themes: return "Themes"
            case .settings: return "Settings"
            }
        }

        var size: NSSize {
            switch self {
            case .builder: return NSSize(width: 1080, height: 760)
            case .themes: return NSSize(width: 900, height: 620)
            case .settings: return NSSize(width: 900, height: 640)
            }
        }

        var minSize: NSSize {
            switch self {
            case .builder: return NSSize(width: 1000, height: 720)
            case .themes: return NSSize(width: 860, height: 580)
            case .settings: return NSSize(width: 860, height: 600)
            }
        }
    }

    private var windows: [Surface: NSWindow] = [:]
    private weak var model: BuilderModel?
    private let watcher = WindowWatcher()

    func attach(_ model: BuilderModel) { self.model = model }

    func show(_ surface: Surface) {
        if let existing = windows[surface] {
            promote()
            place(existing)
            existing.makeKeyAndOrderFront(nil)
            return
        }
        // Before promoting, not after: a show that cannot go on used to leave
        // the app activated with a dock icon and nothing on screen.
        guard let model else { return }

        let window = NSWindow(
            contentRect: NSRect(origin: .zero, size: surface.size),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered, defer: false)
        window.title = surface.title
        window.titlebarAppearsTransparent = true
        window.titleVisibility = .hidden
        // **Not** movable by its background.
        //
        // These windows are made of sliders, colour wells and a dial, and a
        // window that moves when you drag its background moves when you drag
        // any of them: the thumb stayed put and the whole window slid across
        // the desk instead. The title bar is what a window is dragged by, and
        // in the builder that bar is the toolbar — see `BuilderView`.
        window.isMovableByWindowBackground = false
        window.minSize = surface.minSize
        window.backgroundColor = .black
        // Closing a window puts it away; it is not the app quitting, and the
        // next open should be instant rather than a rebuild.
        window.isReleasedWhenClosed = false

        let root: AnyView
        switch surface {
        case .builder: root = AnyView(BuilderView().environment(model))
        case .themes: root = AnyView(ThemesView().environment(model))
        case .settings: root = AnyView(SettingsView().environment(model))
        }
        window.contentView = NSHostingView(rootView: root)
        window.delegate = watcher
        window.center()

        windows[surface] = window
        promote()
        place(window)
        window.makeKeyAndOrderFront(nil)
    }

    /// A window on screen makes this a regular application for as long as it is
    /// there.
    ///
    /// An `.accessory` app contributes no menu bar when it activates, so
    /// focusing the builder over a fullscreen app left the bar blank — which
    /// reads as Clevertuna having dismissed macOS's own status bar, and took
    /// ⌘W, ⌘Q and ⌘V in the sheets' name fields with it. Becoming regular while
    /// a window is up is what every other application does; the dock icon goes
    /// away again with the last window, in `closed`.
    private func promote() {
        NSApp.setActivationPolicy(.regular)
        // Here rather than at launch: SwiftUI installs its own bar after the
        // delegate has run, so anything added there is thrown away.
        MainMenu.install()
        NSApp.activate(ignoringOtherApps: true)
    }

    /// Opened from the menu bar, which is reachable from inside a fullscreen
    /// app — so the window comes up on whatever Space is in front. Without this
    /// it lands on the desktop Space and the person who clicked sees nothing.
    private func place(_ window: NSWindow) {
        window.collectionBehavior.insert(.moveToActiveSpace)
        window.collectionBehavior.insert(.fullScreenAuxiliary)
    }

    /// Put a surface away, the way its close button does.
    func close(_ surface: Surface) {
        windows[surface]?.performClose(nil)
    }

    /// Whether a surface is on screen.
    func isUp(_ surface: Surface) -> Bool {
        windows[surface]?.isVisible ?? false
    }

    /// The last window going away puts the dock icon away with it.
    fileprivate func closed(_ window: NSWindow) {
        let stillUp = windows.values.contains { $0 !== window && $0.isVisible }
        if !stillUp { retire() }
    }

    /// Go back to being a menu bar app.
    ///
    /// **`setActivationPolicy(.accessory)` is refused while this app is still
    /// the active one** — it returns `false` and the dock icon simply stays,
    /// with no error anywhere. So: stand aside, then keep asking until it takes.
    /// The self-test is what caught this; a single call looked like it worked.
    private func retire(attempt: Int = 0) {
        guard NSApp.activationPolicy() != .accessory else { return }
        NSApp.deactivate()
        if NSApp.setActivationPolicy(.accessory) { return }
        guard attempt < 40 else { return }
        Task { @MainActor in
            try? await Task.sleep(for: .milliseconds(50))
            self.retire(attempt: attempt + 1)
        }
    }
}

/// Windows are put away rather than destroyed, so the one thing worth knowing
/// about a close is that it happened.
@MainActor
final class WindowWatcher: NSObject, NSWindowDelegate {
    func windowWillClose(_ notification: Notification) {
        guard let window = notification.object as? NSWindow else { return }
        // After this run loop turn, so `isVisible` reports the close.
        Task { @MainActor in Windows.shared.closed(window) }
    }
}
