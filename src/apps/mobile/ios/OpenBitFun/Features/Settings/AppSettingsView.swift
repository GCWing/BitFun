import SwiftUI

struct SettingsView: View {
    @ObservedObject var model: MobileAppModel
    @Environment(\.dismiss) private var dismiss
    @State private var accountOpen = false

    private var appVersion: String {
        Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? "1.0.0"
    }

    private var selectedModelName: String {
        model.modelOptions.first(where: \.selected)?.primaryLabel
            ?? model.modelOptions.first?.primaryLabel
            ?? model.localized("未配置")
    }

    var body: some View {
        ZStack(alignment: .topTrailing) {
            ScrollView(showsIndicators: false) {
                VStack(alignment: .leading, spacing: 0) {
                    Text(model.localized("设置"))
                        .font(.system(size: 28, weight: .bold))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .padding(.bottom, 30)

                    Button { accountOpen = true } label: {
                        SettingsCard {
                            SettingsProfileRow(
                                subtitle: model.accountUser ?? model.localized("未登录")
                            )
                        }
                    }
                    .buttonStyle(.plain)
                    .padding(.bottom, 24)

                    SettingsGroup(title: "通用") {
                        VStack(spacing: 0) {
                            Button { model.languagePickerOpen = true } label: {
                                SettingsValueRow(
                                    icon: "textformat",
                                    title: "语言",
                                    value: model.appLanguage.nativeName,
                                    showsChevron: true
                                )
                            }
                            .buttonStyle(.plain)
                            Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 26)
                            Button { model.generalConfigOpen = true } label: {
                                SettingsValueRow(
                                    icon: "square.grid.2x2",
                                    title: "模型",
                                    value: selectedModelName,
                                    showsChevron: true
                                )
                            }
                            .buttonStyle(.plain)
                        }
                    }
                    SettingsGroup(title: "关于") {
                        VStack(spacing: 0) {
                            SettingsValueRow(
                                icon: nil,
                                title: "产品",
                                value: "OpenBitFun iOS版"
                            )
                            Divider().overlay(OpenBitFunTheme.line).padding(.horizontal, 26)
                            SettingsValueRow(icon: nil, title: "版本", value: appVersion)
                        }
                    }
                }
                .padding(.horizontal, 16)
                .padding(.top, 64)
                .padding(.bottom, 34)
            }

            Button { dismiss() } label: {
                Image(systemName: "xmark")
                    .font(.system(size: 18, weight: .regular))
                    .foregroundStyle(OpenBitFunTheme.ink)
                    .frame(width: 40, height: 40)
                    .background(OpenBitFunTheme.card)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.localized("关闭"))
            .padding(.top, 22)
            .padding(.trailing, 18)

            if model.languagePickerOpen {
                LanguagePickerSheet(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            } else if model.generalConfigOpen {
                GeneralChatConfigSheet(model: model)
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            } else if accountOpen {
                AccountSettingsView(model: model, onClose: { accountOpen = false })
                    .transition(.move(edge: .trailing).combined(with: .opacity))
            }
        }
        .background(OpenBitFunTheme.page)
        .animation(.easeInOut(duration: 0.2), value: model.languagePickerOpen)
        .animation(.easeInOut(duration: 0.2), value: model.generalConfigOpen)
        .animation(.easeInOut(duration: 0.2), value: accountOpen)
    }
}

private struct LanguagePickerSheet: View {
    @ObservedObject var model: MobileAppModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            OpenBitFunSelectionHeader(title: "选择语言", onClose: { model.languagePickerOpen = false })
            Divider().overlay(OpenBitFunTheme.line)

            VStack(spacing: 0) {
                ForEach(MobileLanguage.allCases) { language in
                    Button {
                        model.setLanguage(language)
                        model.languagePickerOpen = false
                    } label: {
                        HStack {
                            Text(language.nativeName)
                                .font(.system(size: 16, weight: .medium))
                                .foregroundStyle(OpenBitFunTheme.ink)
                            Spacer()
                            if model.appLanguage == language {
                                Image(systemName: "checkmark")
                                    .font(.system(size: 18, weight: .medium))
                                    .foregroundStyle(OpenBitFunTheme.ink)
                            }
                        }
                        .padding(.horizontal, 16)
                        .frame(height: MobileDesignGeometry.selectionRowHeight)
                        .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.top, 8)
            .padding(.bottom, 28)

            Spacer(minLength: 0)
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .top)
        .background(OpenBitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.selectionTopRadius))
    }
}
private struct SettingsGroup<Content: View>: View {
    let title: String
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(MobileLocalization.text(title))
                .font(.system(size: 18, weight: .bold))
                .foregroundStyle(OpenBitFunTheme.muted)
                .padding(.leading, 12)
            SettingsCard(content: content)
        }
        .padding(.bottom, 24)
    }
}

struct SettingsCard<Content: View>: View {
    @ViewBuilder let content: () -> Content

    var body: some View {
        OpenBitFunModalCard(
            radius: MobileDesignGeometry.settingsCompactCardRadius,
            bordered: false,
            content: content
        )
    }
}

private struct SettingsProfileRow: View {
    let subtitle: String

    var body: some View {
        HStack(spacing: 12) {
            Image(systemName: "person.crop.circle")
                .font(.system(size: 24, weight: .regular))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(width: 34, height: 34)
            VStack(alignment: .leading, spacing: 2) {
                Text(MobileLocalization.text("个人资料"))
                    .font(.system(size: 16, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.ink)
                Text(MobileLocalization.text(subtitle))
                    .font(.system(size: 13))
                    .foregroundStyle(OpenBitFunTheme.muted)
                    .lineLimit(1)
            }
            Spacer(minLength: 8)
            Image(systemName: "chevron.right")
                .font(.system(size: 14, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.muted.opacity(0.72))
        }
        .padding(.horizontal, 18)
        .frame(height: 64)
    }
}

private struct SettingsValueRow: View {
    let icon: String?
    let title: String
    let value: String
    var showsChevron: Bool = false

    var body: some View {
        HStack(spacing: 14) {
            if let icon {
                Image(systemName: icon)
                    .font(.system(size: 20, weight: .regular))
                    .foregroundStyle(OpenBitFunTheme.muted)
                    .frame(width: 23, height: 23)
            }
            Text(MobileLocalization.text(title))
                .font(.system(size: 16, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.ink)
            Spacer(minLength: 12)
            Text(MobileLocalization.text(value))
                .font(.system(size: 15))
                .foregroundStyle(OpenBitFunTheme.muted)
                .lineLimit(1)
            if showsChevron {
                Image(systemName: "chevron.right")
                    .font(.system(size: 14, weight: .medium))
                    .foregroundStyle(OpenBitFunTheme.muted.opacity(0.72))
            }
        }
        .padding(.horizontal, 18)
        .frame(height: 52)
    }
}
