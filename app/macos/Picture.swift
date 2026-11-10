import AppKit

/// The height of the bar a hovered Picture in Picture window shows over its page's top.
private let pictureBarHeight: CGFloat = 28

/// A page's Picture in Picture window (the Document Picture-in-Picture API, which Google Meet's
/// calls use), like Arc's: only its page, with rounded corners, floating over every app and
/// Space, without bringing the app in front. Hovered, a bar over its page's top moves it and has
/// a button going back to its tab, which closes it: the same on every site.
@MainActor
final class PictureWindow: NSObject, NSWindowDelegate {
    let id: Int
    let window: NSPanel
    let page = PageHost()

    init(id: Int, size: NSSize) {
        self.id = id
        window = PicturePanel(
            contentRect: NSRect(origin: .zero, size: size),
            styleMask: [.borderless, .resizable, .nonactivatingPanel],
            backing: .buffered, defer: false)
        super.init()
        window.title = "Picture in Picture"
        window.isReleasedWhenClosed = false
        window.level = .floating
        window.hidesOnDeactivate = false
        window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary]
        window.isOpaque = false
        window.backgroundColor = .clear
        window.hasShadow = true
        let content = PictureContent(frame: NSRect(origin: .zero, size: size))
        content.wantsLayer = true
        content.layer?.cornerRadius = 10
        content.layer?.masksToBounds = true
        content.layer?.backgroundColor = NSColor.black.cgColor
        page.frame = content.bounds
        page.autoresizingMask = [.width, .height]
        content.addSubview(page)
        let bar = PictureBar(
            frame: NSRect(
                x: 0, y: size.height - pictureBarHeight, width: size.width,
                height: pictureBarHeight))
        bar.autoresizingMask = [.width, .minYMargin]
        let back = NSButton(
            image: NSImage(systemSymbolName: "pip.exit", accessibilityDescription: "Back to Tab")!,
            target: self, action: #selector(backToTab))
        back.isBordered = false
        back.contentTintColor = .white
        back.frame = NSRect(
            x: size.width - pictureBarHeight - 2, y: 0, width: pictureBarHeight,
            height: pictureBarHeight)
        back.autoresizingMask = [.minXMargin]
        back.toolTip = "Back to Tab"
        bar.addSubview(back)
        content.bar = bar
        content.addSubview(bar)
        window.contentView = content
        window.delegate = self
        // Where the user last left one, else at the screen's bottom right, like Chromium's.
        if !window.setFrameUsingName("Picture in Picture"), let screen = NSScreen.main {
            let area = screen.visibleFrame
            window.setFrameOrigin(NSPoint(x: area.maxX - size.width - 20, y: area.minY + 20))
        }
        window.setFrameAutosaveName("Picture in Picture")
        window.orderFrontRegardless()
    }

    @objc private func backToTab() {
        arbalete_picture_back(UInt32(id))
    }

    /// Its page closes first, which closes the window (arbalete_ui_picture_closed).
    func windowShouldClose(_ sender: NSWindow) -> Bool {
        arbalete_picture_should_close(UInt32(id))
    }
}

/// A panel without a title bar that takes the keyboard, for the page's own fields, like a
/// call's chat.
private final class PicturePanel: NSPanel {
    override var canBecomeKey: Bool { true }
}

/// A Picture in Picture window's content: its page, under its bar showing while hovered.
private final class PictureContent: NSView {
    var bar: NSView? {
        didSet { show(false) }
    }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(
            NSTrackingArea(
                rect: .zero, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                owner: self))
    }

    override func mouseEntered(with event: NSEvent) { show(true) }
    override func mouseExited(with event: NSEvent) { show(false) }

    private func show(_ shown: Bool) {
        NSAnimationContext.runAnimationGroup { context in
            context.duration = 0.15
            bar?.animator().alphaValue = shown ? 1 : 0
        }
    }
}

/// The bar over a hovered Picture in Picture window's page: dragged, it moves the window, and
/// its handle shows it does. Gone, it lets the page have every click.
private final class PictureBar: NSVisualEffectView {
    override init(frame: NSRect) {
        super.init(frame: frame)
        material = .hudWindow
        blendingMode = .withinWindow
        state = .active
        appearance = NSAppearance(named: .darkAqua)
        let handle = NSView(
            frame: NSRect(x: (frame.width - 36) / 2, y: (frame.height - 4) / 2, width: 36, height: 4))
        handle.wantsLayer = true
        handle.layer?.cornerRadius = 2
        handle.layer?.backgroundColor = NSColor.white.withAlphaComponent(0.7).cgColor
        handle.autoresizingMask = [.minXMargin, .maxXMargin]
        addSubview(handle)
    }

    required init?(coder: NSCoder) { nil }

    override func hitTest(_ point: NSPoint) -> NSView? {
        alphaValue == 0 ? nil : super.hitTest(point)
    }

    override func mouseDown(with event: NSEvent) {
        window?.performDrag(with: event)
    }
}

@MainActor var pictures: [Int: PictureWindow] = [:]

/// Opens the Picture in Picture window `id`, its page `width` by `height` points, and returns
/// the view CEF draws its page in.
@_cdecl("arbalete_ui_open_picture")
public func openPicture(_ id: UInt32, _ width: Int32, _ height: Int32) -> UnsafeMutableRawPointer? {
    // As an address: a pointer may not leave the main actor's closure.
    let address: UInt = MainActor.assumeIsolated {
        let picture = PictureWindow(
            id: Int(id), size: NSSize(width: CGFloat(width), height: CGFloat(height)))
        pictures[Int(id)] = picture
        return UInt(bitPattern: Unmanaged.passUnretained(picture.page).toOpaque())
    }
    return UnsafeMutableRawPointer(bitPattern: address)
}

/// Closes the Picture in Picture window `id`, its page closing: its view goes with the window.
@_cdecl("arbalete_ui_picture_closed")
public func pictureClosed(_ id: UInt32) {
    MainActor.assumeIsolated { pictures.removeValue(forKey: Int(id))?.window.close() }
}
