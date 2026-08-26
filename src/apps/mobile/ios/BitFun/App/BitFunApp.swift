import SwiftUI

@main
struct BitFunApp: App {
    @StateObject private var model = MobileAppModel.launchConfigured
    @Environment(\.scenePhase) private var scenePhase
    private let designPreviewScenario = Self.resolveDesignPreviewScenario()

    var body: some Scene {
        WindowGroup {
            if let scenario = designPreviewScenario {
                MobileDesignGallery(scenario: scenario)
                    .preferredColorScheme(scenario.appearance == "dark" ? .dark : .light)
            } else {
                MobileShellView(model: model)
                    .onChange(of: scenePhase) { model.handleScenePhase($0) }
                    .environment(\.locale, Locale(identifier: model.appLanguage.rawValue))
            }
        }
    }

    private static func resolveDesignPreviewScenario() -> MobilePreviewScenario? {
        let arguments = ProcessInfo.processInfo.arguments
        guard let marker = arguments.firstIndex(of: "--design-preview") else { return nil }
        let scenarioID = arguments.indices.contains(marker + 1) ? arguments[marker + 1] : "connected-conversation"
        switch scenarioID {
        case MobilePreviewScenarios.streamingDark.id:
            return MobilePreviewScenarios.streamingDark
        case MobilePreviewScenarios.reconnectingWide.id:
            return MobilePreviewScenarios.reconnectingWide
        default:
            return MobilePreviewScenarios.connectedConversation
        }
    }
}
