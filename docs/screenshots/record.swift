// record.swift <window id> <seconds> <out dir>: JPEG frames of one window as it changes, at most
// 24 fps, at the screen's pixel size, named by their time in milliseconds.
import AppKit
import CoreImage
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

NSApplication.shared.setActivationPolicy(.prohibited)
let args = CommandLine.arguments
let windowID = CGWindowID(args[1])!
let seconds = Double(args[2])!
let out = URL(fileURLWithPath: args[3])

let writes = DispatchQueue(label: "writes", attributes: .concurrent)
let pending = DispatchGroup()

final class Output: NSObject, SCStreamOutput {
    var count = 0
    let context = CIContext()
    func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let pixels = CMSampleBufferGetImageBuffer(buffer),
            let image = context.createCGImage(CIImage(cvPixelBuffer: pixels), from: CIImage(cvPixelBuffer: pixels).extent)
        else { return }
        count += 1
        let ms = Int(CMSampleBufferGetPresentationTimeStamp(buffer).seconds * 1000)
        let url = out.appendingPathComponent(String(format: "%012d.jpg", ms))
        // Written apart, so that writing a frame never holds the next one back.
        writes.async(group: pending) {
            guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.jpeg.identifier as CFString, 1, nil)
            else { return }
            CGImageDestinationAddImage(dest, image, [kCGImageDestinationLossyCompressionQuality: 0.95] as CFDictionary)
            CGImageDestinationFinalize(dest)
        }
    }
}

let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
guard let window = content.windows.first(where: { $0.windowID == windowID }) else { fatalError("no window") }
// The display without the other apps' windows: a window captured or included by itself shows the
// screen sharing icon in place of its buttons.
guard let display = content.displays.first(where: { $0.frame.contains(window.frame.origin) })
else { fatalError("no display") }
let others = content.applications.filter { $0.processID != window.owningApplication?.processID }
let filter = SCContentFilter(display: display, excludingApplications: others, exceptingWindows: [])
let config = SCStreamConfiguration()
let scale = NSScreen.main?.backingScaleFactor ?? 2
config.width = Int(window.frame.width * scale)
config.height = Int(window.frame.height * scale)
config.sourceRect = CGRect(
    x: window.frame.minX - display.frame.minX, y: window.frame.minY - display.frame.minY,
    width: window.frame.width, height: window.frame.height)
config.backgroundColor = .clear
config.minimumFrameInterval = CMTime(value: 1, timescale: 24)
config.showsCursor = false
let output = Output()
let stream = SCStream(filter: filter, configuration: config, delegate: nil)
try stream.addStreamOutput(output, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames"))
try await stream.startCapture()
try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
try await stream.stopCapture()
pending.wait()
print(output.count, "frames")
