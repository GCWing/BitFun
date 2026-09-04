import AVFoundation
import SwiftUI
import UIKit

struct QRCodeScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void

    func makeUIViewController(context: Context) -> QRScannerController {
        let controller = QRScannerController()
        controller.onCode = onCode
        return controller
    }

    func updateUIViewController(_ uiViewController: QRScannerController, context: Context) {}
}

final class QRScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let session = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?
    var onCode: ((String) -> Void)?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = UIColor(OpenBitFunTheme.mediaBackground)
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.tintColor = UIColor(OpenBitFunTheme.contentOnAction)
        close.backgroundColor = UIColor(OpenBitFunTheme.mediaControlBackground)
        close.layer.cornerRadius = 22
        close.addAction(UIAction { [weak self] _ in self?.dismiss(animated: true) }, for: .touchUpInside)
        close.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(close)
        NSLayoutConstraint.activate([
            close.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            close.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -20),
            close.widthAnchor.constraint(equalToConstant: 44),
            close.heightAnchor.constraint(equalToConstant: 44),
        ])

        guard AVCaptureDevice.authorizationStatus(for: .video) != .denied else { return }
        AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
            guard granted else { return }
            DispatchQueue.main.async { self?.configureCapture() }
        }
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    private func configureCapture() {
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else { return }
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.addInput(input)
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: .main)
        output.metadataObjectTypes = [.qr]
        let layer = AVCaptureVideoPreviewLayer(session: session)
        layer.videoGravity = .resizeAspectFill
        view.layer.insertSublayer(layer, at: 0)
        previewLayer = layer
        session.startRunning()
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection,
    ) {
        guard let value = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue,
              !value.isEmpty else { return }
        session.stopRunning()
        onCode?(value)
        dismiss(animated: true)
    }
}
