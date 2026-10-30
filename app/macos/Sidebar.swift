// The native sidebar: the displayed Space's tabs over the desktop, blurred through the window and
// tinted by the Space's theme, like Arc's. The app (app/src/native.rs) attaches it to its window,
// sends it the state the web UI gets (UiState::to_json in app/src/shell.rs), and runs the
// commands it sends back, the same as the web UI's (app/src/command.rs).

import AppKit
import SwiftUI

/// Runs a command, given as its path, like `select/12`.
public typealias Send = @convention(c) (UnsafePointer<CChar>) -> Void

/// The part of the app's state the sidebar shows.
struct State: Decodable {
    var space = 0
    var spaces: [Space] = []
    var pages: [Page] = []
}

struct Space: Decodable, Identifiable {
    var id: Int
    var title: String
    var theme: Theme
}

/// How a Space colors the window (Theme in crates/arbalete-core/src/sidebar.rs).
struct Theme: Decodable {
    var colors: [String]
    var mode: String
    var intensity: Double
}

/// A Space's tabs.
struct Page: Decodable {
    var space: Int
    var selected: Int?
    var pinned: [Item]
    var today: [Item]
}

/// A tab, a folder of items, or tabs split side by side.
struct Item: Decodable, Identifiable {
    var id: Int
    var title: String?
    var children: [Item]?
    var split: [Item]?
}

@MainActor
final class Model: ObservableObject {
    @Published var state = State()
    let send: Send

    init(send: @escaping Send) {
        self.send = send
    }

    func run(_ path: String) {
        path.withCString { send($0) }
    }

    var page: Page? {
        state.pages.first { $0.space == state.space }
    }

    var theme: Theme? {
        state.spaces.first { $0.id == state.space }?.theme
    }
}

@MainActor private var model: Model?
@MainActor private var blur: NSVisualEffectView?

/// Puts the sidebar, `width` points wide, at the left of the window `handle` is, or is in.
@_cdecl("arbalete_sidebar_attach")
public func attach(_ handle: UnsafeMutableRawPointer, _ width: Double, _ send: Send) {
    MainActor.assumeIsolated {
        let object = Unmanaged<AnyObject>.fromOpaque(handle).takeUnretainedValue()
        let window = (object as? NSWindow) ?? (object as? NSView)?.window
        guard let window, let content = window.contentView else { return }
        // The blur shows under the window's buttons too.
        window.titlebarAppearsTransparent = true
        let sidebar = Model(send: send)
        // Through the window, what is behind it, blurred.
        let effect = NSVisualEffectView(
            frame: NSRect(x: 0, y: 0, width: width, height: content.bounds.height))
        effect.material = .sidebar
        effect.blendingMode = .behindWindow
        effect.state = .followsWindowActiveState
        effect.autoresizingMask = [.height, .maxXMargin]
        let host = NSHostingView(rootView: SidebarView(model: sidebar))
        host.frame = effect.bounds
        host.autoresizingMask = [.width, .height]
        effect.addSubview(host)
        content.addSubview(effect)
        model = sidebar
        blur = effect
    }
}

/// The app's state, as JSON.
@_cdecl("arbalete_sidebar_state")
public func update(_ json: UnsafePointer<CChar>) {
    MainActor.assumeIsolated {
        guard let model,
            let state = try? JSONDecoder().decode(State.self, from: Data(String(cString: json).utf8))
        else { return }
        model.state = state
        blur?.appearance = NSAppearance(named: isDark(model.theme) ? .darkAqua : .aqua)
    }
}

/// Whether a theme shows light text: as its mode forces, or as its colors are, and as the system
/// is without colors (isDark in ui/src/theme.ts).
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

func luminance(_ hex: String) -> Double {
    let linear = channels(hex).map { $0 <= 0.04045 ? $0 / 12.92 : pow(($0 + 0.055) / 1.055, 2.4) }
    return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
}

func color(_ hex: String) -> Color {
    let rgb = channels(hex)
    return Color(red: rgb[0], green: rgb[1], blue: rgb[2])
}

struct SidebarView: View {
    @ObservedObject var model: Model

    var body: some View {
        let page = model.page
        VStack(alignment: .leading, spacing: 0) {
            // Room for the window's buttons.
            Spacer().frame(height: 44)
            ScrollView {
                VStack(alignment: .leading, spacing: 2) {
                    ForEach(page?.pinned ?? []) { item in
                        Row(item: item, selected: page?.selected, model: model)
                    }
                    if !(page?.pinned.isEmpty ?? true) {
                        Divider().padding(.vertical, 6)
                    }
                    Button {
                        model.run("overlay/show")
                    } label: {
                        Label("New Tab", systemImage: "plus")
                            .frame(maxWidth: .infinity, alignment: .leading)
                            .padding(.horizontal, 10)
                            .frame(height: 32)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .foregroundStyle(.secondary)
                    ForEach(page?.today ?? []) { item in
                        Row(item: item, selected: page?.selected, model: model)
                    }
                }
            }
            HStack(spacing: 10) {
                ForEach(model.state.spaces) { space in
                    Circle()
                        .fill(space.id == model.state.space ? Color.primary : Color.secondary)
                        .frame(width: 6, height: 6)
                        .padding(4)
                        .contentShape(Rectangle())
                        .onTapGesture { model.run("space/\(space.id)") }
                }
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
        }
        .padding(.horizontal, 8)
        .background(Tint(theme: model.theme))
    }
}

/// The theme's colors over the blur, the stronger the more intense.
struct Tint: View {
    let theme: Theme?

    var body: some View {
        if let theme, !theme.colors.isEmpty {
            LinearGradient(
                colors: theme.colors.map(color),
                startPoint: .topLeading,
                endPoint: .bottomTrailing
            )
            .opacity(0.2 + 0.6 * theme.intensity)
        }
    }
}

struct Row: View {
    let item: Item
    let selected: Int?
    let model: Model

    var body: some View {
        if let children = item.children {
            Label(item.title ?? "", systemImage: "folder")
                .padding(.horizontal, 10)
                .frame(height: 32)
            ForEach(children) { child in
                Row(item: child, selected: selected, model: model).padding(.leading, 14)
            }
        } else if let split = item.split {
            ForEach(split) { tab in
                Row(item: tab, selected: selected, model: model)
            }
        } else {
            Text(item.title ?? "")
                .lineLimit(1)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(.horizontal, 10)
                .frame(height: 32)
                .background(
                    RoundedRectangle(cornerRadius: 8)
                        .fill(Color.primary.opacity(item.id == selected ? 0.12 : 0))
                )
                .contentShape(Rectangle())
                .onTapGesture { model.run("select/\(item.id)") }
        }
    }
}
