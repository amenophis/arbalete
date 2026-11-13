// Sharing the screen, like Arc's: the picker of what a site shares, its tabs, windows and screens
// (app/src/shell/screen_share.rs).

import ScreenCaptureKit
import SwiftUI

/// What a site's request to share the screen offers, besides the windows and the screens.
struct ScreenRequest: Decodable, Equatable {
    struct Tab: Decodable, Equatable, Identifiable {
        var id: Int
        var title: String
        var url: String
        var favicon: String?
    }
    /// The tabs with a live page, but the asking one.
    var tabs: [Tab]
    /// The site asks for the sound too.
    var sound: Bool
}

/// A window or a screen the picker offers, with its picture once taken.
struct ScreenSource: Identifiable {
    /// As Chromium names it: "window:<id>:0" or "screen:<id>:0".
    let id: String
    let title: String
    /// A window's app's icon.
    var icon: NSImage?
    var picture: CGImage?
}

/// The windows and screens to share, read once macOS lets Arbalete record the screen.
@MainActor
final class ScreenSources: ObservableObject {
    @Published var windows: [ScreenSource] = []
    @Published var screens: [ScreenSource] = []
    /// macOS lets Arbalete record the screen, which sharing a window or a screen needs.
    @Published var allowed = CGPreflightScreenCaptureAccess()

    func load() {
        allowed = CGPreflightScreenCaptureAccess()
        guard allowed else { return }
        Task {
            guard
                let content = try? await SCShareableContent.excludingDesktopWindows(
                    true, onScreenWindowsOnly: true)
            else { return }
            let own = Bundle.main.bundleIdentifier
            let windows = content.windows.filter { window in
                window.windowLayer == 0 && window.frame.width > 80 && window.frame.height > 80
                    && !(window.title ?? "").isEmpty
                    && window.owningApplication?.bundleIdentifier != own
            }
            self.windows = windows.map { window in
                let app = window.owningApplication
                let icon = app.flatMap { NSRunningApplication(processIdentifier: $0.processID)?.icon }
                return ScreenSource(id: "window:\(window.windowID):0", title: window.title ?? "", icon: icon)
            }
            self.screens = content.displays.enumerated().map { index, display in
                let title = content.displays.count > 1 ? "Screen \(index + 1)" : "Entire Screen"
                return ScreenSource(id: "screen:\(display.displayID):0", title: title)
            }
            for window in windows {
                let filter = SCContentFilter(desktopIndependentWindow: window)
                picture(filter, window.frame.size, "window:\(window.windowID):0")
            }
            for display in content.displays {
                let filter = SCContentFilter(display: display, excludingWindows: [])
                let size = CGSize(width: display.width, height: display.height)
                picture(filter, size, "screen:\(display.displayID):0")
            }
        }
    }

    /// Takes a small picture of a source, for its card.
    private func picture(_ filter: SCContentFilter, _ size: CGSize, _ id: String) {
        let configuration = SCStreamConfiguration()
        let scale = min(1, 400 / max(size.width, size.height, 1))
        configuration.width = Int(size.width * scale * 2)
        configuration.height = Int(size.height * scale * 2)
        configuration.showsCursor = false
        Task {
            guard
                let image = try? await SCScreenshotManager.captureImage(
                    contentFilter: filter, configuration: configuration)
            else { return }
            if let index = windows.firstIndex(where: { $0.id == id }) {
                windows[index].picture = image
            } else if let index = screens.firstIndex(where: { $0.id == id }) {
                screens[index].picture = image
            }
        }
    }

    /// Asks macOS to let Arbalete record the screen, then opens its settings to allow it.
    func askToRecord() {
        if !CGRequestScreenCaptureAccess(),
            let url = URL(
                string: "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
        {
            NSWorkspace.shared.open(url)
        }
        load()
    }
}

/// The picker of what a site shares, like Chrome's: a tab, a window or a whole screen, each with
/// its picture, its sound too if the site asks, then Share or Cancel. A double-click shares.
struct ScreenPicker: View {
    @ObservedObject var prompt: PermissionPrompt
    let id: UInt64
    let site: String
    let request: ScreenRequest
    @StateObject private var sources = ScreenSources()
    // Without the @State macro, which the interface's build has no plug-in for.
    private let _kind = State(initialValue: Kind.tabs)
    private let _picked = State<String?>(initialValue: nil)
    private let _sound = State(initialValue: true)
    private var kind: Kind {
        get { _kind.wrappedValue }
        nonmutating set { _kind.wrappedValue = newValue }
    }
    private var picked: String? {
        get { _picked.wrappedValue }
        nonmutating set { _picked.wrappedValue = newValue }
    }

    enum Kind: CaseIterable {
        case tabs, windows, screens

        var title: String {
            switch self {
            case .tabs: "Tab"
            case .windows: "Window"
            case .screens: "Entire Screen"
            }
        }

        var symbol: String {
            switch self {
            case .tabs: "square.on.square"
            case .windows: "macwindow"
            case .screens: "display"
            }
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 14) {
            VStack(alignment: .leading, spacing: 3) {
                (Text("Share with ") + Text(site).fontWeight(.semibold))
                    .font(.system(size: 15))
                    .lineLimit(1)
                    .truncationMode(.head)
                Text("Choose a tab, a window or a screen.")
                    .font(.system(size: 12))
                    .foregroundStyle(.secondary)
            }
            HStack(spacing: 4) {
                ForEach(Kind.allCases, id: \.self) { choice in
                    KindButton(kind: choice, chosen: kind == choice) {
                        kind = choice
                        picked = nil
                    }
                }
            }
            .padding(3)
            .background(Capsule().fill(.primary.opacity(0.06)))
            ScrollView {
                content.padding(3)
            }
            .frame(height: 330)
            HStack(spacing: 8) {
                if request.sound {
                    Toggle(kind == .tabs ? "Share tab audio" : "Share system audio", isOn: _sound.projectedValue)
                        .toggleStyle(.switch)
                        .controlSize(.small)
                        .font(.system(size: 12))
                }
                Spacer(minLength: 0)
                PermissionButton(title: "Cancel", prominent: false) { answer(nil) }
                    .frame(width: 96)
                PermissionButton(title: "Share", prominent: true) { answer(picked) }
                    .frame(width: 96)
                    .disabled(picked == nil)
                    .opacity(picked == nil ? 0.45 : 1)
            }
        }
        .onAppear { sources.load() }
    }

    @ViewBuilder private var content: some View {
        switch kind {
        case .tabs:
            if request.tabs.isEmpty {
                empty("square.on.square", "No other tab is open.")
            } else {
                grid(request.tabs.map { tab in
                    Card(
                        id: "tab:\(tab.id)", title: tab.title,
                        icon: AnyView(Favicon(url: tab.favicon, size: 14)),
                        picture: prompt.snapshots[tab.id],
                        placeholder: AnyView(TabPlaceholder(tab: tab)))
                })
            }
        case .windows, .screens:
            if !sources.allowed {
                VStack(spacing: 12) {
                    Image(systemName: "lock.rectangle.on.rectangle")
                        .font(.system(size: 30, weight: .light))
                        .foregroundStyle(.secondary)
                    Text("Allow Arbalete to record your screen")
                        .font(.system(size: 14, weight: .semibold))
                    Text("Sharing a window or a screen needs it. Allow Arbalete in System Settings, then quit and reopen it.")
                        .font(.system(size: 12))
                        .foregroundStyle(.secondary)
                        .multilineTextAlignment(.center)
                        .fixedSize(horizontal: false, vertical: true)
                    PermissionButton(title: "Open System Settings", prominent: true) { sources.askToRecord() }
                        .frame(width: 200)
                }
                .frame(maxWidth: 360)
                .frame(maxWidth: .infinity)
                .padding(.vertical, 40)
            } else {
                let list = kind == .windows ? sources.windows : sources.screens
                if list.isEmpty {
                    empty(kind.symbol, kind == .windows ? "No window to share." : "Looking for screens…")
                } else {
                    grid(list.map { source in
                        Card(
                            id: source.id, title: source.title,
                            icon: source.icon.map { AnyView(Image(nsImage: $0).resizable().frame(width: 16, height: 16)) },
                            picture: source.picture.map { NSImage(cgImage: $0, size: .zero) },
                            placeholder: AnyView(
                                Image(systemName: kind.symbol).font(.system(size: 26, weight: .light))
                                    .foregroundStyle(.secondary)))
                    })
                }
            }
        }
    }

    private func empty(_ symbol: String, _ text: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: symbol).font(.system(size: 28, weight: .light))
            Text(text).font(.system(size: 13))
        }
        .foregroundStyle(.secondary)
        .frame(maxWidth: .infinity)
        .padding(.vertical, 70)
    }

    private func grid(_ cards: [Card]) -> some View {
        LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 12), count: 3), spacing: 14) {
            ForEach(cards, id: \.id) { card in
                CardView(card: card, picked: picked == card.id)
                    .onTapGesture(count: 2) { answer(card.id) }
                    .onTapGesture { picked = card.id }
            }
        }
    }

    /// Gives the site the source picked, or nothing.
    private func answer(_ source: String?) {
        if let source {
            prompt.app.run("share-screen", id, source, request.sound && _sound.wrappedValue)
        } else {
            prompt.app.run("share-screen", id, NSNull(), false)
        }
    }
}

/// A tab without a picture, its page not loaded since the app opened: its site's icon on a tile,
/// over its colors, and its address.
private struct TabPlaceholder: View {
    let tab: ScreenRequest.Tab

    var body: some View {
        let colors = uiImage(tab.favicon).map(averageColors)
        ZStack {
            if let colors {
                LinearGradient(colors: [colors.top, colors.bottom], startPoint: .top, endPoint: .bottom)
                    .opacity(0.35)
            }
            VStack(spacing: 8) {
                Favicon(url: tab.favicon, size: 30)
                    .padding(9)
                    .background(
                        RoundedRectangle(cornerRadius: 10, style: .continuous)
                            .fill(.background.opacity(0.85))
                            .shadow(color: .black.opacity(0.18), radius: 3, y: 1))
                Text(URL(string: tab.url)?.host() ?? tab.url)
                    .font(.system(size: 11))
                    .foregroundStyle(.secondary)
                    .lineLimit(1)
                    .truncationMode(.middle)
                    .padding(.horizontal, 10)
            }
        }
        .clipShape(RoundedRectangle(cornerRadius: 9, style: .continuous))
    }
}

/// One of the picker's kinds of source, in its row of pills.
private struct KindButton: View {
    let kind: ScreenPicker.Kind
    let chosen: Bool
    let action: () -> Void

    var body: some View {
        Button(action: action) {
            Label(kind.title, systemImage: kind.symbol)
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(chosen ? Color.primary : Color.secondary)
                .frame(maxWidth: .infinity)
                .frame(height: 26)
                .background(
                    Capsule().fill(chosen ? AnyShapeStyle(.background) : AnyShapeStyle(.clear))
                        .shadow(color: .black.opacity(chosen ? 0.15 : 0), radius: 1, y: 0.5))
                .contentShape(Capsule())
        }
        .buttonStyle(.plain)
    }
}

/// A source the picker offers: its picture, else a placeholder, and its icon and title under it.
private struct Card {
    let id: String
    let title: String
    let icon: AnyView?
    let picture: NSImage?
    let placeholder: AnyView
}

/// A source's card: its picture in a rounded frame, ringed with the accent color once picked.
private struct CardView: View {
    let card: Card
    let picked: Bool
    private let _hovered = State(initialValue: false)

    var body: some View {
        VStack(alignment: .leading, spacing: 7) {
            ZStack {
                RoundedRectangle(cornerRadius: 9, style: .continuous).fill(.primary.opacity(0.06))
                if let picture = card.picture {
                    Image(nsImage: picture).resizable().aspectRatio(contentMode: .fit)
                        .clipShape(RoundedRectangle(cornerRadius: 5, style: .continuous))
                        .shadow(color: .black.opacity(0.2), radius: 2, y: 1)
                        .padding(8)
                } else {
                    card.placeholder
                }
            }
            .aspectRatio(16 / 10, contentMode: .fit)
            .overlay(
                RoundedRectangle(cornerRadius: 9, style: .continuous)
                    .strokeBorder(
                        picked ? Color.accentColor : .primary.opacity(_hovered.wrappedValue ? 0.18 : 0.08),
                        lineWidth: picked ? 2.5 : 1))
            HStack(spacing: 6) {
                if let icon = card.icon { icon }
                Text(card.title)
                    .font(.system(size: 12, weight: picked ? .semibold : .regular))
                    .lineLimit(1)
                    .truncationMode(.tail)
            }
            .padding(.horizontal, 2)
        }
        .contentShape(Rectangle())
        .onHover { _hovered.wrappedValue = $0 }
        .help(card.title)
    }
}

