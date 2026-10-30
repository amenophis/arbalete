// The native macOS interface's side of the bridge with the app (app/src/native.rs): the app
// attaches it to its window, gives it the events the web UI gets, and runs the commands it sends
// back, the same as the web UI's (app/src/command.rs).

import AppKit
import SwiftUI

/// Runs a command, given as its path, like `select/12`.
public typealias Send = @convention(c) (UnsafePointer<CChar>) -> Void

/// What the interface knows of the app, from its events.
@MainActor
final class AppModel: ObservableObject {
    @Published var state: AppState?
    /// The folder or the tab whose name is being edited.
    @Published var renaming: Int?
    let send: Send

    init(send: @escaping Send) {
        self.send = send
    }

    func run(_ path: String) {
        path.withCString { send($0) }
    }

    /// The displayed Space's page.
    var page: SpacePage? {
        guard let state else { return nil }
        return state.pages.first { $0.space == state.space }
    }

    var space: Space? {
        guard let state else { return nil }
        return state.spaces.first { $0.id == state.space }
    }

    /// The displayed Space's selected tab, a favorite, a pinned tab or one of today's.
    var activeTab: Tab? {
        guard let page else { return nil }
        let all = page.favorites + (page.pinned + page.today).flatMap(\.tabs)
        return all.first { $0.id == page.selected }
    }

    /// The window's colors last sent, so as to send them only when they change.
    private var colors = ""

    /// An event dispatched to the web UI's sidebar.
    func handle(_ name: String, _ detail: Data) {
        switch name {
        case "arbalete:state":
            do {
                state = try JSONDecoder().decode(AppState.self, from: detail)
            } catch {
                NSLog("arbalete: cannot read the state: \(error)")
            }
            paintWindow()
        case "arbalete:rename":
            renaming = try? JSONDecoder().decode(Int.self, from: detail)
        default:
            break
        }
    }

    /// Tells the app the card's color, which an empty Space shows, a little tinted by the
    /// displayed Space's leading color (WindowColors in app/src/command.rs).
    private func paintWindow() {
        let theme = space?.theme
        let dark = isDark(theme)
        let panel = dark ? "#1e1a28" : "#f8f6fb"
        let lead = theme?.colors.first
        let card = lead.map { mix($0, panel, 0.08) } ?? panel
        let command = "window/colors/\(card.dropFirst())/0/\((lead ?? panel).dropFirst())"
        if command != colors {
            colors = command
            run(command)
        }
    }
}

@MainActor var model: AppModel?
@MainActor var backdrop: Backdrop?
@MainActor var sidebar: SidebarHost?

/// Puts the interface in the window `handle` is, or is in: the blur behind every view, and the
/// sidebar above them.
@_cdecl("arbalete_ui_attach")
public func attach(_ handle: UnsafeMutableRawPointer, _ send: Send) {
    MainActor.assumeIsolated {
        let object = Unmanaged<AnyObject>.fromOpaque(handle).takeUnretainedValue()
        guard let window = (object as? NSWindow) ?? (object as? NSView)?.window,
            let content = window.contentView
        else { return }
        let app = AppModel(send: send)
        model = app
        window.isOpaque = false
        window.backgroundColor = .clear
        window.titlebarAppearsTransparent = true
        backdrop = Backdrop(content: content, model: app)
        sidebar = SidebarHost(content: content, model: app)
    }
}

/// An event dispatched to a web surface the interface stands in for, like "sidebar".
@_cdecl("arbalete_ui_event")
public func event(
    _ surface: UnsafePointer<CChar>, _ name: UnsafePointer<CChar>, _ detail: UnsafePointer<CChar>
) {
    MainActor.assumeIsolated {
        let detail = Data(String(cString: detail).utf8)
        switch String(cString: surface) {
        case "sidebar":
            model?.handle(String(cString: name), detail)
        default:
            break
        }
    }
}

/// Shows the sidebar at a frame in the window's points from its top left corner, or hides it.
@_cdecl("arbalete_ui_place_sidebar")
public func placeSidebar(
    _ visible: Bool, _ x: Double, _ y: Double, _ width: Double, _ height: Double,
    _ floating: Bool, _ revealed: Bool
) {
    MainActor.assumeIsolated {
        sidebar?.place(
            visible ? NSRect(x: x, y: y, width: width, height: height) : nil,
            floating: floating, revealed: revealed)
    }
}

/// The image behind a url of the app's UI, like a tab's cached icon, read once.
@MainActor func uiImage(_ url: String?) -> NSImage? {
    guard let url else { return nil }
    if let image = images[url] { return image }
    let path = url.withCString { arbalete_ui_file($0) }
    defer { arbalete_free_string(path) }
    let image = path.flatMap { NSImage(contentsOfFile: String(cString: $0)) }
    if image != nil { images[url] = image }
    return image
}

@MainActor private var images: [String: NSImage] = [:]
