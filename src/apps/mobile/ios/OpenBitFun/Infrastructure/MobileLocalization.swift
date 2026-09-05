import Foundation

enum MobileLanguage: String, CaseIterable, Identifiable {
    case simplifiedChinese = "zh-Hans"
    case english = "en"

    var id: String { rawValue }

    var nativeName: String {
        switch self {
        case .simplifiedChinese: return "简体中文"
        case .english: return "English"
        }
    }
}

enum MobileLocalization {
    static let preferenceKey = "openbitfun.mobile.language"

    static func restoredLanguage() -> MobileLanguage {
        if let saved = UserDefaults.standard.string(forKey: preferenceKey),
           let language = MobileLanguage(rawValue: saved) {
            return language
        }
        return Locale.preferredLanguages.first?.hasPrefix("zh") == true ? .simplifiedChinese : .english
    }

    static func text(_ key: String, language: MobileLanguage) -> String {
        // The catalog's source language is Simplified Chinese and UI call sites
        // use those source strings as stable keys. Asking Foundation to resolve
        // an untranslated source key with an English development region can
        // still fall through to the English localization, so keep the source
        // language explicit instead of relying on Bundle fallback order.
        if language == .simplifiedChinese { return key }
        return String(localized: String.LocalizationValue(key), locale: Locale(identifier: language.rawValue))
    }

    static func text(_ key: String) -> String {
        text(key, language: restoredLanguage())
    }

    static func format(_ key: String, language: MobileLanguage, _ arguments: CVarArg...) -> String {
        String(
            format: text(key, language: language),
            locale: Locale(identifier: language.rawValue),
            arguments: arguments
        )
    }
}
