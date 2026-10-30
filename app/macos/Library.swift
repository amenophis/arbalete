// The library (ui/src/Library.tsx), which the sidebar shows instead of a Space's page: a rail of
// sections, the one picked last kept until the app quits, and the picked section under a field
// searching it, but for the Spaces and the extensions, which fill the window.

import AppKit
import SwiftUI

enum LibrarySection: String, CaseIterable, Identifiable {
    case spaces, closed, archive, history, media, downloads, extensions

    var id: String { rawValue }

    var label: String {
        switch self {
        case .spaces: "Spaces"
        case .closed: "Recently Closed"
        case .archive: "Archived Tabs"
        case .history: "History"
        case .media: "Media"
        case .downloads: "Downloads"
        case .extensions: "Extensions"
        }
    }

    var symbol: String {
        switch self {
        case .spaces: "square.split.3x1"
        case .closed: "clock.arrow.circlepath"
        case .archive: "archivebox"
        case .history: "clock"
        case .media: "photo"
        case .downloads: "arrow.down.circle"
        case .extensions: "puzzlepiece.extension"
        }
    }

    /// The Spaces and the extensions fill the window, the page hidden; the other sections rest
    /// beside it.
    var wide: Bool { self == .spaces || self == .extensions }
}

struct ClosedTab: Decodable {
    var url: String
    var title: String
    var favicon: String?
    var closedAt: Double
}

struct ArchivedTab: Decodable {
    var url: String
    var title: String
    var favicon: String?
    var space: String?
    var archivedAt: Double
}

struct Visit: Decodable {
    var url: String
    var title: String
    var lastVisit: Double
}

struct Download: Decodable, Identifiable {
    var id: Int
    var name: String
    var url: String
    var received: Double
    var total: Double
    var speed: Double
    var state: String
    var startedAt: Double
    var missing: Bool
}

struct InstalledExtension: Decodable, Identifiable {
    var id: String
    var name: String
    var icon: String?
    var popup: Bool
    var enabled: Bool
    var pinned: Bool
    var options: Bool
    var description: String
    var version: String
}

/// How long ago a time was, both in milliseconds since the epoch, in a few words: "2 min ago".
func ago(_ time: Double, now: Double = Date().timeIntervalSince1970 * 1000) -> String {
    let minutes = Int((now - time) / 60_000)
    if minutes < 1 { return "Just now" }
    if minutes < 60 { return "\(minutes) min ago" }
    let hours = minutes / 60
    if hours < 24 { return "\(hours) h ago" }
    return "\(hours / 24) d ago"
}

/// Whether any of `texts` holds what is typed in a section's search field, whatever the case.
func matches(_ query: String, _ texts: String...) -> Bool {
    let query = query.trimmingCharacters(in: .whitespaces).lowercased()
    return query.isEmpty || texts.contains { $0.lowercased().contains(query) }
}

/// "12.3 / 45.6 MB · 8 s left" while downloading, then what became of it.
func downloadDetail(_ download: Download) -> String {
    func unit(_ bytes: Double) -> (String, Double) {
        bytes >= 1e9 ? ("GB", 1e9) : bytes >= 1e6 ? ("MB", 1e6) : bytes >= 1e3 ? ("KB", 1e3) : ("B", 1)
    }
    func amount(_ bytes: Double, _ divisor: Double) -> String {
        divisor == 1 ? String(Int(bytes)) : String(format: "%.1f", bytes / divisor)
    }
    func size(_ bytes: Double) -> String {
        let (name, divisor) = unit(bytes)
        return "\(amount(bytes, divisor)) \(name)"
    }
    switch download.state {
    case "complete": return download.missing ? "Deleted" : size(download.received)
    case "canceled": return "Canceled"
    case "failed": return "Failed"
    default: break
    }
    guard download.total > 0 else { return size(download.received) }
    let (name, divisor) = unit(download.total)
    let progress = "\(amount(download.received, divisor)) / \(amount(download.total, divisor)) \(name)"
    guard download.speed > 0 else { return progress }
    let left = Int(((download.total - download.received) / download.speed).rounded(.up))
    let time = left < 60 ? "\(left) s" : left < 3600 ? "\(left / 60) min" : "\(left / 3600) h"
    return "\(progress) · \(time) left"
}

struct LibraryView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        HStack(spacing: 0) {
            VStack(spacing: 4) {
                Spacer().frame(height: 40)
                ForEach(LibrarySection.allCases) { section in
                    Button {
                        model.pick(section)
                    } label: {
                        VStack(spacing: 3) {
                            Image(systemName: section.symbol).font(.system(size: 18))
                            Text(section.label).font(.system(size: 9)).multilineTextAlignment(.center)
                        }
                        .frame(width: 64, height: 54)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(.primary.opacity(model.section == section ? 0.14 : 0)))
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
                Spacer()
                IconButton(symbol: "arrow.left", label: "Back to the Space") { model.closeLibrary() }
                    .padding(.bottom, 10)
            }
            .padding(.horizontal, 4)
            VStack(alignment: .leading, spacing: 8) {
                Spacer().frame(height: 36)
                if !model.section.wide {
                    TextField("Search \(model.section.label)…", text: model.queryBinding)
                        .textFieldStyle(.roundedBorder)
                }
                ScrollView {
                    VStack(alignment: .leading, spacing: 2) {
                        switch model.section {
                        case .spaces: ManageSpacesView(model: model)
                        case .closed: ClosedTabs(model: model)
                        case .archive: ArchivedTabs(model: model)
                        case .history: HistoryList(model: model)
                        case .media: MediaGrid(model: model)
                        case .downloads: DownloadsList(model: model)
                        case .extensions: ExtensionsList(model: model)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
            }
            .padding(.trailing, 10)
        }
    }
}

/// A row opening something: an icon, a title and a detail at its right.
struct LibraryRow: View {
    let favicon: String?
    let title: String
    let detail: String
    var help = ""
    let action: () -> Void
    private let hovered = State(initialValue: false)

    var body: some View {
        Button(action: action) {
            HStack(spacing: 8) {
                Favicon(url: favicon, size: 16)
                Text(title).lineLimit(1)
                Spacer(minLength: 6)
                Text(detail).font(.system(size: 11)).foregroundStyle(.secondary)
            }
            .padding(.horizontal, 8)
            .frame(height: 32)
            .background(
                RoundedRectangle(cornerRadius: 7).fill(.primary.opacity(hovered.wrappedValue ? 0.08 : 0)))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hovered.wrappedValue = $0 }
        .help(help)
    }
}

struct Empty: View {
    let text: String

    var body: some View {
        Text(text).foregroundStyle(.secondary).padding(.top, 20).frame(maxWidth: .infinity)
    }
}

/// The tabs closed since launch, the last closed first, which reopen in the displayed Space.
struct ClosedTabs: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let closed = Array((model.state?.closed ?? []).enumerated())
            .filter { matches(model.query, $0.element.title, $0.element.url) }
        if model.state?.closed.isEmpty ?? true {
            Empty(text: "No tab closed yet")
        } else if closed.isEmpty {
            Empty(text: "No closed tab matches")
        }
        ForEach(closed, id: \.offset) { rank, tab in
            LibraryRow(
                favicon: tab.favicon, title: tab.title.isEmpty ? tab.url : tab.title,
                detail: ago(tab.closedAt), help: tab.url
            ) {
                model.run("reopen/\(rank)")
                model.closeLibrary()
            }
        }
    }
}

/// The tabs archived, the most recent first: a click opens one again in its Space.
struct ArchivedTabs: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let found = Array(model.archive.enumerated())
            .filter { matches(model.query, $0.element.title, $0.element.url) }
        if model.archive.isEmpty {
            Empty(text: "No archived tabs yet")
        } else if found.isEmpty {
            Empty(text: "No archived tab matches")
        }
        ForEach(found, id: \.offset) { rank, tab in
            LibraryRow(
                favicon: tab.favicon, title: tab.title.isEmpty ? tab.url : tab.title,
                detail: ago(tab.archivedAt), help: tab.space.map { "\(tab.url)\nin \($0)" } ?? tab.url
            ) {
                model.run("archive/restore/\(rank)")
                model.closeLibrary()
            }
        }
    }
}

/// The displayed Space's profile history, the latest visit first, which the app searches.
struct HistoryList: View {
    @ObservedObject var model: AppModel

    var body: some View {
        if model.history.isEmpty {
            Empty(text: model.query.isEmpty ? "No history yet" : "No page matches")
        }
        ForEach(Array(model.history.enumerated()), id: \.offset) { _, visit in
            LibraryRow(
                favicon: nil, title: visit.title.isEmpty ? visit.url : visit.title,
                detail: ago(visit.lastVisit), help: visit.url
            ) {
                model.run("open/\(percentEncoded(visit.url))")
                model.closeLibrary()
            }
            .rightClick { model.run("history/remove/\(percentEncoded(visit.url))") }
        }
    }
}

/// The images and videos downloaded and still there, as a grid of their previews.
struct MediaGrid: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let media = model.downloads.reversed().filter { download in
            let ext = (download.name as NSString).pathExtension.lowercased()
            return download.state == "complete" && !download.missing
                && ["png", "jpg", "jpeg", "gif", "webp", "avif", "mp4", "m4v", "webm", "mov"].contains(ext)
                && matches(model.query, download.name)
        }
        if media.isEmpty { Empty(text: "No images or videos downloaded") }
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 90), spacing: 6)], spacing: 6) {
            ForEach(media) { download in
                Button {
                    model.run("download/open/\(download.id)")
                } label: {
                    Group {
                        if let image = uiImage("arbalete://ui/media/\(download.id)") {
                            Image(nsImage: image).resizable().scaledToFill()
                        } else {
                            Image(systemName: "film").font(.system(size: 24))
                        }
                    }
                    .frame(width: 90, height: 90)
                    .clipShape(RoundedRectangle(cornerRadius: 8))
                }
                .buttonStyle(.plain)
                .help(download.name)
            }
        }
    }
}

/// Every download, the most recent first.
struct DownloadsList: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let found = model.downloads.reversed().filter { matches(model.query, $0.name, $0.url) }
        if found.isEmpty { Empty(text: model.downloads.isEmpty ? "No downloads yet" : "No download matches") }
        ForEach(found) { download in
            DownloadRow(model: model, download: download)
        }
    }
}

/// A finished download opens on a click, and shows in the Finder from its button; the × cancels
/// one in progress, or takes another off the list.
struct DownloadRow: View {
    @ObservedObject var model: AppModel
    let download: Download

    var body: some View {
        let running = download.state == "inProgress"
        HStack(spacing: 8) {
            Image(systemName: "doc").frame(width: 16)
            VStack(alignment: .leading, spacing: 1) {
                Text(download.name).lineLimit(1)
                Text(downloadDetail(download)).font(.system(size: 11)).foregroundStyle(.secondary)
            }
            Spacer(minLength: 4)
            if download.state == "complete" && !download.missing {
                IconButton(symbol: "magnifyingglass", label: "Show in Finder", size: 22) {
                    model.run("download/show/\(download.id)")
                }
            }
            IconButton(symbol: "xmark", label: running ? "Cancel" : "Remove", size: 22) {
                model.run("download/\(running ? "cancel" : "remove")/\(download.id)")
            }
        }
        .padding(.horizontal, 8)
        .frame(height: 40)
        .contentShape(Rectangle())
        .onTapGesture { if download.state == "complete" { model.run("download/open/\(download.id)") } }
        .help(download.url)
    }
}

/// One card per Space, in sidebar order: its color, name and profile; a click shows the Space,
/// its menu has the rest. The "+" after them creates a Space.
struct ManageSpacesView: View {
    @ObservedObject var model: AppModel

    var body: some View {
        LazyVGrid(columns: [GridItem(.adaptive(minimum: 200), spacing: 10)], spacing: 10) {
            ForEach(model.state?.spaces ?? []) { space in
                SpaceCard(model: model, space: space)
            }
            Button {
                newSpace(model)
            } label: {
                Image(systemName: "plus").font(.system(size: 20))
                    .frame(maxWidth: .infinity, minHeight: 120)
                    .background(RoundedRectangle(cornerRadius: 12).strokeBorder(.primary.opacity(0.2), style: StrokeStyle(dash: [4])))
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .help("New Space")
        }
    }
}

struct SpaceCard: View {
    @ObservedObject var model: AppModel
    let space: Space

    var body: some View {
        let profile = model.state?.profiles.first { $0.id == space.profile }
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text(space.emoji ?? "•").font(.system(size: 16))
                Text(space.title).font(.system(size: 13, weight: .semibold)).lineLimit(1)
                Spacer()
                IconButton(symbol: "ellipsis", label: "Space options", size: 22) {
                    model.run("menu/space/\(space.id)")
                }
            }
            Button(profile?.name ?? "Profile") { model.run("menu/space-profile/\(space.id)") }
                .buttonStyle(.plain)
                .font(.system(size: 11))
                .foregroundStyle(.secondary)
            Spacer(minLength: 0)
        }
        .padding(12)
        .frame(maxWidth: .infinity, minHeight: 120, alignment: .topLeading)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(space.theme.colors.first.map(color) ?? Color.gray)
                .opacity(0.35))
        .contentShape(Rectangle())
        .onTapGesture {
            model.run("space/\(space.id)")
            model.closeLibrary()
        }
        .rightClick { model.run("menu/space/\(space.id)") }
    }
}

/// The displayed Space's profile's extensions: each with its switch, pin, options and removal;
/// the Chrome Web Store to add more, a folder to load one from, and updates.
struct ExtensionsList: View {
    @ObservedObject var model: AppModel

    var body: some View {
        HStack {
            Button("Add Extensions") { model.run("site/store") }
            Button("Load Unpacked…") { model.run("extension/load-unpacked") }
            Button("Update") { model.run("extension/update") }
        }
        .padding(.bottom, 8)
        let installed = model.state?.installed ?? []
        if installed.isEmpty { Empty(text: "No extensions yet") }
        ForEach(installed) { ext in
            HStack(spacing: 10) {
                Favicon(url: ext.icon, size: 24)
                VStack(alignment: .leading, spacing: 2) {
                    Text(ext.name).font(.system(size: 13, weight: .semibold))
                    Text(ext.description).font(.system(size: 11)).foregroundStyle(.secondary).lineLimit(2)
                    Text(ext.version).font(.system(size: 10)).foregroundStyle(.tertiary)
                }
                Spacer()
                IconButton(symbol: ext.pinned ? "pin.fill" : "pin", label: ext.pinned ? "Unpin" : "Pin", size: 24) {
                    model.run("extension/\(ext.pinned ? "unpin" : "pin")/\(ext.id)")
                }
                if ext.options {
                    IconButton(symbol: "gearshape", label: "Options", size: 24) {
                        model.run("extension/options/\(ext.id)")
                    }
                }
                IconButton(symbol: "trash", label: "Remove", size: 24) {
                    model.run("extension/remove/\(ext.id)")
                }
                Toggle("", isOn: Binding(
                    get: { ext.enabled },
                    set: { _ in model.run("extension/\(ext.enabled ? "off" : "on")/\(ext.id)") }))
                    .toggleStyle(.switch)
                    .labelsHidden()
                    .controlSize(.small)
            }
            .padding(10)
            .background(RoundedRectangle(cornerRadius: 10).fill(.primary.opacity(0.06)))
        }
    }
}

/// The downloads in progress and the ones of the last day at the bottom of the sidebar, three at
/// most; the library lists them all.
struct RecentDownloads: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let now = Date().timeIntervalSince1970 * 1000
        let recent = model.downloads.reversed()
            .filter { $0.state == "inProgress" || now - $0.startedAt < 24 * 3600 * 1000 }
            .prefix(3)
        VStack(spacing: 0) {
            ForEach(Array(recent)) { download in
                DownloadRow(model: model, download: download)
            }
        }
    }
}
