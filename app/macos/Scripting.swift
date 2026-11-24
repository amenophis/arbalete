// AppleScript, like Arc's (decision 0017): scripts and automation tools (Shortcuts, Raycast,
// Alfred) list the windows, their Spaces and their tabs, show and close them, open tabs and run
// JavaScript in their pages. The terms are the bundle's dictionary (app/resources/mac/Arbalete.sdef);
// the objects read each window's state as its interface has it, and act through its commands.

import AppKit

/// Why a script's command failed, for AppleScript's error.
private let commandFailed = -10000

private func fail(_ command: NSScriptCommand, _ message: String) {
    command.scriptErrorNumber = commandFailed
    command.scriptErrorString = message
}

/// The four characters of an Apple event code, as the dictionary gives them.
private func code(_ text: StaticString) -> FourCharCode {
    text.withUTF8Buffer { $0.reduce(0) { $0 << 8 | FourCharCode($1) } }
}

/// The tabs of a Space's page, in sidebar order.
private func tabs(of page: SpacePage) -> [Tab] {
    (page.favorites + page.pinned + page.today).flatMap(\.tabs)
}

extension NSApplication {
    /// The main windows, front to back.
    @objc var scriptWindows: [ScriptWindow] {
        let ids = Dictionary(
            nativeWindows.compactMap { id, native in native.window.map { (ObjectIdentifier($0), id) } },
            uniquingKeysWith: { first, _ in first })
        return orderedWindows.compactMap { ids[ObjectIdentifier($0)] }.map(ScriptWindow.init)
    }

    @objc func valueInScriptWindows(withUniqueID id: NSNumber) -> ScriptWindow? {
        nativeWindows[id.uint32Value] == nil ? nil : ScriptWindow(id: id.uint32Value)
    }
}

@MainActor @objc(ArbaleteScriptWindow)
final class ScriptWindow: NSObject {
    let id: UInt32

    nonisolated init(id: UInt32) {
        self.id = id
    }

    var native: NativeWindow? { nativeWindows[id] }
    var model: AppModel? { native?.model }

    @objc var uniqueID: Int { Int(id) }
    @objc var name: String { model?.activeTab?.label ?? native?.window?.title ?? "" }
    @objc var index: Int { (NSApp.scriptWindows.firstIndex { $0.id == id } ?? -1) + 1 }
    @objc var visible: Bool { native?.window?.isVisible ?? false }
    @objc var incognito: Bool { model?.state?.incognito ?? false }

    @objc var activeTab: ScriptTab? {
        model?.activeTab.map { ScriptTab(id: $0.id, window: id, space: nil) }
    }

    @objc var activeSpace: ScriptSpace? {
        model?.state.map { ScriptSpace(id: $0.space, window: id) }
    }

    @objc var scriptTabs: [ScriptTab] {
        (model?.page).map(tabs).map { $0.map { ScriptTab(id: $0.id, window: id, space: nil) } }
            ?? []
    }

    @objc var scriptSpaces: [ScriptSpace] {
        (model?.state?.spaces ?? []).map { ScriptSpace(id: $0.id, window: id) }
    }

    @objc func valueInScriptTabs(withUniqueID id: NSNumber) -> ScriptTab? {
        scriptTabs.first { $0.id == id.intValue }
    }

    @objc func valueInScriptSpaces(withUniqueID id: NSNumber) -> ScriptSpace? {
        scriptSpaces.first { $0.id == id.intValue }
    }

    nonisolated override var objectSpecifier: NSScriptObjectSpecifier? {
        NSUniqueIDSpecifier(
            containerClassDescription: NSScriptClassDescription(for: NSApplication.self)!,
            containerSpecifier: nil, key: "scriptWindows", uniqueID: Int(id))
    }

    @objc func scriptClose(_ command: NSScriptCommand) -> Any? {
        native?.window?.performClose(nil)
        return nil
    }

    /// Brings the window in front of the others, as what a script shows in it.
    func bringForward() {
        native?.window?.makeKeyAndOrderFront(nil)
    }
}

@MainActor @objc(ArbaleteScriptSpace)
final class ScriptSpace: NSObject {
    let id: Int
    let window: UInt32

    nonisolated init(id: Int, window: UInt32) {
        self.id = id
        self.window = window
    }

    private var model: AppModel? { nativeWindows[window]?.model }

    @objc var uniqueID: Int { id }
    @objc var title: String { model?.state?.spaces.first { $0.id == id }?.title ?? "" }

    @objc var scriptTabs: [ScriptTab] {
        let page = model?.state?.pages.first { $0.space == id }
        return page.map(tabs).map { $0.map { ScriptTab(id: $0.id, window: window, space: id) } }
            ?? []
    }

    @objc func valueInScriptTabs(withUniqueID id: NSNumber) -> ScriptTab? {
        scriptTabs.first { $0.id == id.intValue }
    }

    nonisolated override var objectSpecifier: NSScriptObjectSpecifier? {
        NSUniqueIDSpecifier(
            containerClassDescription: NSScriptClassDescription(for: ScriptWindow.self)!,
            containerSpecifier: ScriptWindow(id: window).objectSpecifier, key: "scriptSpaces",
            uniqueID: id)
    }

    @objc func scriptFocus(_ command: NSScriptCommand) -> Any? {
        model?.run("show-space", id)
        ScriptWindow(id: window).bringForward()
        return nil
    }
}

@MainActor @objc(ArbaleteScriptTab)
final class ScriptTab: NSObject {
    let id: Int
    let window: UInt32
    /// The Space it was found in, else the window's displayed one.
    let space: Int?

    nonisolated init(id: Int, window: UInt32, space: Int?) {
        self.id = id
        self.window = window
        self.space = space
    }

    private var model: AppModel? { nativeWindows[window]?.model }

    private var tab: Tab? {
        model?.state?.pages.lazy.flatMap(tabs).first { $0.id == self.id }
    }

    @objc var uniqueID: Int { id }
    @objc var title: String { tab?.title ?? "" }

    @objc var address: String {
        get { tab?.url ?? "" }
        set {
            model?.run("select", id)
            model?.run("navigate", newValue)
        }
    }

    nonisolated override var objectSpecifier: NSScriptObjectSpecifier? {
        let container: NSObject =
            space.map { ScriptSpace(id: $0, window: window) } ?? ScriptWindow(id: window)
        return NSUniqueIDSpecifier(
            containerClassDescription: NSScriptClassDescription(for: type(of: container))!,
            containerSpecifier: container.objectSpecifier, key: "scriptTabs", uniqueID: id)
    }

    @objc func scriptSelect(_ command: NSScriptCommand) -> Any? {
        model?.run("select", id)
        ScriptWindow(id: window).bringForward()
        return nil
    }

    @objc func scriptClose(_ command: NSScriptCommand) -> Any? {
        model?.run("close", id)
        return nil
    }

    @objc func scriptExecute(_ command: NSScriptCommand) -> Any? {
        guard let script = command.evaluatedArguments?["javascript"] as? String else {
            fail(command, "The script is missing.")
            return nil
        }
        let context = Unmanaged.passRetained(command).toOpaque()
        command.suspendExecution()
        let started = script.withCString {
            arbalete_execute_javascript(UInt64(id), $0, javascriptDone, context)
        }
        if !started {
            Unmanaged<NSScriptCommand>.fromOpaque(context).release()
            fail(command, "The tab's page is not loaded: select the tab first.")
            command.resumeExecution(withResult: nil)
        }
        return nil
    }
}

/// Gives a script's result to the command that ran it (scriptExecute).
private let javascriptDone: @convention(c) (UnsafeMutableRawPointer?, UnsafePointer<CChar>?, Bool)
    -> Void = { context, result, ok in
        guard let context else { return }
        let command = Unmanaged<NSScriptCommand>.fromOpaque(context).takeRetainedValue()
        let text = result.map { String(cString: $0) }
        if !ok { fail(command, text ?? "The script failed.") }
        command.resumeExecution(withResult: ok ? text : nil)
    }

/// `make new tab with properties {URL: …}`, in a window or a Space, or `make new window`.
@objc(ArbaleteScriptMake)
final class ScriptMake: NSCreateCommand {
    override func performDefaultImplementation() -> Any? {
        MainActor.assumeIsolated { make() }
    }

    @MainActor private func make() -> Any? {
        let container = (arguments?["Location"] as? NSPositionalSpecifier)?.insertionContainer
        let space = container as? ScriptSpace
        guard
            let window = (container as? ScriptWindow)
                ?? space.map({ ScriptWindow(id: $0.window) }) ?? NSApp.scriptWindows.first,
            let model = window.model
        else {
            fail(self, "No window is open.")
            return nil
        }
        switch createClassDescription.appleEventCode {
        case code("cwin"):
            model.run("new-window")
            return nil
        case code("Atab"):
            guard let url = resolvedKeyDictionary["address"] as? String, !url.isEmpty else {
                fail(self, "A new tab needs its URL: make new tab with properties {URL: \"…\"}.")
                return nil
            }
            if let space { model.run("show-space", space.id) }
            let before = model.activeTab?.id
            model.run("open", url)
            // The app sends the window its state as the tab opens.
            guard let tab = model.activeTab, tab.id != before else { return nil }
            // A reference to it, as its result's type says.
            return ScriptTab(id: tab.id, window: window.id, space: space?.id).objectSpecifier
        default:
            fail(self, "Only tabs and windows can be made.")
            return nil
        }
    }
}
