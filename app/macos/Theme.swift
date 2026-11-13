// A Space's colors over the window.

import AppKit
import SwiftUI

/// Whether a theme shows light text, like Arc's: as its mode forces; in automatic appearance, as
/// its leading color holds the palette's dark title better than its light one; without colors, as
/// the system's appearance is.
@MainActor func isDark(_ theme: Theme?) -> Bool {
    guard let theme, let lead = theme.colors.first else { return systemDark() }
    if theme.mode != "auto" { return theme.mode == "dark" }
    let titles = ThemePalette(lead).titles
    return contrast(lead, titles.dark) >= contrast(lead, titles.light)
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

/// The WCAG contrast between two #rrggbb colors, from 1 to 21.
func contrast(_ a: String, _ b: String) -> Double {
    let (x, y) = (luminance(a), luminance(b))
    return (max(x, y) + 0.05) / (min(x, y) + 0.05)
}

/// A theme's palette from its leading color, like Arc's (PrimaryColorPalette): the color made
/// lighter or darker, by steps of 0.01 of its Display P3 lightness, until its luminance falls in
/// a band.
struct ThemePalette {
    var tintedLight: String
    var shadedDark: String

    init(_ lead: String) {
        tintedLight = ThemePalette.within(lead, 0.85, 0.95)
        shadedDark = ThemePalette.within(lead, 0, 0.1)
    }

    /// The tabs' titles, over a light and over a dark background (SemanticColorPalette.title):
    /// the dark shade at 30% over black, and at 15% over white.
    var titles: (light: String, dark: String) {
        (mix(shadedDark, "#000000", 0.3), mix(shadedDark, "#ffffff", 0.15))
    }

    /// A picked row's tile over a light background (SemanticColorPalette.focus), and a hovered
    /// one's over a dark background (hover), as Arc derives them (themeSemanticColorPalette): the
    /// dark shade at half over white; the dark shade at 35% over white at 20%.
    var focus: Color { over(shadedDark, 0.5, "#ffffff") }
    var hover: Color { over(shadedDark, 0.35, "#ffffff", 0.2) }

    /// The color that stands out most on the theme's background (maxContrastColor), which marks
    /// where a drop lands.
    func strongest(dark: Bool) -> String { dark ? tintedLight : shadedDark }

    private static func within(_ hex: String, _ low: Double, _ high: Double) -> String {
        var hsl = toHsl(hex)
        // Luminance as Arc measures it, on the Display P3 components.
        func lum(_ hsl: Hsl) -> Double {
            let p3 = p3Channels(hsl).map {
                $0 <= 0.03928 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4)
            }
            return 0.2126 * p3[0] + 0.7152 * p3[1] + 0.0722 * p3[2]
        }
        if lum(hsl) < low {
            while lum(hsl) < min(low, 1), hsl.l < 1 { hsl.l = min(1, hsl.l + 0.01) }
        } else if lum(hsl) > high {
            while lum(hsl) > max(high, 0), hsl.l > 0 { hsl.l = max(0, hsl.l - 0.01) }
        } else {
            return hex
        }
        return fromHsl(hsl)
    }
}

/// `share` of `color` mixed into `base`, both #rrggbb.
func mix(_ color: String, _ base: String, _ share: Double) -> String {
    let (from, to) = (channels(color), channels(base))
    let mixed = zip(from, to).map { Int((($0 * share + $1 * (1 - share)) * 255).rounded()) }
    return "#" + mixed.map { String(format: "%02x", $0) }.joined()
}

/// `top` at `alpha` over `bottom` at `under`, both #rrggbb, composited as Arc composites its
/// palette's colors: in sRGB, the result as opaque as both together.
func over(_ top: String, _ alpha: Double, _ bottom: String, _ under: Double = 1) -> Color {
    let opacity = alpha + under * (1 - alpha)
    let rgb = zip(channels(top), channels(bottom)).map {
        ($0 * alpha + $1 * under * (1 - alpha)) / opacity
    }
    return Color(red: rgb[0], green: rgb[1], blue: rgb[2], opacity: opacity)
}

func color(_ hex: String) -> Color {
    let rgb = channels(hex)
    return Color(red: rgb[0], green: rgb[1], blue: rgb[2])
}

/// Arc's dark window without theme colors.
private let defaultViolet = "#100a1c"
/// Its light counterpart, measured on Arc's while it falls flat.
private let defaultLavender = "#eeeafe"

/// A theme's colors over the blur, like Arc's: the colors at 20% to 75% as the theme is more
/// intense, over a wash as opaque up to half, white, or black at a third of that in a dark
/// appearance; and grain, multiplied, fainter while the app is in the background. Two colors
/// make a gradient from the top left corner, whole from three quarters of the way; a third one
/// rises from the bottom edge. Without colors, a dark window is Arc's deep violet.
struct Tint: View {
    let theme: Theme?
    /// Whether the app is in front, which shows the blur.
    var active = true
    /// The system's appearance, an input so that the view follows its changes: SwiftUI draws a
    /// view again only once its inputs change.
    var systemIsDark = systemDark()

    var body: some View {
        TintLook(theme, active: active, systemIsDark: systemIsDark).view
    }

    /// A theme's colors alone, as the window shows them.
    static func colors(_ colors: [String]) -> some View {
        TintLook(
            Theme(colors: colors, mode: "auto", intensity: 1, noise: 0), active: true,
            systemIsDark: false
        ).colors
    }
}

/// What a Tint draws, as numbers, so that two themes' blend, like Arc's between Spaces, goes
/// through the colors between theirs rather than a crossfade, which dims the window midway.
struct TintLook {
    /// The wash under the colors, and how opaque it is.
    var wash: [Double]
    var washOpacity: Double
    /// The gradient's first and second colors, and the one rising from the bottom edge, as red,
    /// green and blue from 0 to 1, with how much the latter shows.
    var stops: [[Double]]
    var rising: Double
    /// How opaque the colors are, and the grain over them.
    var opacity: Double
    var grain: Double

    init(_ theme: Theme?, active: Bool, systemIsDark: Bool) {
        if let theme, let lead = theme.colors.first {
            let alpha = 0.2 + 0.55 * theme.intensity
            let under = min(alpha, 0.5)
            wash = systemIsDark ? [0, 0, 0] : [1, 1, 1]
            washOpacity = systemIsDark ? 0.35 * under : under
            // One color is a gradient from it to itself; two, without the rising one.
            let second = theme.colors.count > 1 ? theme.colors[1] : lead
            stops = [lead, second, theme.colors.count > 2 ? theme.colors[2] : second].map(channels)
            rising = theme.colors.count > 2 ? 1 : 0
            opacity = alpha
            grain = theme.noise * 0.8 * (active ? 1 : 0.25)
        } else {
            // Three quarters over the blur, measured, and whole once it falls flat.
            let flat = channels(systemIsDark ? defaultViolet : defaultLavender)
            wash = flat
            washOpacity = 0
            stops = [flat, flat, flat]
            rising = 0
            opacity = active ? 0.75 : 1
            grain = 0
        }
    }

    /// `share` of the way from this look to `other`.
    func blended(with other: TintLook, _ share: Double) -> TintLook {
        func mix(_ a: Double, _ b: Double) -> Double { a + (b - a) * share }
        func mix(_ a: [Double], _ b: [Double]) -> [Double] { zip(a, b).map { mix($0, $1) } }
        var look = self
        look.wash = mix(wash, other.wash)
        look.washOpacity = mix(washOpacity, other.washOpacity)
        look.stops = zip(stops, other.stops).map { mix($0, $1) }
        look.rising = mix(rising, other.rising)
        look.opacity = mix(opacity, other.opacity)
        look.grain = mix(grain, other.grain)
        return look
    }

    /// The colors alone, without the wash, the grain or their opacity.
    var colors: some View {
        ZStack {
            LinearGradient(
                stops: [
                    .init(color: paint(stops[0]), location: 0),
                    .init(color: paint(stops[1]), location: 0.75),
                ], startPoint: .topLeading, endPoint: .bottomTrailing)
            LinearGradient(
                colors: [paint(stops[2]).opacity(0), paint(stops[2])], startPoint: .top,
                endPoint: .bottom
            )
            .opacity(rising)
        }
        .drawingGroup(colorMode: .extendedLinear)
    }

    private func paint(_ rgb: [Double]) -> Color { Color(red: rgb[0], green: rgb[1], blue: rgb[2]) }

    var view: some View {
        ZStack {
            paint(wash).opacity(washOpacity)
            colors.opacity(opacity)
            if grain > 0 {
                Image(nsImage: grainTile).resizable(resizingMode: .tile)
                    .blendMode(.multiply)
                    .opacity(grain)
            }
        }
    }
}

/// Grain of our own, a tile of gray noise, which multiplies the colors like Arc's.
private let grainTile: NSImage = {
    let size = 128
    var generator = SystemRandomNumberGenerator()
    let pixels = (0..<size * size).map { _ in UInt8.random(in: 180...255, using: &generator) }
    let provider = CGDataProvider(data: Data(pixels) as CFData)!
    let image = CGImage(
        width: size, height: size, bitsPerComponent: 8, bitsPerPixel: 8, bytesPerRow: size,
        space: CGColorSpaceCreateDeviceGray(), bitmapInfo: [], provider: provider, decode: nil,
        shouldInterpolate: false, intent: .defaultIntent)!
    return NSImage(cgImage: image, size: NSSize(width: size, height: size))
}()
