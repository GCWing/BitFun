import SwiftUI
import BitFunMobileCore

/// Shared chrome for every BitFun modal page. The presentation primitive stays
/// native; this view owns the paper-and-ink header geometry inside it.
struct BitFunModalHeader: View {
    let title: String
    var subtitle: String? = nil
    let onClose: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                Text(MobileLocalization.text(title))
                    .font(MobileDesignTypography.headlineSmall.font)
                    .foregroundStyle(BitFunTheme.ink)
                    .lineLimit(1)
                if let subtitle, !subtitle.isEmpty {
                    Text(MobileLocalization.text(subtitle))
                        .font(MobileDesignTypography.labelSmall.font)
                        .foregroundStyle(BitFunTheme.muted)
                        .lineLimit(2)
                }
            }
            Spacer(minLength: 8)
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 15, weight: .medium))
                    .foregroundStyle(BitFunTheme.ink)
                    .frame(width: 40, height: 40)
                    .background(BitFunTheme.soft)
                    .clipShape(Circle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(MobileLocalization.text("关闭"))
        }
        .frame(minHeight: MobileDesignGeometry.sheetHeaderHeight)
    }
}

/// Header used when a picker or provider page replaces the current modal
/// content. Harmony's selection panels use a quiet 32-point dismissal target
/// on a 56-point row rather than another filled circular control.
struct BitFunSelectionHeader: View {
    let title: String
    let onClose: () -> Void

    var body: some View {
        HStack(spacing: 12) {
            Text(MobileLocalization.text(title))
                .font(MobileDesignTypography.headlineSmall.font)
                .foregroundStyle(BitFunTheme.ink)
                .lineLimit(1)
            Spacer(minLength: 8)
            Button(action: onClose) {
                Image(systemName: "xmark")
                    .font(.system(size: 18, weight: .regular))
                    .foregroundStyle(BitFunTheme.muted)
                    .frame(
                        width: MobileDesignGeometry.selectionCloseSize,
                        height: MobileDesignGeometry.selectionCloseSize
                    )
            }
            .buttonStyle(.plain)
            .accessibilityLabel(MobileLocalization.text("关闭"))
        }
        .padding(.horizontal, 16)
        .frame(height: MobileDesignGeometry.sheetHeaderHeight)
    }
}

struct BitFunModalCard<Content: View>: View {
    var radius: CGFloat = MobileDesignGeometry.settingsCardRadius
    var bordered: Bool = true
    @ViewBuilder let content: () -> Content

    var body: some View {
        VStack(spacing: 0, content: content)
            .background(BitFunTheme.card)
            .clipShape(RoundedRectangle(cornerRadius: radius))
            .overlay(
                RoundedRectangle(cornerRadius: radius)
                    .stroke(bordered ? BitFunTheme.line : BitFunTheme.transparent, lineWidth: 1)
            )
    }
}

/// One signed-out connection decision, reused wherever the user can enter the
/// remote product. Keeping the order and treatment here prevents the sidebar
/// and pairing sheet from drifting into two different connection flows.
struct SignedOutConnectionActions: View {
    let scanTitle: String
    let accountTitle: String
    let onScan: () -> Void
    let onOpenAccount: () -> Void
    var showScan = true
    var enabled = true
    var buttonHeight: CGFloat = 48
    var spacing: CGFloat = 10
    var fontSize: CGFloat = 16

    var body: some View {
        VStack(spacing: spacing) {
            if showScan {
                Button(action: onScan) {
                    Text(scanTitle)
                        .font(.system(size: fontSize, weight: .bold))
                        .foregroundStyle(BitFunTheme.ink)
                        .frame(maxWidth: .infinity, minHeight: buttonHeight)
                        .background(BitFunTheme.card)
                        .overlay(Capsule().stroke(BitFunTheme.line, lineWidth: 1))
                        .clipShape(Capsule())
                }
                .buttonStyle(.plain)
                .disabled(!enabled)
            }
            Button(action: onOpenAccount) {
                Text(accountTitle)
                    .font(.system(size: fontSize, weight: .bold))
                    .foregroundStyle(BitFunTheme.contentOnAction)
                    .frame(maxWidth: .infinity, minHeight: buttonHeight)
                    .background(BitFunTheme.accent)
                    .clipShape(Capsule())
            }
            .buttonStyle(.plain)
            .disabled(!enabled)
        }
    }
}

struct BitFunPopoverSurfaceModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(.horizontal, MobileDesignGeometry.popoverPadding)
            .padding(.vertical, MobileDesignGeometry.popoverVerticalPadding)
            .frame(width: MobileDesignGeometry.popoverWidth)
            .background(MobileDesignColors.floatingPanelBg)
            .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.popoverRadius))
            .overlay(
                RoundedRectangle(cornerRadius: MobileDesignGeometry.popoverRadius)
                    .stroke(BitFunTheme.line, lineWidth: 1)
            )
            .shadow(
                color: BitFunTheme.line,
                radius: MobileDesignGeometry.popoverShadowRadius,
                y: 7
            )
    }
}

struct BitFunCompactPopoverSurfaceModifier: ViewModifier {
    func body(content: Content) -> some View {
        content
            .padding(.vertical, 8)
            .frame(width: MobileDesignGeometry.compactPopoverWidth)
            .background(MobileDesignColors.floatingPanelBg)
            .clipShape(RoundedRectangle(cornerRadius: MobileDesignGeometry.compactPopoverRadius))
            .overlay(
                RoundedRectangle(cornerRadius: MobileDesignGeometry.compactPopoverRadius)
                    .stroke(BitFunTheme.line, lineWidth: 1)
            )
            .shadow(color: BitFunTheme.line, radius: MobileDesignGeometry.popoverShadowRadius, y: 7)
    }
}

extension View {
    func bitFunPopoverSurface() -> some View {
        modifier(BitFunPopoverSurfaceModifier())
    }

    func bitFunCompactPopoverSurface() -> some View {
        modifier(BitFunCompactPopoverSurfaceModifier())
    }

    func bitFunAdaptiveModal<ModalContent: View>(
        isPresented: Binding<Bool>,
        placement: SettingsPlacement,
        onDismiss: (() -> Void)? = nil,
        @ViewBuilder content: @escaping () -> ModalContent
    ) -> some View {
        modifier(
            BitFunAdaptiveModalModifier(
                isPresented: isPresented,
                placement: placement,
                onDismiss: onDismiss,
                modalContent: content
            )
        )
    }
}

/// Selects the native presentation lifecycle from the KMP placement decision.
/// Compact devices keep a system sheet; side placements use a native full-screen
/// cover containing a trailing paper surface so focus and VoiceOver are isolated
/// from the covered conversation while the dimensions remain Harmony-compatible.
private struct BitFunAdaptiveModalModifier<ModalContent: View>: ViewModifier {
    @Binding var isPresented: Bool
    let placement: SettingsPlacement
    let onDismiss: (() -> Void)?
    @ViewBuilder let modalContent: () -> ModalContent

    private var isSide: Bool { placement.mode == .side }

    private var compactPresented: Binding<Bool> {
        Binding(
            get: { isPresented && !isSide },
            set: { if !$0 { isPresented = false } }
        )
    }

    private var sidePresented: Binding<Bool> {
        Binding(
            get: { isPresented && isSide },
            set: { if !$0 { isPresented = false } }
        )
    }

    func body(content base: Content) -> some View {
        base
            .sheet(isPresented: compactPresented, onDismiss: onDismiss) {
                compactSheet
            }
            .fullScreenCover(isPresented: sidePresented, onDismiss: onDismiss) {
                sideCover
            }
    }

    @ViewBuilder
    private var compactSheet: some View {
        let surface = modalContent()
            .presentationDetents([.large])
            .presentationDragIndicator(.hidden)
        if #available(iOS 16.4, *) {
            surface.presentationCornerRadius(MobileDesignGeometry.sheetTopRadius)
        } else {
            surface
        }
    }

    @ViewBuilder
    private var sideCover: some View {
        let cover = ZStack(alignment: .trailing) {
            BitFunTheme.scrim
                .ignoresSafeArea()
                .contentShape(Rectangle())
                .onTapGesture { isPresented = false }

            modalContent()
                .frame(
                    width: CGFloat(placement.width),
                    height: CGFloat(placement.height)
                )
                .background(BitFunTheme.page)
                .clipShape(
                    RoundedRectangle(
                        cornerRadius: MobileDesignGeometry.sheetSideRadius,
                        style: .continuous
                    )
                )
                .overlay(
                    RoundedRectangle(
                        cornerRadius: MobileDesignGeometry.sheetSideRadius,
                        style: .continuous
                    )
                    .stroke(BitFunTheme.line, lineWidth: 1)
                )
                .shadow(color: BitFunTheme.shadowStrong, radius: 18, x: -5, y: 8)
                .accessibilityAddTraits(.isModal)
        }
        if #available(iOS 16.4, *) {
            cover.presentationBackground(BitFunTheme.transparent)
        } else {
            cover
        }
    }
}
