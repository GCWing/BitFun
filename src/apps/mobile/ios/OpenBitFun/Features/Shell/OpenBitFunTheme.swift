import SwiftUI

enum OpenBitFunTheme {
    // Generated from the HarmonyOS baseline through the mobile design contract.
    static let page = MobileDesignColors.pageBg
    static let transparent = MobileDesignColors.transparent
    static let card = MobileDesignColors.card
    static let soft = MobileDesignColors.soft
    static let ink = MobileDesignColors.ink
    static let muted = MobileDesignColors.muted
    static let line = MobileDesignColors.line
    static let accent = MobileDesignColors.accent
    static let contentOnAction = MobileDesignColors.contentOnAction
    static let scrim = MobileDesignColors.scrim
    static let shellScrim = MobileDesignColors.shellScrim
    static let mediaBackground = MobileDesignColors.mediaBackground
    static let mediaScrim = MobileDesignColors.mediaScrim
    static let mediaControlBackground = MobileDesignColors.mediaControlBackground
    static let toastBackground = MobileDesignColors.toastBackground
    static let shadowSubtle = MobileDesignColors.shadowSubtle
    static let shadowMedium = MobileDesignColors.shadowMedium
    static let shadowStrong = MobileDesignColors.shadowStrong
    static let floatingBorder = MobileDesignColors.floatingBorder
    static let statusSuccess = MobileDesignColors.statusSuccess
    static let statusDanger = MobileDesignColors.statusDanger
}

struct CircleControl: View {
    let systemName: String
    var size: CGFloat = MobileDesignGeometry.controlTouchSize
    var glyphSize: CGFloat = 18
    var action: () -> Void

    var body: some View {
        Button(action: action) {
            Image(systemName: systemName)
                .font(.system(size: glyphSize, weight: .medium))
                .foregroundStyle(OpenBitFunTheme.ink)
                .frame(width: size, height: size)
                .background(OpenBitFunTheme.card)
                .overlay(Circle().stroke(OpenBitFunTheme.line, lineWidth: 1))
                .clipShape(Circle())
                .shadow(color: OpenBitFunTheme.shadowMedium, radius: 8, y: 3)
        }
        .buttonStyle(.plain)
        .accessibilityLabel(systemName)
    }
}

struct ReferenceGlyph: View {
    let assetName: String
    let width: CGFloat
    let height: CGFloat

    var body: some View {
        Image(assetName)
            .resizable()
            .renderingMode(.template)
            .foregroundStyle(OpenBitFunTheme.ink)
            .aspectRatio(contentMode: .fit)
            .frame(width: width, height: height)
    }
}

struct ReferenceImage: View {
    let assetName: String
    let width: CGFloat
    let height: CGFloat

    var body: some View {
        Image(assetName)
            .resizable()
            .aspectRatio(contentMode: .fit)
            .frame(width: width, height: height)
    }
}
