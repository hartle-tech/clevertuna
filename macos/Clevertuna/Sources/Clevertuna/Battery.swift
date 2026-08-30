import Foundation
import CoreBluetooth

/// How much charge the keyboard has left.
///
/// Not from the protocol: the vendor interface carries a *battery saving*
/// setting and no level, and macOS publishes no percentage for this device
/// either — `system_profiler` lists it with no battery line and there is no
/// `BatteryPercent` on its IOKit node. What it does have is the standard BLE
/// Battery Service, which any client may read, so that is where this comes
/// from — read over the connection macOS already holds, not a new one.
///
/// Everything here is best-effort and quiet: a keyboard on the cable has no
/// battery service to answer with, and a menu that shouts about that is worse
/// than a menu that simply does not show a figure.
@MainActor
final class Battery: NSObject {
    static let shared = Battery()

    private static let service = CBUUID(string: "180F")
    private static let level = CBUUID(string: "2A19")

    private var central: CBCentralManager?
    private var keyboard: CBPeripheral?
    private var waiting: CheckedContinuation<Int?, Never>?
    /// The last figure read, so the menu has something the instant it opens.
    private(set) var percent: Int?

    /// Read the level, or nil if this keyboard will not say.
    ///
    /// Times out on a clock rather than on an attempt count — the same rule the
    /// rest of the Bluetooth code follows, because a first packet can be far
    /// slower than the gaps inside one reply.
    func read(timeout: Duration = .seconds(6)) async -> Int? {
        guard waiting == nil else { return percent }
        if central == nil { central = CBCentralManager(delegate: self, queue: .main) }

        // One continuation, and a clock that resumes it if nothing else does.
        let deadline = Task { @MainActor in
            try? await Task.sleep(for: timeout)
            self.finish(nil)
        }
        let found = await withCheckedContinuation { (c: CheckedContinuation<Int?, Never>) in
            self.waiting = c
            self.begin()
        }
        deadline.cancel()
        if let found { percent = found }
        return percent
    }

    private func begin() {
        guard let central, central.state == .poweredOn else { return }
        // The connection macOS already has. Scanning would be a second one, and
        // this keyboard grants one at a time.
        let connected = central.retrieveConnectedPeripherals(withServices: [Self.service])
        guard let peripheral = connected.first else { return finish(nil) }
        keyboard = peripheral
        peripheral.delegate = self
        if peripheral.state == .connected {
            peripheral.discoverServices([Self.service])
        } else {
            central.connect(peripheral)
        }
    }

    private func finish(_ value: Int?) {
        guard let waiting else { return }
        self.waiting = nil
        waiting.resume(returning: value)
    }
}

extension Battery: @MainActor CBCentralManagerDelegate {
    func centralManagerDidUpdateState(_ central: CBCentralManager) {
        if central.state == .poweredOn { begin() } else { finish(nil) }
    }

    func centralManager(_ central: CBCentralManager, didConnect peripheral: CBPeripheral) {
        peripheral.discoverServices([Self.service])
    }

    func centralManager(_ central: CBCentralManager, didFailToConnect peripheral: CBPeripheral,
                        error: Error?) {
        finish(nil)
    }
}

extension Battery: @MainActor CBPeripheralDelegate {
    func peripheral(_ peripheral: CBPeripheral, didDiscoverServices error: Error?) {
        guard let service = peripheral.services?.first(where: { $0.uuid == Self.service }) else {
            return finish(nil)
        }
        peripheral.discoverCharacteristics([Self.level], for: service)
    }

    func peripheral(_ peripheral: CBPeripheral, didDiscoverCharacteristicsFor service: CBService,
                    error: Error?) {
        guard let c = service.characteristics?.first(where: { $0.uuid == Self.level }) else {
            return finish(nil)
        }
        peripheral.readValue(for: c)
    }

    func peripheral(_ peripheral: CBPeripheral, didUpdateValueFor characteristic: CBCharacteristic,
                    error: Error?) {
        guard characteristic.uuid == Self.level,
              let byte = characteristic.value?.first else { return finish(nil) }
        finish(Int(byte))
    }
}
