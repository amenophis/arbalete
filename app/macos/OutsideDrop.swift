// A tab dragged from the sidebar out of every window, like Arc's: a card follows the pointer
// there, the drop landing on it, and the tab opens in a blank window at that place.

import AppKit

/// How far beside a window, at its left, a drag still counts as over it, like Arc's: its sidebar's
/// edge switches Spaces there (PageDrop).
private let windowSlop: CGFloat = 80
/// The card the drag turns into out of the windows, like Arc's.
private let cardSide: CGFloat = 180
/// Where the blank window opens, from the pointer: its sidebar's top under it, like Arc's.
private let windowOffset = CGPoint(x: 120, y: 20)

/// The panel under the pointer while a drag from the sidebar is out of every window, which takes
/// its drop: one for the app.
@MainActor
final class OutsideDrop {
    static let shared = OutsideDrop()

    private let panel: NSPanel
    private let view = CatchView()
    private var timer: Timer?

    private init() {
        panel = NSPanel(
            contentRect: NSRect(x: 0, y: 0, width: cardSide, height: cardSide),
            styleMask: [.borderless, .nonactivatingPanel], backing: .buffered, defer: true)
        panel.isOpaque = false
        panel.backgroundColor = .clear
        panel.hasShadow = false
        panel.level = .floating
        panel.isFloatingPanel = true
        panel.hidesOnDeactivate = false
        panel.isReleasedWhenClosed = false
        panel.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle]
        view.registerForDraggedTypes([.sidebarNode])
        panel.contentView = view
    }

    /// A drag starts from `model`'s sidebar: unless it carries a folder, a favorite or an
    /// incognito window's tab, like Arc's, the panel follows it out of the windows until the
    /// mouse's buttons come up.
    func start(from model: AppModel) {
        guard let item = model.draggedItem, model.state?.incognito != true,
            !(model.page?.favorites.contains { $0.id == item.id } ?? false)
        else { return }
        if case .folder = item { return }
        view.card = card(item.tabs.first)
        timer?.invalidate()
        // A drop takes a moment after the buttons come up, which must still find the panel.
        var released: Date?
        let timer = Timer(timeInterval: 1.0 / 60, repeats: true) { [weak self] _ in
            MainActor.assumeIsolated {
                guard let self else { return }
                if NSEvent.pressedMouseButtons == 0 {
                    let since = released ?? Date()
                    released = since
                    if Date().timeIntervalSince(since) >= 0.5 { self.stop() }
                    return
                }
                self.follow()
            }
        }
        // Also while AppKit tracks the drag.
        RunLoop.main.add(timer, forMode: .common)
        self.timer = timer
    }

    private func stop() {
        timer?.invalidate()
        timer = nil
        panel.orderOut(nil)
    }

    /// Shows the panel under the pointer out of every window, else hides it.
    private func follow() {
        let point = NSEvent.mouseLocation
        let over = shownMainWindows().contains { window in
            var frame = window.frame
            frame.origin.x -= windowSlop
            frame.size.width += windowSlop
            return frame.contains(point)
        }
        guard !over else {
            panel.orderOut(nil)
            return
        }
        panel.setFrameOrigin(NSPoint(x: point.x - cardSide / 2, y: point.y - cardSide / 2))
        if !panel.isVisible { panel.orderFront(nil) }
    }

    /// Opens what the drag carries in a blank window, at `point` on the screen.
    fileprivate func drop(at point: NSPoint) {
        stop()
        guard let model = AppModel.sidebarDrag, let item = model.draggedItem else { return }
        let tabs = model.draggedTab ? model.draggedTabs : item.tabs.first.map { [$0.id] } ?? []
        let screen = NSScreen.screens.first { $0.frame.contains(point) } ?? NSScreen.main
        guard !tabs.isEmpty, let screen, let name = uuid(of: screen) else { return }
        let size = model.host?.frame.size ?? CGSize(width: 1200, height: 800)
        let left = point.x - windowOffset.x
        let top = point.y + windowOffset.y
        model.marked = []
        _ = model.endDrag()
        let command = command(
            "drop-outside",
            [
                "tabs": tabs,
                "place": [
                    "screen": name, "x": left - screen.frame.minX, "y": screen.frame.maxY - top,
                    "width": size.width, "height": size.height,
                ],
            ])
        // Once AppKit is done with the drag: a window made within it shows nothing.
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) { model.run(command: command) }
    }

    /// The card the drag shows out of the windows, like Arc's: the tab's icon over its title.
    private func card(_ tab: Tab?) -> NSImage {
        let size = CGSize(width: cardSide, height: cardSide)
        return NSImage(size: size, flipped: true) { rect in
            let shape = NSBezierPath(roundedRect: rect, xRadius: 16, yRadius: 16)
            NSColor.black.withAlphaComponent(0.85).setFill()
            shape.fill()
            let icon: CGFloat = 32
            let top = rect.midY - 30
            uiImage(tab?.favicon)?.draw(
                in: NSRect(x: rect.midX - icon / 2, y: top, width: icon, height: icon))
            let style = NSMutableParagraphStyle()
            style.alignment = .center
            style.lineBreakMode = .byTruncatingTail
            NSString(string: tab?.label ?? "").draw(
                with: NSRect(x: 12, y: top + icon + 10, width: rect.width - 24, height: 36),
                options: [.usesLineFragmentOrigin, .truncatesLastVisibleLine],
                attributes: [
                    .font: NSFont.systemFont(ofSize: 13, weight: .semibold),
                    .foregroundColor: NSColor.white,
                    .paragraphStyle: style,
                ])
            return true
        }
    }
}

/// The panel's view, which shows the card: macOS lets a drag through a window that draws nothing.
/// Over it, the drag's own image hides behind the card, and drops there.
private final class CatchView: NSView {
    var card = NSImage() { didSet { needsDisplay = true } }
    private let shape = DragShape()

    override func draw(_ dirtyRect: NSRect) {
        card.draw(in: bounds)
    }

    override func draggingEntered(_ sender: any NSDraggingInfo) -> NSDragOperation {
        // Like Arc's, a little tap as the card shows.
        NSHapticFeedbackManager.defaultPerformer.perform(.generic, performanceTime: .now)
        shape.morph(sender, in: self, size: CGSize(width: 1, height: 1), image: NSImage())
        return .move
    }

    override func draggingUpdated(_ sender: any NSDraggingInfo) -> NSDragOperation { .move }

    override func draggingExited(_ sender: (any NSDraggingInfo)?) {
        if let sender { shape.restore(sender, in: self) }
    }

    override func performDragOperation(_ sender: any NSDraggingInfo) -> Bool {
        shape.forget()
        let point = window?.convertPoint(toScreen: sender.draggingLocation) ?? NSEvent.mouseLocation
        MainActor.assumeIsolated { OutsideDrop.shared.drop(at: point) }
        return true
    }
}
