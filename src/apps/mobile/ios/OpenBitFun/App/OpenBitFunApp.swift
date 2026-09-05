import SwiftUI

@main
struct OpenBitFunApp: App {
    @StateObject private var model = MobileLaunchConfiguration.makeModel()
    @Environment(\.scenePhase) private var scenePhase
    private let designPreviewScenario = MobileLaunchConfiguration.designPreviewScenario()

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

}
