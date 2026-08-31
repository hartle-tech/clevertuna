import AppKit

/// The menu bar a focused window is supposed to bring with it.
///
/// This app is `LSUIElement`, and an accessory app that activates **without a
/// main menu leaves the menu bar empty** — so opening the builder over a
/// fullscreen app looked exactly like macOS's own menu bar had been dismissed.
/// It also meant the app had no ⌘Q, no ⌘W, and no ⌘C/⌘V in the one place it
/// asks you to type: the name field in the Save and Rename sheets.
///
/// Every item here is a standard responder-chain action, so the menu works on
/// whatever is in front without this file knowing anything about it.
/// **SwiftUI owns the main menu, so this adds to it rather than sets it.**
///
/// An earlier version built the whole bar in `applicationDidFinishLaunching` and
/// guarded on `NSApp.mainMenu == nil`. SwiftUI's `App` installs its own bar
/// *after* the delegate runs, so that guard was either true and then overwritten
/// or false and skipped — either way the Keyboard menu never appeared, and the
/// self-test is what found it. SwiftUI already gives Clevertuna / Edit / View /
/// Window / Help, and with them ⌘Q, ⌘W and ⌘V; all that is missing is the app's
/// own commands. So: insert one menu, once, whenever the bar is next needed.
enum MainMenu {
    static func install() {
        guard let bar = NSApp.mainMenu else { return }
        let title = "Keyboard"
        guard !bar.items.contains(where: { $0.submenu?.title == title }) else { return }

        let device = NSMenu(title: title)
        device.addItem(withTitle: "Theme Builder",
                       action: #selector(AppCommands.openBuilder(_:)), keyEquivalent: "1")
        device.addItem(withTitle: "Themes",
                       action: #selector(AppCommands.openThemes(_:)), keyEquivalent: "2")
        device.addItem(withTitle: "Settings",
                       action: #selector(AppCommands.openSettings(_:)), keyEquivalent: ",")
        device.addItem(.separator())
        device.addItem(withTitle: "Read the Keyboard",
                       action: #selector(AppCommands.reread(_:)), keyEquivalent: "r")

        let item = NSMenuItem()
        item.submenu = device
        // After the application menu, where an app's own commands belong.
        bar.insertItem(item, at: min(1, bar.items.count))

        // ⌘W, which SwiftUI does not provide here.
        //
        // Its Window menu is built for the scenes an app declares, and the only
        // scene here is the menu bar extra — so a window made by hand got a
        // Window menu with nothing in it that closes anything. Found by the
        // self-test, not by looking at the menu.
        if let window = bar.items.first(where: { $0.submenu?.title == "Window" })?.submenu,
           !window.items.contains(where: { $0.keyEquivalent == "w" }) {
            let close = NSMenuItem(title: "Close",
                                   action: #selector(NSWindow.performClose(_:)),
                                   keyEquivalent: "w")
            window.insertItem(close, at: 0)
            window.insertItem(.separator(), at: 1)
        }
    }
}

/// The menu's own actions, on an object that is always in the responder chain.
///
/// A menu item is disabled unless something answers its selector, so these live
/// on the app delegate rather than on a view that may not be on screen.
@MainActor
@objc protocol AppCommands {
    @objc func openBuilder(_ sender: Any?)
    @objc func openThemes(_ sender: Any?)
    @objc func openSettings(_ sender: Any?)
    @objc func reread(_ sender: Any?)
}
