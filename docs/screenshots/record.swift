// record.swift <window id> <seconds> <out dir>: PNG frames of one window as it changes, at most
// 12 fps, 1280 px wide, named by their time in milliseconds.
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

final class Output: NSObject, SCStreamOutput {
    var count = 0
    let context = CIContext()
    func stream(_ stream: SCStream, didOutputSampleBuffer buffer: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen, let pixels = CMSampleBufferGetImageBuffer(buffer),
            let image = context.createCGImage(CIImage(cvPixelBuffer: pixels), from: CIImage(cvPixelBuffer: pixels).extent)
        else { return }
        count += 1
        let ms = Int(CMSampleBufferGetPresentationTimeStamp(buffer).seconds * 1000)
        let url = out.appendingPathComponent(String(format: "%012d.png", ms))
        guard let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil) else { return }
        CGImageDestinationAddImage(dest, image, nil)
        CGImageDestinationFinalize(dest)
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
config.width = 1280
config.height = Int(1280 * window.frame.height / window.frame.width)
config.sourceRect = CGRect(
    x: window.frame.minX - display.frame.minX, y: window.frame.minY - display.frame.minY,
    width: window.frame.width, height: window.frame.height)
config.backgroundColor = .clear
config.minimumFrameInterval = CMTime(value: 1, timescale: 12)
config.showsCursor = false
let output = Output()
let stream = SCStream(filter: filter, configuration: config, delegate: nil)
try stream.addStreamOutput(output, type: .screen, sampleHandlerQueue: DispatchQueue(label: "frames"))
try await stream.startCapture()
try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
try await stream.stopCapture()
print(output.count, "frames")
