import SwiftUI

@main
struct OpenBitFunApp: App {
    @State private var notificationOnboardingOpen = false
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
                    .task {
                        notificationOnboardingOpen = await TaskCompletionNotifier.shouldOfferOnboarding()
                    }
                    .alert(model.localized("开启任务完成提醒"), isPresented: $notificationOnboardingOpen) {
                        Button(model.localized("稍后"), role: .cancel) {
                            TaskCompletionNotifier.finishOnboarding(enable: false)
                        }
                        Button(model.localized("开启通知")) {
                            TaskCompletionNotifier.finishOnboarding(enable: true)
                        }
                    } message: {
                        Text(model.localized("允许 OpenBitFun 在任务完成时发送通知。你可以稍后在系统设置中更改。"))
                    }
                    .onChange(of: scenePhase) { model.handleScenePhase($0) }
                    .environment(\.locale, Locale(identifier: model.appLanguage.rawValue))
            }
        }
    }

}
