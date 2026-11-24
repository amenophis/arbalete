// Translating pages on the device (decision 0016, app/src/shell/translate.rs): macOS recognizes a
// page's language and translates its text, and the address offers it, like Arc's.

import AppKit
import NaturalLanguage
import SwiftUI
import Translation

/// What the address shows of the selected tab's page's language (translation_json in
/// app/src/shell/translate.rs).
struct PageTranslation: Decodable, Equatable {
    var language: String
    var translated: Bool
    var always: Bool
}

/// The languages the user reads: System Settings' preferred languages.
private func readLanguages() -> [Locale.Language] {
    Locale.preferredLanguages.map { Locale.Language(identifier: $0) }
}

/// What pages are translated to: the user's first preferred language.
private func targetLanguage() -> Locale.Language {
    readLanguages().first ?? Locale.Language(identifier: "en")
}

/// A language's name, in English like the rest of the interface, like "German".
func languageName(_ code: String) -> String {
    Locale(identifier: "en").localizedString(forIdentifier: code) ?? code
}

/// The language of a page, from a sample of its text, or its `lang` when the text does not
/// tell, like "de"; none when the user reads it or macOS cannot translate pages. The caller
/// frees it.
@_cdecl("arbalete_ui_page_language")
public func pageLanguage(_ lang: UnsafePointer<CChar>, _ sample: UnsafePointer<CChar>)
    -> UnsafeMutablePointer<CChar>?
{
    guard #available(macOS 26, *) else { return nil }
    let recognizer = NLLanguageRecognizer()
    recognizer.processString(String(cString: sample))
    let recognized = recognizer.languageHypotheses(withMaximum: 1).first
        .flatMap { $0.value > 0.8 && $0.key != .undetermined ? $0.key.rawValue : nil }
    let declared = Locale.Language(identifier: String(cString: lang)).languageCode?.identifier
    guard let code = recognized ?? declared,
        let languageCode = Locale.Language(identifier: code).languageCode,
        !readLanguages().contains(where: { $0.languageCode == languageCode })
    else { return nil }
    return strdup(code)
}

/// The sessions translating from each language to the user's.
@available(macOS 26, *)
@MainActor private var sessions: [String: TranslationSession] = [:]
/// The last batch to translate: batches are translated one after the other.
@MainActor private var translating: Task<Void, Never>?

/// Each page's count of stops: a batch asked before the last stop is dropped.
@MainActor private var epochs: [Int32: Int] = [:]

/// Drops the batches of a page's text not translated yet: it shows its original, or went.
@_cdecl("arbalete_ui_translate_stop")
public func translateStop(_ page: Int32) {
    MainActor.assumeIsolated { epochs[page, default: 0] += 1 }
}

/// Translates a batch of a page's text from its language, `source`, to the user's, and gives
/// the translations to arbalete_translated, or null when macOS cannot, like a language not
/// downloaded.
@_cdecl("arbalete_ui_translate")
public func translate(
    _ page: Int32, _ batch: UInt32, _ source: UnsafePointer<CChar>, _ texts: UnsafePointer<CChar>
) {
    let source = String(cString: source)
    let texts = (try? JSONDecoder().decode([String].self, from: Data(String(cString: texts).utf8)))
    MainActor.assumeIsolated {
        guard #available(macOS 26, *), let texts else {
            arbalete_translated(page, batch, nil)
            return
        }
        let previous = translating
        let epoch = epochs[page, default: 0]
        translating = Task { @MainActor in
            await previous?.value
            // The page showed its original or went since.
            guard epochs[page, default: 0] == epoch else { return }
            let json = await translations(texts, from: source)
                .flatMap { try? JSONEncoder().encode($0) }
                .flatMap { String(data: $0, encoding: .utf8) }
            if let json {
                json.withCString { arbalete_translated(page, batch, $0) }
            } else {
                arbalete_translated(page, batch, nil)
            }
        }
    }
}

/// The translations of `texts`, in their order, from `source` to the user's language.
@available(macOS 26, *)
@MainActor private func translations(_ texts: [String], from source: String) async -> [String]? {
    let session =
        sessions[source]
        ?? TranslationSession(
            installedSource: Locale.Language(identifier: source), target: targetLanguage())
    let requests = texts.enumerated().map {
        TranslationSession.Request(sourceText: $1, clientIdentifier: String($0))
    }
    do {
        let responses = try await session.translations(from: requests)
        sessions[source] = session
        var translated = texts
        for response in responses {
            if let index = response.clientIdentifier.flatMap(Int.init), translated.indices.contains(index) {
                translated[index] = response.targetText
            }
        }
        return translated
    } catch {
        NSLog("Arbalete: translating from %@ failed: %@", source, String(describing: error))
        sessions[source] = nil
        return nil
    }
}

/// The address's button on a page in another language, like Arc's: its popover translates the
/// page or shows its original, and remembers the choice for its language or its site.
struct TranslateButton: View {
    @ObservedObject var model: AppModel
    let tab: Tab
    let translation: PageTranslation
    private let _shown = State<Bool>(initialValue: false)
    private var shown: Bool {
        get { _shown.wrappedValue }
        nonmutating set { _shown.wrappedValue = newValue }
    }

    var body: some View {
        IconButton(
            symbol: "translate", label: "Translate page", size: 26,
            active: shown || translation.translated, rest: 1
        ) { shown.toggle() }
        .popover(isPresented: _shown.projectedValue, arrowEdge: .bottom) {
            if #available(macOS 26, *) {
                TranslatePopover(model: model, tab: tab, translation: translation) { shown = false }
            }
        }
    }
}

@available(macOS 26, *)
private struct TranslatePopover: View {
    @ObservedObject var model: AppModel
    let tab: Tab
    let translation: PageTranslation
    let close: () -> Void
    /// Whether macOS translates the page's language to the user's: installed, to download, or
    /// not at all.
    private let _status = State<LanguageAvailability.Status?>(initialValue: nil)
    private var status: LanguageAvailability.Status? {
        get { _status.wrappedValue }
        nonmutating set { _status.wrappedValue = newValue }
    }
    /// Set to download the page's language, through the system's sheet.
    private let _download = State<TranslationSession.Configuration?>(initialValue: nil)

    var body: some View {
        let name = languageName(translation.language)
        let target = languageName(targetLanguage().minimalIdentifier)
        VStack(alignment: .leading, spacing: 10) {
            Text(
                translation.translated
                    ? "Translated from \(name)." : "Translate this page from \(name) to \(target)?"
            )
            .font(.system(size: 13, weight: .semibold))
            .fixedSize(horizontal: false, vertical: true)
            switch status {
            case .unsupported:
                Text("macOS can't translate \(name) to \(target).")
                    .font(.system(size: 12)).foregroundStyle(.secondary)
            case .supported:
                Button("Download \(name)…") {
                    _download.wrappedValue = TranslationSession.Configuration(
                        source: Locale.Language(identifier: translation.language),
                        target: targetLanguage())
                }
                .keyboardShortcut(.defaultAction)
            default:
                if translation.translated {
                    Button("Show Original") {
                        model.run("translate", tab.id, false)
                        close()
                    }
                } else {
                    Button("Translate") {
                        model.run("translate", tab.id, true)
                        close()
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            Divider()
            Toggle(
                "Always translate \(name)",
                isOn: Binding(
                    get: { translation.always },
                    set: { model.run("translate-language", translation.language, $0 ? true as Any : NSNull()) })
            )
            .toggleStyle(.checkbox)
            Button("Never Translate \(name)") {
                model.run("translate", tab.id, false)
                model.run("translate-language", translation.language, false)
                close()
            }
            .buttonStyle(.link)
            Button("Never Translate \(domain(tab.url))") {
                model.run("translate", tab.id, false)
                model.run("never-translate-site", site(tab.url), true)
                close()
            }
            .buttonStyle(.link)
        }
        .font(.system(size: 12))
        .padding(14)
        .frame(width: 270, alignment: .leading)
        .task(id: _download.wrappedValue == nil) { await checkStatus() }
        .translationTask(_download.wrappedValue) { session in
            try? await session.prepareTranslation()
            await checkStatus()
        }
    }

    private func checkStatus() async {
        status = await LanguageAvailability().status(
            from: Locale.Language(identifier: translation.language), to: targetLanguage())
    }
}

/// The site of a page's address, as the app keeps it: its host and port (site in app/src/shell.rs).
private func site(_ url: String) -> String {
    guard let address = URL(string: url), let host = address.host else { return url }
    return address.port.map { "\(host):\($0)" } ?? host
}
