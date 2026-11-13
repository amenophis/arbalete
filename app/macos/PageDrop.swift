// A tab dragged from the sidebar onto the page splits it, like Arc's: dropped on an edge of a
// page shown alone, it opens beside it, side by side or one above the other; dropped on a page
// of a split, it joins the split beside that page. A tinted card shows where it would go. Like
// Arc's too, any drag from the sidebar resting just beside it shows the previous or next Space.

import AppKit
import Combine
import SwiftUI

/// Where a tab dropped on the page goes: beside the tab `tab`, before it with `before`, the pages
/// one above the other with `stacked`; `preview` is the room it would take, in the drop's view.
struct PageDropTarget: Equatable {
    var tab: Int
    var before: Bool
    var stacked: Bool
    var preview: NSRect
    /// Like Arc's, it takes the tab's place in the split, which the tab leaves.
    var replace = false
    /// Where among the split's pages it would open.
    var index: Int?
}

/// The card's margin from the page's room, and the room between a split's pages (CARD_MARGIN
/// and SPLIT_GAP in app/src/shell.rs).
private let cardMargin: CGFloat = 10
private let splitGap: CGFloat = 10
/// How far from a page's edges a link dragged over it, from a page or another app, offers to
/// open beside it: a quarter of its width at most at either side, as at the top and the bottom.
/// The page's middle half takes the drop itself.
private let linkSide: CGFloat = 240
private let linkBand: CGFloat = 80
/// Like Arc's, the room beside the sidebar where a drag shows the previous Space, at its left,
/// or the next one, at its right.
private let leadingEdge: CGFloat = 80
private let trailingEdge: CGFloat = 30

/// The drop over the page, shown while a tab is dragged from the sidebar.
@MainActor
final class PageDrop {
    private let parent: NSWindow
    private let model: AppModel
    private let view = DropView()
    /// Where a link would open, drawn apart from the view taking the drop: it takes no drag, so
    /// that it stays while the page's middle, which the page keeps, is crossed.
    private let outline = DropView()
    private let outlinePanel: Overlay
    private let panel: Overlay
    private let edge = EdgeView()
    private let edgePanel: Overlay
    private var watch: AnyCancellable?
    private var timer: Timer?
    /// The web link a drag from a page or another app carries over the window, while it does.
    private var link: String?
    /// Watches for such a drag, and the drag pasteboard's count as the mouse's buttons last came
    /// up: a drag since then changed it.
    private var linkWatch: Timer?
    private var idleCount = NSPasteboard(name: .drag).changeCount
    /// The sidebar, docked or floating, from the window's top left corner, if it shows.
    var sidebar: NSRect?
    /// The split's pages shown, from the card's top left corner.
    var panes: [Pane] = [] {
        didSet { if previewed == command("preview-pane") { stable = panes } }
    }
    /// The empty page the app opened among the split's pages for a drag, where the drag shows
    /// the room it would take.
    var emptyPane: Pane? {
        didSet {
            view.room = emptyPane.map(frame)
            outline.room = view.room
        }
    }
    /// The pages as they stood before an empty one opened among them for a drag, which the drag
    /// keeps finding its places by: the pages move to make room meanwhile.
    private var stable: [Pane] = []

    init(in parent: NSWindow, model: AppModel) {
        self.parent = parent
        self.model = model
        panel = Overlay(view: view)
        outlinePanel = Overlay(view: outline)
        edgePanel = Overlay(view: edge)
        edge.registerForDraggedTypes([.sidebarNode])
        edge.rested = { model.edge($0 ? true : nil) }
        view.registerForDraggedTypes([.sidebarNode, .URL])
        view.locate = { [unowned self] point in target(at: point) }
        view.morphs = { [unowned self] in link == nil }
        view.draws = { [unowned self] in link == nil }
        view.drop = { [unowned self] target in drop(target) }
        view.shown = { [unowned self] target in preview(target) }
        view.card = { [unowned self] in cardImage() }
        // Once the row's drag has said whether it carries a tab.
        watch = model.$dragged.sink { [weak self] _ in
            DispatchQueue.main.async { self?.update() }
        }
        // Also while AppKit tracks a drag.
        let watch = Timer(timeInterval: 0.05, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated { self?.watchLink() }
        }
        RunLoop.main.add(watch, forMode: .common)
        linkWatch = watch
    }

    /// A web link dragged over the page from a page or another app offers to open beside it, as
    /// a tab from the sidebar does, though Arc does not: near its edges only, where the drop's
    /// view shows, leaving the rest of the page to the page itself, which may take a drop of its
    /// own.
    private func watchLink() {
        let board = NSPasteboard(name: .drag)
        guard NSEvent.pressedMouseButtons != 0 else {
            idleCount = board.changeCount
            if link != nil { endLink() }
            return
        }
        guard model.dragged == nil, board.changeCount != idleCount, parent.isVisible,
            let content = parent.contentView,
            let url = board.readObjects(forClasses: [NSURL.self])?.first as? URL,
            ["http", "https"].contains(url.scheme?.lowercased())
        else {
            if link != nil { endLink() }
            return
        }
        link = url.absoluteString
        let left = sidebar?.maxX ?? 0
        let room = NSRect(
            x: left, y: 0, width: max(0, content.bounds.width - left),
            height: content.bounds.height)
        let mouse = parent.mouseLocationOutsideOfEventStream
        let point = NSPoint(x: mouse.x - left, y: content.bounds.height - mouse.y)
        if view.bounds.size != room.size { view.frame.size = room.size }
        let inside = NSRect(origin: .zero, size: room.size).contains(point)
        if inside, target(at: point) != nil {
            if !panel.isVisible {
                view.tint = NSColor(model.accent)
                panel.place(in: parent, room)
            }
            return
        }
        if panel.isVisible {
            view.show(nil)
            panel.place(in: parent, nil)
        }
        // Out of the page, the room it would take goes.
        if !inside { preview(nil, leaving: true) }
    }

    /// Shows where a link would open, over the page, or nothing.
    private func showOutline(_ target: PageDropTarget?) {
        guard let target, let content = parent.contentView else {
            outline.show(nil)
            outlinePanel.place(in: parent, nil)
            return
        }
        let left = sidebar?.maxX ?? 0
        outline.tint = NSColor(model.accent)
        outline.show(target)
        outlinePanel.place(
            in: parent,
            NSRect(
                x: left, y: 0, width: max(0, content.bounds.width - left),
                height: content.bounds.height),
            raise: false)
    }

    /// The link's drag is over.
    private func endLink() {
        link = nil
        view.show(nil)
        preview(nil)
        if model.dragged == nil { panel.place(in: parent, nil) }
    }

    func close() {
        linkWatch?.invalidate()
        outlinePanel.close()
        timer?.invalidate()
        panel.close()
        edgePanel.close()
    }

    /// Like Arc's, the page shown alone gives a tab dragged over it the room it would take.
    /// A pane's frame in the drop's view, from where the app placed the card.
    private func frame(_ pane: Pane) -> NSRect {
        NSRect(
            x: pane.cardX + pane.x - (sidebar?.maxX ?? 0), y: pane.cardY + pane.y,
            width: pane.width, height: pane.height)
    }

    /// Over a split, like Arc's, an empty page opens among its pages where the tab would go.
    private func preview(_ target: PageDropTarget?, leaving: Bool = false) {
        // A link crossing the page's middle, which the page keeps, from one edge to the other:
        // the room it would take stays, rather than the page jumping back and forth.
        if target == nil, link != nil, !leaving { return }
        // A link's outline, which stays meanwhile too.
        if link != nil || target == nil { showOutline(target) }
        let page: String
        if let target, stable.count < 2 {
            page = command("preview-drop", [target.before, target.stacked])
        } else {
            page = command("preview-drop")
        }
        if page != previewedPage {
            previewedPage = page
            model.run(command: page)
        }
        let pane = target?.index.map { command("preview-pane", $0) } ?? command("preview-pane")
        if pane != previewed {
            previewed = pane
            model.run(command: pane)
        }
    }
    private var previewedPage = command("preview-drop")
    /// The card's frame, from the window's top left corner, before it gave a drag room.
    private var stableCard: NSRect?
    private var previewed = command("preview-pane")

    /// What a drag shows over the page, like Arc's: a dark card with the tab's icon and title.
    private func cardImage() -> NSImage {
        let tab = model.dragged.flatMap { model.item($0) }?.tabs.first
        let size = cardSize
        return NSImage(size: size, flipped: true) { rect in
            let shape = NSBezierPath(roundedRect: rect, xRadius: 14, yRadius: 14)
            NSColor.black.withAlphaComponent(0.92).setFill()
            shape.fill()
            let icon: CGFloat = 20
            let top = rect.midY - 28
            uiImage(tab?.favicon)?.draw(
                in: NSRect(x: rect.midX - icon / 2, y: top, width: icon, height: icon))
            let style = NSMutableParagraphStyle()
            style.alignment = .center
            style.lineBreakMode = .byTruncatingTail
            NSString(string: tab?.label ?? "").draw(
                with: NSRect(x: 8, y: top + icon + 6, width: rect.width - 16, height: 44),
                options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine],
                attributes: [
                    .font: NSFont.systemFont(ofSize: 12, weight: .semibold),
                    .foregroundColor: NSColor.white,
                    .paragraphStyle: style,
                ])
            return true
        }
    }

    private func update() {
        timer?.invalidate()
        timer = nil
        guard model.dragged != nil, let content = parent.contentView else {
            preview(nil)
            panel.place(in: parent, nil)
            edgePanel.place(in: parent, nil)
            return
        }
        view.tint = NSColor(model.accent)
        view.show(nil)
        let left = sidebar?.maxX ?? 0
        panel.place(
            in: parent,
            NSRect(
                x: left, y: 0, width: max(0, content.bounds.width - left),
                height: content.bounds.height))
        edgePanel.place(
            in: parent,
            sidebar.map {
                NSRect(x: $0.minX - leadingEdge, y: $0.minY, width: leadingEdge, height: $0.height)
            })
        // A drag that ends anywhere else leaves the mouse's buttons up; a drop takes a moment
        // after they come up, which must still find the drag.
        var released: Date?
        timer = Timer.scheduledTimer(withTimeInterval: 0.1, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard NSEvent.pressedMouseButtons == 0, let self else {
                    released = nil
                    return
                }
                let since = released ?? Date()
                released = since
                guard Date().timeIntervalSince(since) >= 0.5 else { return }
                self.model.dragged = nil
                self.model.dropTarget = nil
            }
        }
    }

    /// Where a tab dropped at `point`, from the view's top left corner, would go.
    private func target(at point: NSPoint) -> PageDropTarget? {
        // Just beside the docked sidebar, the next Space shows, for a tab.
        let linking = link != nil && model.dragged == nil
        let beside = sidebar != nil && point.x < trailingEdge
        if !linking { model.edge(beside ? false : nil) }
        guard !beside, linking || model.draggedTab else { return nil }
        let dragged = model.dragged ?? -1
        let panes = stable
        if panes.count > 1 {
            guard !panes.contains(where: { $0.tab == dragged }) else { return nil }
            let stacked = panes[0].x == panes[1].x
            let frames = panes.map(frame)
            for (index, frame) in frames.enumerated() {
                guard frame.contains(point) else { continue }
                // Like Arc's: its first quarter along the split puts the tab before it, its last
                // after it, which between two pages is the same place, and its middle replaces it.
                let share = stacked
                    ? (point.y - frame.minY) / frame.height : (point.x - frame.minX) / frame.width
                // A link, near the page's edges only: elsewhere, the page takes it.
                if linking {
                    guard panes.count < 4, share <= 0.25 || share >= 0.75 else { return nil }
                    let before = share <= 0.25
                    return PageDropTarget(
                        tab: panes[index].tab, before: before, stacked: stacked, preview: .zero,
                        index: index + (before ? 0 : 1))
                }
                if share > 0.25 && share < 0.75 {
                    return PageDropTarget(
                        tab: panes[index].tab, before: true, stacked: stacked, preview: frame,
                        replace: true)
                }
                // A split holds four pages at most.
                guard panes.count < 4 else { return nil }
                let before = share <= 0.25
                let at = index + (before ? 0 : 1)
                // The app opens an empty page there, which it tells the place of (emptyPane).
                return PageDropTarget(
                    tab: panes[index].tab, before: before, stacked: stacked, preview: .zero,
                    index: at)
            }
            return nil
        }
        guard let selected = model.page?.selected, selected != dragged else { return nil }
        // Arc's edges: up to 50 points at the top and the bottom, then 45% at either side, of
        // the card as it stood before it gave the tab room, in the view's coordinates.
        if previewedPage == command("preview-drop") { stableCard = model.card }
        let room =
            stableCard?.offsetBy(dx: -(sidebar?.maxX ?? 0), dy: 0)
            ?? view.bounds.insetBy(dx: cardMargin, dy: cardMargin)
        let band = min(room.height / 3, linking ? linkBand : 50)
        let side = linking ? min(linkSide, room.width * 0.25) : room.width * 0.45
        let (top, bottom) = (point.y < room.minY + band, point.y > room.maxY - band)
        let (leading, trailing) = (point.x < room.minX + side, point.x > room.maxX - side)
        guard top || bottom || leading || trailing else { return nil }
        let stacked = top || bottom
        let before = stacked ? top : leading
        // The half the page gives up, as the app lays it out (SPLIT_GAP, 10).
        var half = room
        if stacked {
            half.size.height = (room.height - splitGap) / 2
            if !before { half.origin.y = room.maxY - half.height }
        } else {
            half.size.width = (room.width - splitGap) / 2
            if !before { half.origin.x = room.maxX - half.width }
        }
        return PageDropTarget(tab: selected, before: before, stacked: stacked, preview: half)
    }

    private func drop(_ target: PageDropTarget) {
        preview(nil, leaving: true)
        if let link, model.dragged == nil {
            model.run("drop-link-beside", ["url": link, "tab": target.tab, "before": target.before])
            if target.stacked, panes.count < 2 { model.run("flip-panes") }
            endLink()
            return
        }
        guard let dragged = model.dragged else { return }
        let (model, stable, alone) = (model, stable, panes.count < 2)
        model.land {
            // A full split's page replaced leaves first, the tab joining beside its neighbor.
            if target.replace, stable.count >= 4,
                let index = stable.firstIndex(where: { $0.tab == target.tab })
            {
                let next = index + 1 < stable.count
                model.run("separate", target.tab)
                model.split(dragged, onto: stable[next ? index + 1 : index - 1].tab, before: next)
            } else {
                model.split(dragged, onto: target.tab, before: target.before)
                // Replaced, the tab leaves the split, as a row after it.
                if target.replace { model.run("separate", target.tab) }
            }
            // A split made of a page shown alone lays its pages out as the drop asked.
            if target.stacked, alone { model.run("flip-panes") }
        }
        model.dragged = nil
        model.dropTarget = nil
    }
}

/// The room left of the sidebar, where a drag shows the previous Space; it takes no drop.
private final class EdgeView: NSView {
    var rested: (Bool) -> Void = { _ in }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        draggingUpdated(sender)
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        rested(true)
        return []
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        rested(false)
    }
}

/// The view taking the drop, which draws where the tab would go.
private final class DropView: NSView {
    var locate: (NSPoint) -> PageDropTarget? = { _ in nil }
    var drop: (PageDropTarget) -> Void = { _ in }
    /// Called as where the tab would go changes.
    var shown: (PageDropTarget?) -> Void = { _ in }
    /// What the drag shows over the page.
    var card: () -> NSImage = { NSImage() }
    /// Whether the drag turns into the card over the page: a tab's, not a link's.
    var morphs: () -> Bool = { true }
    /// Whether it draws where the drag would go: not for a link, whose outline is drawn apart.
    var draws: () -> Bool = { true }
    var tint = NSColor.controlAccentColor
    /// Where the empty page the app opened for the drag stands, over a split.
    var room: NSRect? { didSet { needsDisplay = true } }
    private var target: PageDropTarget?
    private let shape = DragShape()

    override var isFlipped: Bool { true }

    func show(_ target: PageDropTarget?) {
        guard target != self.target else { return }
        self.target = target
        shown(target)
        needsDisplay = true
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        draggingUpdated(sender)
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation {
        show(locate(convert(sender.draggingLocation, from: nil)))
        // Like Arc's, over the page the drag turns into a card, following the pointer.
        if target != nil {
            if morphs() { shape.morph(sender, in: self, size: cardSize, image: card()) }
        } else {
            shape.restore(sender, in: self)
        }
        return target == nil ? [] : .move
    }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        show(nil)
        if let sender { shape.restore(sender, in: self) }
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        shape.forget()
        guard let target else { return false }
        show(nil)
        drop(target)
        return true
    }

    /// Like Arc's, the room the tab would take shows as an empty page, outlined; a page it would
    /// replace dims under it.
    override func draw(_ dirtyRect: NSRect) {
        guard let target, draws() else { return }
        // A page it would replace takes an outline in the theme's color, like Arc's.
        if target.replace {
            let page = NSBezierPath(
                roundedRect: target.preview.insetBy(dx: 1, dy: 1), xRadius: 10, yRadius: 10)
            tint.setStroke()
            page.lineWidth = 2
            page.stroke()
            return
        }
        // Over a split, the empty page the app opened; over a page alone, the half it gave up.
        let place = target.index == nil ? target.preview : room
        guard let place else { return }
        let page = NSBezierPath(
            roundedRect: place.insetBy(dx: 0.75, dy: 0.75), xRadius: 10, yRadius: 10)
        NSColor.white.withAlphaComponent(0.04).setFill()
        page.fill()
        NSColor.white.withAlphaComponent(0.35).setStroke()
        page.lineWidth = 1.5
        page.stroke()
    }
}

/// The card a drag shows over the page, about a quarter of the window high like Arc's.
private let cardSize = CGSize(width: 92, height: 200)
