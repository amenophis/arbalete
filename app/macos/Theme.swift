// A Space's colors over the window (ui/src/theme.ts).

import AppKit
import SwiftUI

/// Whether a theme shows light text: as its mode forces, or as its colors are, and as the system
/// is without colors.
@MainActor func isDark(_ theme: Theme?) -> Bool {
    guard let theme else { return systemDark() }
    if theme.mode != "auto" { return theme.mode == "dark" }
    if theme.colors.isEmpty { return systemDark() }
    let total = theme.colors.reduce(0) { $0 + luminance($1) }
    return total / Double(theme.colors.count) < 0.4
}

@MainActor func systemDark() -> Bool {
    NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua
}

/// A #rrggbb color's red, green and blue, from 0 to 1.
func channels(_ hex: String) -> [Double] {
    let value = UInt32(hex.dropFirst(), radix: 16) ?? 0
    return [16, 8, 0].map { Double((value >> $0) & 0xff) / 255 }
}

/// The relative luminance of a #rrggbb color, from 0 to 1.
func luminance(_ hex: String) -> Double {
    let linear = channels(hex).map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

/// `share` of `color` mixed into `base`, both #rrggbb.
func mix(_ color: String, _ base: String, _ share: Double) -> String {
    let (from, to) = (channels(color), channels(base))
    let mixed = zip(from, to).map { Int((($0 * share + $1 * (1 - share)) * 255).rounded()) }
    return "#" + mixed.map { String(format: "%02x", $0) }.joined()
}

func color(_ hex: String) -> Color {
    let rgb = channels(hex)
    return Color(red: rgb[0], green: rgb[1], blue: rgb[2])
}

/// A theme's colors over the blur: a gradient from the top left corner to the bottom right one,
/// a third color rising from the bottom left corner, the stronger the more intense.
struct Tint: View {
    let theme: Theme?

    var body: some View {
        if let theme, let first = theme.colors.first {
            let second = theme.colors.count > 1 ? theme.colors[1] : first
            ZStack {
                LinearGradient(
                    colors: [color(first), color(second)], startPoint: .topLeading,
                    endPoint: .bottomTrailing)
                if theme.colors.count > 2 {
                    LinearGradient(
                        colors: [color(theme.colors[2]).opacity(0), color(theme.colors[2])],
                        startPoint: .bottomLeading, endPoint: .topTrailing)
                }
            }
            .opacity(0.15 + 0.7 * theme.intensity)
        }
    }
}
