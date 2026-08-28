import BitFunMobileCore

enum PairingFailureCopy {
    static func message(_ failure: PairingFailure, localized: (String) -> String) -> String {
        if let remote = failure.remoteMessage?.trimmingCharacters(in: .whitespacesAndNewlines), !remote.isEmpty {
            return remote
        }
        switch failure.reason.name {
        case "PAIRING_LINK_EMPTY", "PAIRING_LINK_INCOMPLETE", "PAIRING_LINK_UNDECODABLE", "PAIRING_LINK_KEY_UNUSABLE":
            return localized("连接链接无效，请重新扫描或粘贴桌面端链接")
        case "ACCOUNT_USERNAME_REQUIRED":
            return localized("请输入桌面端账号")
        case "ACCOUNT_PASSWORD_REQUIRED":
            return localized("请输入桌面端密码")
        case "REJECTED", "DESKTOP_REJECTED":
            return localized("桌面端拒绝了这次连接")
        case "ROOM_NOT_FOUND":
            return localized("找不到桌面端房间，请确认桌面端仍在等待连接")
        case "RATE_LIMITED", "TOO_MANY_ATTEMPTS":
            return localized("尝试次数过多，请稍后再试")
        case "RELAY_UNAVAILABLE", "NETWORK_UNREACHABLE":
            return localized("网络不可用，请检查手机与桌面端的网络")
        case "TIMEOUT":
            return localized("连接超时，请重新尝试")
        case "PROTOCOL_MISMATCH":
            return localized("桌面端版本不兼容，请升级后重试")
        default:
            return localized("连接失败，请检查桌面端链接")
        }
    }
}
