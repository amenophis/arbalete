// What the interface puts in the main window: the blur behind its views, and the sidebar above
// them, docked at its left edge or floating above the page.

import AppKit
import Combine
import SwiftUI

/// Under the window's views, which leave it showing where they are transparent: the desktop
/// blurred through the window, tinted by the displayed Space's theme.
@MainActor
final class Backdrop {
    private let effect: NSVisualEffectView
    private var watch: AnyCancellable?

    init(content: NSView, model: AppModel) {
        effect = NSVisualEffectView(frame: content.frame)
        effect.material = .underWindowBackground
        effect.blendingMode = .behindWindow
        effect.state = .followsWindowActiveState
        effect.autoresizingMask = [.width, .height]
        let tint = NSHostingView(rootView: BackdropTint(model: model))
        tint.frame = effect.bounds
        tint.autoresizingMask = [.width, .height]
        effect.addSubview(tint)
        content.superview?.addSubview(effect, positioned: .below, relativeTo: content)
        watch = model.$state.sink { [effect] _ in
            effect.appearance = NSAppearance(named: isDark(model.space?.theme) ? .darkAqua : .aqua)
        }
    }
}

struct BackdropTint: View {
    @ObservedObject var model: AppModel

    var body: some View {
        Tint(theme: model.space?.theme).ignoresSafeArea()
    }
}

/// The sidebar, in a child window of the main one, whose own views Chromium keeps every click
/// from: docked, it shows the backdrop through, and its transparent corner the window's buttons,
/// which clicks there reach; floating above the page, it blurs the page under it.
@MainActor
final class SidebarHost {
    private let window: NSWindow
    private let panel = ChildPanel(view: NSView())
    private let container = NSView()
    private let effect = NSVisualEffectView()
    /// While the sidebar hides, its edge of the window shows it again on hover.
    private let edge = Tracking()
    private var watch: AnyCancellable?
    private var revealed = false

    init(window: NSWindow, model: AppModel) {
        self.window = window
        container.wantsLayer = true
        effect.material = .sidebar
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.autoresizingMask = [.width, .height]
        let host = NSHostingView(rootView: SidebarView(model: model))
        host.autoresizingMask = [.width, .height]
        let leave = Tracking()
        leave.autoresizingMask = [.width, .height]
        container.addSubview(leave)
        container.addSubview(effect)
        container.addSubview(host)
        leave.exited = { [weak self] in
            if self?.revealed == true { model.run("sidebar/conceal") }
        }
        edge.entered = { model.run("sidebar/reveal") }
        edge.isHidden = true
        edge.autoresizingMask = [.height]
        window.contentView?.addSubview(edge)
        watch = model.$state.sink { [container] _ in
            container.appearance = NSAppearance(
                named: isDark(model.space?.theme) ? .darkAqua : .aqua)
        }
    }

    /// Shows the sidebar at `frame`, from the window's top left corner, or hides it.
    func place(_ frame: NSRect?, floating: Bool, revealed: Bool) {
        self.revealed = revealed
        if let content = window.contentView {
            edge.frame = NSRect(x: 0, y: 0, width: 8, height: content.bounds.height)
        }
        edge.isHidden = frame != nil
        effect.isHidden = !floating
        container.layer?.cornerRadius = floating ? 10 : 0
        container.layer?.masksToBounds = floating
        panel.hasShadow = floating
        panel.place(in: window, frame)
        if container.superview != panel.contentView {
            panel.contentView?.addSubview(container)
        }
        container.frame = panel.contentView?.bounds ?? .zero
        container.autoresizingMask = [.width, .height]
        container.subviews.forEach { $0.frame = container.bounds }
        panel.invalidateShadow()
    }
}

/// A view that says when the pointer enters or leaves it.
final class Tracking: NSView {
    var entered: () -> Void = {}
    var exited: () -> Void = {}

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        trackingAreas.forEach(removeTrackingArea)
        addTrackingArea(
            NSTrackingArea(
                rect: bounds, options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
                owner: self))
    }

    override func mouseEntered(with event: NSEvent) { entered() }
    override func mouseExited(with event: NSEvent) { exited() }
}
