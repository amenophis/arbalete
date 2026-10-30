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
    @Published var downloads: [Download] = []
    @Published var archive: [ArchivedTab] = []
    @Published var history: [Visit] = []
    /// The sidebar shows the library, on a section, under what its field searches.
    @Published var libraryShown = false
    @Published var section = LibrarySection.spaces
    @Published var query = "" {
        didSet { if section == .history { searchHistory() } }
    }
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
        case "arbalete:downloads":
            downloads = (try? JSONDecoder().decode([Download].self, from: detail)) ?? downloads
        case "arbalete:archive":
            archive = (try? JSONDecoder().decode([ArchivedTab].self, from: detail)) ?? archive
        case "arbalete:history":
            struct Found: Decodable {
                var text: String
                var entries: [Visit]
            }
            if let found = try? JSONDecoder().decode(Found.self, from: detail), found.text == query {
                history = found.entries
            }
        case "arbalete:library":
            openLibrary(.spaces)
        case "arbalete:open-library":
            openLibrary(nil)
        case "arbalete:show-downloads":
            openLibrary(.downloads)
        case "arbalete:show-history":
            openLibrary(.history)
        case "arbalete:show-extensions":
            openLibrary(.extensions)
        default:
            break
        }
    }

    var queryBinding: Binding<String> {
        Binding(get: { self.query }, set: { self.query = $0 })
    }

    /// Shows the library, on a section or the one it showed last.
    func openLibrary(_ section: LibrarySection?) {
        libraryShown = true
        run("library/show")
        pick(section ?? self.section)
    }

    func pick(_ section: LibrarySection) {
        self.section = section
        query = ""
        run(section.wide ? "library/wide" : "library/narrow")
        // Their page may have changed them.
        switch section {
        case .extensions: run("extension/reload")
        case .archive: run("archive/list")
        case .history: searchHistory()
        default: break
        }
    }

    /// Back to the displayed Space, the card resting beside the sidebar again.
    func closeLibrary() {
        libraryShown = false
        run("card/offset/0")
    }

    private func searchHistory() {
        run("history/search/200/\(percentEncoded(query))")
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
@MainActor weak var mainWindow: NSWindow?
@MainActor var backdrop: Backdrop?
@MainActor var sidebar: SidebarHost?
@MainActor var commandBar: CommandBar?
@MainActor var findBar: FindBar?
@MainActor var siteControls: SiteControls?
@MainActor var themePicker: ThemePickerModel?
@MainActor var importModel: ImportModel?

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
        mainWindow = window
        window.isOpaque = false
        window.backgroundColor = .clear
        window.titlebarAppearsTransparent = true
        backdrop = Backdrop(content: content, model: app)
        sidebar = SidebarHost(window: window, model: app)
        let bar = CommandBar(app: app)
        commandBar = bar
        let overlay = Surface(in: window, radius: 14) { CommandBarView(bar: bar) }
        overlay.takesKeyboard = true
        overlay.shown = { [weak bar] in
            guard let field = bar?.field, field.currentEditor() == nil else { return }
            field.window?.makeFirstResponder(field)
        }
        surfaces["overlay"] = overlay
        let find = FindBar(app: app)
        findBar = find
        let findSurface = Surface(in: window, radius: 10) { FindView(bar: find) }
        findSurface.takesKeyboard = true
        findSurface.shown = { [weak find] in
            guard let field = find?.field else { return }
            field.window?.makeFirstResponder(field)
            field.selectText(nil)
        }
        surfaces["find"] = findSurface
        surfaces["toolbar"] = Surface(in: window, radius: 0, material: .titlebar) {
            PageToolbar(model: app)
        }
        let site = SiteControls(app: app)
        siteControls = site
        let siteSurface = Surface(in: window, radius: 12) { SiteView(controls: site) }
        siteSurface.takesKeyboard = true
        siteSurface.resigned = { app.run("site/hide") }
        surfaces["site"] = siteSurface
        let picker = ThemePickerModel(app: app)
        themePicker = picker
        let themeSurface = Surface(in: window, radius: 20) { ThemePickerView(picker: picker) }
        themeSurface.takesKeyboard = true
        themeSurface.resigned = { app.run("theme/hide") }
        surfaces["theme"] = themeSurface
        settingsModel = SettingsModel(app: app)
        let importer = ImportModel(app: app)
        importModel = importer
        let importSurface = Surface(in: window, radius: 16) { ImportView(model: importer) }
        importSurface.takesKeyboard = true
        surfaces["import"] = importSurface
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
        case "overlay":
            commandBar?.handle(String(cString: name), detail)
        case "find":
            findBar?.handle(String(cString: name), detail)
        case "site":
            siteControls?.handle(String(cString: name), detail)
        case "theme":
            themePicker?.handle(String(cString: name), detail)
        case "settings":
            settingsModel?.handle(String(cString: name), detail)
        case "import":
            importModel?.handle(String(cString: name), detail)
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
