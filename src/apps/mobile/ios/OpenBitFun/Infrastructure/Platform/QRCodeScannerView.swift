import AVFoundation
import SwiftUI
import UIKit

struct QRCodeScannerView: UIViewControllerRepresentable {
    let onCode: (String) -> Void
    let onCancel: () -> Void

    func makeUIViewController(context: Context) -> QRScannerController {
        let controller = QRScannerController()
        controller.onCode = onCode
        controller.onCancel = onCancel
        return controller
    }

    func updateUIViewController(_ uiViewController: QRScannerController, context: Context) {}
}

final class QRScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    private let sessionQueue = DispatchQueue(label: "com.openbitfun.mobile.ios.qr-session")
    private lazy var session = AVCaptureSession()
    private var previewLayer: AVCaptureVideoPreviewLayer?
    private var captureConfigured = false
    private var emittedCode = false
    var onCode: ((String) -> Void)?
    var onCancel: (() -> Void)?

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = UIColor(OpenBitFunTheme.mediaBackground)
        let close = UIButton(type: .system)
        close.setImage(UIImage(systemName: "xmark"), for: .normal)
        close.tintColor = UIColor(OpenBitFunTheme.contentOnAction)
        close.backgroundColor = UIColor(OpenBitFunTheme.mediaControlBackground)
        close.layer.cornerRadius = 22
        close.addAction(UIAction { [weak self] _ in
            self?.stopCapture()
            self?.onCancel?()
        }, for: .touchUpInside)
        close.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(close)
        NSLayoutConstraint.activate([
            close.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 16),
            close.trailingAnchor.constraint(equalTo: view.trailingAnchor, constant: -20),
            close.widthAnchor.constraint(equalToConstant: 44),
            close.heightAnchor.constraint(equalToConstant: 44),
        ])

        switch AVCaptureDevice.authorizationStatus(for: .video) {
        case .authorized:
            configureCaptureAsync()
        case .notDetermined:
            AVCaptureDevice.requestAccess(for: .video) { [weak self] granted in
                guard granted else { return }
                self?.configureCaptureAsync()
            }
        default:
            break
        }
    }

    override func viewDidDisappear(_ animated: Bool) {
        super.viewDidDisappear(animated)
        stopCapture()
    }

    override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        previewLayer?.frame = view.bounds
    }

    private func configureCaptureAsync() {
        sessionQueue.async { [weak self] in
            self?.configureCapture()
        }
    }

    private func configureCapture() {
        guard !captureConfigured else { return }
        guard let device = AVCaptureDevice.default(for: .video),
              let input = try? AVCaptureDeviceInput(device: device),
              session.canAddInput(input) else { return }
        let output = AVCaptureMetadataOutput()
        guard session.canAddOutput(output) else { return }
        session.beginConfiguration()
        session.addInput(input)
        session.addOutput(output)
        output.setMetadataObjectsDelegate(self, queue: sessionQueue)
        output.metadataObjectTypes = [.qr]
        session.commitConfiguration()
        captureConfigured = true
        DispatchQueue.main.async { [weak self] in
            guard let self, self.previewLayer == nil else { return }
            let layer = AVCaptureVideoPreviewLayer(session: self.session)
            layer.videoGravity = .resizeAspectFill
            layer.frame = self.view.bounds
            self.view.layer.insertSublayer(layer, at: 0)
            self.previewLayer = layer
        }
        session.startRunning()
    }

    private func stopCapture() {
        sessionQueue.async { [weak self] in
            guard let self, self.session.isRunning else { return }
            self.session.stopRunning()
        }
    }

    func metadataOutput(
        _ output: AVCaptureMetadataOutput,
        didOutput metadataObjects: [AVMetadataObject],
        from connection: AVCaptureConnection,
    ) {
        guard let value = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue,
              !value.isEmpty,
              !emittedCode else { return }
        emittedCode = true
        if session.isRunning {
            session.stopRunning()
        }
        DispatchQueue.main.async { [weak self] in
            self?.onCode?(value)
        }
    }
}
