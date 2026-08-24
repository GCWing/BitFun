import SwiftUI

enum BitFunTheme {
    // Generated from the HarmonyOS baseline through the mobile design contract.
    static let page = MobileDesignColors.pageBg
    static let card = MobileDesignColors.card
    static let soft = MobileDesignColors.soft
    static let ink = MobileDesignColors.ink
    static let muted = MobileDesignColors.muted
    static let line = MobileDesignColors.line
    static let accent = MobileDesignColors.accent
    static let green = MobileDesignColors.green
    static let red = MobileDesignColors.red
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
                .foregroundStyle(BitFunTheme.ink)
                .frame(width: size, height: size)
                .background(BitFunTheme.card)
                .overlay(Circle().stroke(BitFunTheme.line, lineWidth: 1))
                .clipShape(Circle())
                .shadow(color: .black.opacity(0.07), radius: 8, y: 3)
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
            .foregroundStyle(BitFunTheme.ink)
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
