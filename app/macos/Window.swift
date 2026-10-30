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

/// The sidebar's place in the window: docked, it shows the backdrop through; floating above the
/// page, it blurs the page under it and casts a shadow.
@MainActor
final class SidebarHost {
    private let content: NSView
    private let container = NSView()
    private let effect = NSVisualEffectView()
    private let host: NSHostingView<SidebarView>
    /// While the sidebar hides, its edge of the window shows it again on hover.
    private let edge = Tracking()
    private var watch: AnyCancellable?

    init(content: NSView, model: AppModel) {
        self.content = content
        host = NSHostingView(rootView: SidebarView(model: model))
        effect.material = .sidebar
        effect.blendingMode = .withinWindow
        effect.state = .active
        effect.autoresizingMask = [.width, .height]
        host.autoresizingMask = [.width, .height]
        container.wantsLayer = true
        container.addSubview(effect)
        container.addSubview(host)
        container.isHidden = true
        content.addSubview(container)
        edge.entered = { model.run("sidebar/reveal") }
        edge.isHidden = true
        content.addSubview(edge)
        watch = model.$state.sink { [container] _ in
            container.appearance = NSAppearance(
                named: isDark(model.space?.theme) ? .darkAqua : .aqua)
        }
        let leave = Tracking()
        leave.autoresizingMask = [.width, .height]
        leave.exited = { [weak self] in
            if self?.revealed == true { model.run("sidebar/conceal") }
        }
        container.addSubview(leave, positioned: .below, relativeTo: effect)
    }

    private var revealed = false

    /// Shows the sidebar at `frame`, from the window's top left corner, or hides it.
    func place(_ frame: NSRect?, floating: Bool, revealed: Bool) {
        self.revealed = revealed
        let bounds = content.bounds
        edge.frame = NSRect(x: 0, y: 0, width: 8, height: bounds.height)
        edge.autoresizingMask = [.height]
        edge.isHidden = frame != nil
        guard let frame else {
            container.isHidden = true
            return
        }
        let y = content.isFlipped ? frame.minY : bounds.height - frame.maxY
        container.frame = NSRect(x: frame.minX, y: y, width: frame.width, height: frame.height)
        container.autoresizingMask = floating ? [] : [.height]
        effect.frame = container.bounds
        host.frame = container.bounds
        effect.isHidden = !floating
        container.layer?.cornerRadius = floating ? 10 : 0
        container.layer?.masksToBounds = floating
        container.isHidden = false
        container.subviews.forEach { $0.frame = container.bounds }
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
