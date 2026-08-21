import SwiftUI

enum BitFunTheme {
    // These values mirror harmonyos/entry/src/main/resources/base/element/color.json.
    static let page = Color(red: 253 / 255, green: 253 / 255, blue: 251 / 255)
    static let card = Color.white
    static let soft = Color(red: 244 / 255, green: 243 / 255, blue: 240 / 255)
    static let ink = Color(red: 23 / 255, green: 23 / 255, blue: 23 / 255)
    static let muted = Color(red: 112 / 255, green: 111 / 255, blue: 106 / 255)
    static let line = Color(red: 233 / 255, green: 231 / 255, blue: 226 / 255)
    static let accent = Color(red: 17 / 255, green: 17 / 255, blue: 17 / 255)
    static let green = Color(red: 39 / 255, green: 196 / 255, blue: 106 / 255)
    static let red = Color(red: 224 / 255, green: 79 / 255, blue: 79 / 255)
}

struct CircleControl: View {
    let systemName: String
    var size: CGFloat = 44
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
