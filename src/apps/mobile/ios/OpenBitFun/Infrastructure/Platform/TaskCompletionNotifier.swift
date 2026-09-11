import OpenBitFunMobileCore
import UIKit
import UserNotifications

@MainActor
final class TaskCompletionNotifier {
    private let policy = TaskCompletionPolicy()
    private var lease: UIBackgroundTaskIdentifier = .invalid
    private var backgrounded = false
    private var expired = false
    private var target: String?

    private static let onboardingKey = "notificationOnboardingCompleted.v1"

    static func shouldOfferOnboarding() async -> Bool {
        guard !UserDefaults.standard.bool(forKey: onboardingKey) else { return false }
        let settings = await UNUserNotificationCenter.current().notificationSettings()
        // Existing grants and refusals survive upgrades without another prompt.
        return settings.authorizationStatus == .notDetermined
    }

    static func finishOnboarding(enable: Bool) {
        UserDefaults.standard.set(true, forKey: onboardingKey)
        guard enable else { return }
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    static func manageNotifications() async {
        let settings = await UNUserNotificationCenter.current().notificationSettings()
        if settings.authorizationStatus == .notDetermined {
            finishOnboarding(enable: true)
        } else if let url = URL(string: UIApplication.openNotificationSettingsURLString) {
            await UIApplication.shared.open(url)
        }
    }

    func observe(_ state: RemoteSessionUiState, target: String) {
        guard !backgrounded || !expired else { return }
        if self.target != target { reset(); self.target = target }
        let completed = policy.observe(state: state, target: target)
        if backgrounded, policy.hasPending(), lease == .invalid { beginLease() }
        if let completed {
            let content = UNMutableNotificationContent()
            content.title = MobileLocalization.text("任务已完成")
            content.body = MobileLocalization.text("打开 OpenBitFun 查看结果。")
            content.sound = .default
            content.userInfo = ["target": completed.target, "sessionId": completed.sessionId]
            let request = UNNotificationRequest(identifier: UUID().uuidString, content: content, trigger: nil)
            UNUserNotificationCenter.current().add(request) { _ in }
        }
        if !policy.hasPending() { endLease() }
    }

    func setBackground(_ value: Bool) {
        backgrounded = value
        policy.setBackground(value: value)
        if value && policy.hasPending() { beginLease() }
        if !value { expired = false; endLease() }
    }

    func reset() { policy.reset(); endLease(); target = nil }

    private func beginLease() {
        guard lease == .invalid else { return }
        lease = UIApplication.shared.beginBackgroundTask(withName: "Observe remote task completion") { [weak self] in
            Task { @MainActor in self?.expired = true; self?.reset() }
        }
        if lease == .invalid { policy.reset() }
    }

    private func endLease() {
        guard lease != .invalid else { return }
        let previous = lease
        lease = .invalid
        UIApplication.shared.endBackgroundTask(previous)
    }
}
