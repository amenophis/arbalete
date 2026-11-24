// The sidebar's media players, like Arc's: above the footer, a card for each tab left while its
// media plays or while it holds a call (app/src/shell/media.rs).
//
// Views keep their own state in State values spelled out, without the @State macro, which only
// Xcode can expand, not the Command Line Tools.

import SwiftUI

/// A tab left playing media or holding a call (UiState::players in app/src/shell/media.rs).
struct Player: Decodable, Identifiable {
    var tab: Int
    /// What plays, or the tab's title, and who plays it, or the tab's site.
    var title: String
    var subtitle: String
    var artwork: String?
    var favicon: String?
    var playing: Bool
    var muted: Bool
    /// Whether its page offers to go to its previous and next tracks.
    var previous: Bool
    var next: Bool
    /// Its call, while it holds one.
    var call: Call?

    var id: Int { tab }
}

/// What a call lets the player toggle: its microphone and its camera, whether on, unless the page
/// offers no toggle, and leaving.
struct Call: Decodable {
    var microphone: Bool?
    var camera: Bool?
    var hangup: Bool
}

/// The players, the first tab opened first.
struct MediaPlayers: View {
    @ObservedObject var model: AppModel

    var body: some View {
        let players = model.state?.players ?? []
        VStack(spacing: 4) {
            ForEach(players) { player in
                PlayerCard(model: model, player: player)
                    .transition(.opacity)
            }
        }
        .padding(.bottom, players.isEmpty ? 0 : 4)
        .animation(.easeInOut(duration: 0.2), value: model.state?.players?.map(\.tab))
    }
}

/// A player, on two lines beside its artwork or the tab's icon: what plays, which with the artwork
/// takes back to the tab, and its buttons.
private struct PlayerCard: View {
    @ObservedObject var model: AppModel
    let player: Player
    private let _hovered = State<Bool>(initialValue: false)
    private var hovered: Bool {
        get { _hovered.wrappedValue }
        nonmutating set { _hovered.wrappedValue = newValue }
    }

    var body: some View {
        HStack(spacing: 8) {
            Button { model.run("select", player.tab) } label: { artwork }
                .buttonStyle(.plain)
                .help("Go to Tab")
            VStack(alignment: .leading, spacing: 2) {
                Button { model.run("select", player.tab) } label: {
                    (Text(player.title).font(.system(size: 12, weight: .semibold)).foregroundStyle(Ink.primary)
                        + Text("  " + player.subtitle).font(.system(size: 11)).foregroundStyle(Ink.secondary))
                        .lineLimit(1)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .help("Go to Tab")
                // The buttons' icons line up with the text.
                HStack(spacing: 0) { buttons }.padding(.leading, -6)
            }
        }
        .padding(6)
        .background(RoundedRectangle(cornerRadius: Measure.radius).fill(hovered ? Ink.itemHovered : Ink.item))
        .onHover { hovered = $0 }
    }

    private var artwork: some View {
        Group {
            if let artwork = player.artwork.flatMap(URL.init(string:)) {
                AsyncImage(url: artwork) { image in
                    image.resizable().scaledToFill()
                } placeholder: {
                    Favicon(url: player.favicon, size: 16)
                }
            } else {
                Favicon(url: player.favicon, size: 16)
            }
        }
        .frame(width: 44, height: 44)
        .clipShape(RoundedRectangle(cornerRadius: 6))
    }

    @ViewBuilder private var buttons: some View {
        if let call = player.call {
            if let on = call.microphone {
                PlayerButton(symbol: on ? "mic.fill" : "mic.slash.fill", help: on ? "Mute Microphone" : "Unmute Microphone") {
                    model.run("media", player.tab, "toggle-microphone")
                }
            }
            if let on = call.camera {
                PlayerButton(symbol: on ? "video.fill" : "video.slash.fill", help: on ? "Turn Camera Off" : "Turn Camera On") {
                    model.run("media", player.tab, "toggle-camera")
                }
            }
            if call.hangup {
                PlayerButton(symbol: "phone.down.fill", help: "Leave Call", tint: .red) {
                    model.run("media", player.tab, "hang-up")
                }
            }
        } else {
            if player.previous {
                PlayerButton(symbol: "backward.end.fill", help: "Previous Track") {
                    model.run("media", player.tab, "previous-track")
                }
            }
            PlayerButton(symbol: player.playing ? "pause.fill" : "play.fill", help: player.playing ? "Pause" : "Play") {
                model.run("media", player.tab, player.playing ? "pause" : "play")
            }
            if player.next {
                PlayerButton(symbol: "forward.end.fill", help: "Next Track") {
                    model.run("media", player.tab, "next-track")
                }
            }
            PlayerButton(symbol: player.muted ? "speaker.slash.fill" : "speaker.wave.2.fill", help: player.muted ? "Unmute Tab" : "Mute Tab") {
                model.run("toggle-mute", player.tab)
            }
            if !player.playing {
                PlayerButton(symbol: "xmark", help: "Dismiss") { model.run("media", player.tab, "dismiss") }
            }
        }
    }
}

/// A player's button, tinted while hovered.
private struct PlayerButton: View {
    let symbol: String
    let help: String
    var tint: Color? = nil
    let action: () -> Void
    private let _hovered = State<Bool>(initialValue: false)

    var body: some View {
        Button(action: action) {
            Image(systemName: symbol)
                .font(.system(size: 11, weight: .semibold))
                .foregroundStyle(tint.map(AnyShapeStyle.init) ?? AnyShapeStyle(Ink.primary))
                .frame(width: 24, height: 24)
                .background(RoundedRectangle(cornerRadius: 6).fill(_hovered.wrappedValue ? Ink.itemHovered : .clear))
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { _hovered.wrappedValue = $0 }
        .help(help)
    }
}
