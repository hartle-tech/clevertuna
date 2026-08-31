import AppKit
import SwiftUI

/// A target for `NSColorPanel`, which is Objective-C and wants a selector.
///
/// The system colour picker is the one place a stock control is the right
/// answer: it is the picker people already know, with their own palettes in it.
final class ColourSink: NSObject {
    private let onChange: (Color) -> Void

    init(onChange: @escaping (Color) -> Void) {
        self.onChange = onChange
    }

    @objc func changed(_ sender: NSColorPanel) {
        onChange(Color(nsColor: sender.color))
    }
}
