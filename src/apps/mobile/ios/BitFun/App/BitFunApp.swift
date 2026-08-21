import SwiftUI

@main
struct BitFunApp: App {
    @StateObject private var model = MobileAppModel.launchConfigured

    var body: some Scene {
        WindowGroup {
            MobileShellView(model: model)
                .preferredColorScheme(.light)
        }
    }
}
