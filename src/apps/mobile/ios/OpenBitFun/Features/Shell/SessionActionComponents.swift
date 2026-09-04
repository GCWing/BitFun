import SwiftUI

enum SessionActionPresentation {
    case bottomSheet
    case popover
}

/// One action contract for sidebar and workspace session rows. Compact lists
/// present it as a sheet; permanent lists present the same rows as a popover.
/// High-impact deletion replaces those rows in place instead of stacking an
/// alert and a second scrim over the action surface.
struct SessionActionSurface: View {
    @ObservedObject var model: MobileAppModel
    let session: ChatSession
    let presentation: SessionActionPresentation
    var canViewDetails = true
    var canArchive = false
    var canExport = false
    var canDelete = true
    let onViewDetails: () -> Void
    let onArchive: () -> Void
    let onExport: () -> Void
    let onDelete: () -> Void
    let onClose: () -> Void
    @State private var confirmingDelete = false

    var body: some View {
        VStack(spacing: 0) {
            if presentation == .bottomSheet {
                Capsule()
                    .fill(OpenBitFunTheme.line)
                    .frame(width: 36, height: 4)
                    .padding(.bottom, 10)
            }

            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.localized("会话操作"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.muted)
                    Text(session.title.isEmpty ? model.localized("未命名会话") : session.title)
                        .font(.system(size: 15, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .lineLimit(1)
                }
                Spacer(minLength: 8)
                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: 16, weight: .regular))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .frame(width: 40, height: 40)
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.localized("关闭"))
            }
            .frame(height: 52)

            Divider().overlay(OpenBitFunTheme.line).padding(.top, 6).padding(.bottom, 8)

            if confirmingDelete {
                deleteConfirmation
            } else {
                actionRows
            }
        }
        .padding(.horizontal, 16)
        .padding(.top, 10)
        .padding(.bottom, 18)
        .frame(width: presentation == .popover ? 300 : nil)
        .frame(maxWidth: presentation == .bottomSheet ? .infinity : nil)
        .background(OpenBitFunTheme.card)
        .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.popoverRadius))
        .overlay(
            RoundedRectangle(cornerRadius: MobileDesignGeometry.popoverRadius)
                .stroke(OpenBitFunTheme.line, lineWidth: 1)
        )
        .shadow(
            color: presentation == .popover ? OpenBitFunTheme.line : OpenBitFunTheme.transparent,
            radius: presentation == .popover ? 20 : 0,
            y: presentation == .popover ? 8 : 0
        )
        .accessibilityAction(.escape, onClose)
    }

    @ViewBuilder
    private var actionRows: some View {
        if canViewDetails {
            actionRow("查看详情", icon: "info.circle") {
                onViewDetails()
                onClose()
            }
        }
        if canArchive {
            actionRow(
                session.status.lowercased() == "archived" ? "取消归档" : "归档会话",
                icon: "archivebox"
            ) {
                onArchive()
                onClose()
            }
        }
        if canExport {
            actionRow("导出会话", icon: "cloud") {
                onExport()
                onClose()
            }
        }
        if canDelete {
            if canArchive || canExport {
                Divider().overlay(OpenBitFunTheme.line).padding(.vertical, 6)
            }
            actionRow("删除", icon: "trash", destructive: true) {
                confirmingDelete = true
            }
        }
    }

    private var deleteConfirmation: some View {
        VStack(spacing: 12) {
            Text(model.localized("删除后无法恢复此会话，是否继续？"))
                .font(.system(size: 13))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(maxWidth: .infinity, alignment: .leading)
            HStack(spacing: 10) {
                confirmationButton(
                    "取消",
                    fill: OpenBitFunTheme.soft,
                    foreground: OpenBitFunTheme.ink,
                    emphasized: false
                ) {
                    confirmingDelete = false
                }
                confirmationButton(
                    "删除",
                    fill: OpenBitFunTheme.statusDanger,
                    foreground: OpenBitFunTheme.contentOnAction,
                    emphasized: true
                ) {
                    onDelete()
                    onClose()
                }
            }
        }
    }

    private func actionRow(
        _ title: String,
        icon: String,
        destructive: Bool = false,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            HStack(spacing: 12) {
                Image(systemName: icon)
                    .font(.system(size: 19, weight: .regular))
                    .foregroundStyle(destructive ? OpenBitFunTheme.statusDanger : OpenBitFunTheme.muted)
                    .frame(width: 23, height: 23)
                Text(model.localized(title))
                    .font(.system(size: 15))
                    .foregroundStyle(destructive ? OpenBitFunTheme.statusDanger : OpenBitFunTheme.ink)
                Spacer(minLength: 0)
            }
            .padding(.horizontal, 10)
            .frame(height: MobileDesignGeometry.sheetActionHeight)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private func confirmationButton(
        _ title: String,
        fill: Color,
        foreground: Color,
        emphasized: Bool,
        action: @escaping () -> Void
    ) -> some View {
        Button(action: action) {
            Text(model.localized(title))
                .font(.system(size: 14, weight: emphasized ? .medium : .regular))
                .foregroundStyle(foreground)
                .frame(maxWidth: .infinity, minHeight: 44)
                .background(fill)
                .clipShape(RoundedRectangle(cornerRadius: 8))
        }
        .buttonStyle(.plain)
    }
}

/// Read-only details use the same field order and paper geometry as Harmony's
/// `SessionDetailsView`; paths stay selectable on the controller device.
struct SessionDetailsView: View {
    @ObservedObject var model: MobileAppModel
    let session: ChatSession
    let onClose: () -> Void

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: 12) {
                VStack(alignment: .leading, spacing: 3) {
                    Text(model.localized("会话详情"))
                        .font(.system(size: 13, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.muted)
                    Text(session.title.isEmpty ? model.localized("未命名会话") : session.title)
                        .font(.system(size: 18, weight: .medium))
                        .foregroundStyle(OpenBitFunTheme.ink)
                        .lineLimit(2)
                }
                Spacer(minLength: 8)
                Button(action: onClose) {
                    Image(systemName: "xmark")
                        .font(.system(size: 17))
                        .foregroundStyle(OpenBitFunTheme.muted)
                        .frame(width: 44, height: 44)
                        .background(OpenBitFunTheme.soft)
                        .clipShape(Circle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(model.localized("关闭"))
            }
            .padding(.leading, 20)
            .padding(.trailing, 16)
            .padding(.top, 18)
            .padding(.bottom, 16)

            Divider().overlay(OpenBitFunTheme.line)

            ScrollView(showsIndicators: false) {
                VStack(spacing: 0) {
                    detailRow("Agent 类型", value: session.agentType.isEmpty ? model.localized("未知") : session.agentType)
                    if let workspaceName = session.workspaceName, !workspaceName.isEmpty {
                        detailRow("工作区", value: workspaceName)
                    }
                    if let path = session.workspacePath, !path.isEmpty {
                        pathRow(path)
                    }
                    if !session.createdAt.isEmpty { detailRow("创建时间", value: session.createdAt) }
                    if !session.updatedLabel.isEmpty { detailRow("更新时间", value: session.updatedLabel) }
                    detailRow("消息数量", value: String(max(0, session.messageCount)))
                    if !session.status.isEmpty {
                        detailRow("状态", value: statusLabel)
                    }
                }
                .padding(.horizontal, 20)
                .padding(.top, 8)
                .padding(.bottom, 24)
            }
        }
        .frame(maxWidth: .infinity, maxHeight: .infinity)
        .background(OpenBitFunTheme.page)
    }

    private var statusLabel: String {
        switch session.status.lowercased() {
        case "archived": return model.localized("已归档")
        case "active": return model.localized("执行中")
        default: return session.status
        }
    }

    private func detailRow(_ label: String, value: String) -> some View {
        HStack(spacing: 16) {
            Text(model.localized(label))
                .font(.system(size: 13))
                .foregroundStyle(OpenBitFunTheme.muted)
                .frame(width: 104, alignment: .leading)
            Text(value)
                .font(.system(size: 15))
                .foregroundStyle(OpenBitFunTheme.ink)
                .multilineTextAlignment(.trailing)
                .frame(maxWidth: .infinity, alignment: .trailing)
                .lineLimit(2)
        }
        .padding(.vertical, 8)
        .frame(minHeight: 52)
        .overlay(alignment: .bottom) { Divider().overlay(OpenBitFunTheme.line) }
    }

    private func pathRow(_ path: String) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(model.localized("工作区路径"))
                .font(.system(size: 13))
                .foregroundStyle(OpenBitFunTheme.muted)
            Text(path)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(OpenBitFunTheme.ink)
                .padding(.horizontal, 10)
                .padding(.vertical, 8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(OpenBitFunTheme.card)
                .overlay(RoundedRectangle(cornerRadius: 6).stroke(OpenBitFunTheme.line, lineWidth: 1))
                .clipShape(RoundedRectangle(cornerRadius: 6))
                .textSelection(.enabled)
        }
        .padding(.vertical, 12)
        .overlay(alignment: .bottom) { Divider().overlay(OpenBitFunTheme.line) }
    }
}
