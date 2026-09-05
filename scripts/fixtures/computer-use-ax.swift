import AppKit

// An isolated, deterministic AX source. It never reads or drives another app.
let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let window = NSWindow(contentRect: NSRect(x: 100, y: 100, width: 500, height: 260),
                      styleMask: [.titled, .closable], backing: .buffered, defer: false)
window.title = "OpenBitFun AX context fixture"
let container = window.contentView!
for (index, title) in ["Save report", "Delete draft", "Unavailable action"].enumerated() {
    let button = NSButton(title: title, target: nil, action: nil)
    button.frame = NSRect(x: 20 + index * 155, y: 170, width: 150, height: 40)
    button.isEnabled = index != 2
    container.addSubview(button)
}
let field = NSTextField(string: "context-value")
field.frame = NSRect(x: 20, y: 90, width: 350, height: 32)
container.addSubview(field)
window.orderFront(nil)
DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    FileHandle.standardOutput.write(Data("READY\n".utf8))
}
app.run()
