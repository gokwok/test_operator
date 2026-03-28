import AppKit
import Foundation
import QuartzCore

struct EffectPoint: Decodable {
    let x: Double
    let y: Double
}

struct EffectRequest: Decodable {
    let kind: String
    let point: EffectPoint?
    let from: EffectPoint?
    let to: EffectPoint?
    let mode: String?
    let dx: Double?
    let dy: Double?
    let label: String?
}

final class OverlayView: NSView {
    private let request: EffectRequest
    private let unionFrame: CGRect

    init(frame: CGRect, unionFrame: CGRect, request: EffectRequest) {
        self.request = request
        self.unionFrame = unionFrame
        super.init(frame: frame)
        wantsLayer = true
        let rootLayer = CALayer()
        rootLayer.frame = CGRect(origin: .zero, size: frame.size)
        rootLayer.backgroundColor = NSColor.clear.cgColor
        layer = rootLayer
        buildLayers()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    override var isOpaque: Bool {
        false
    }

    private func buildLayers() {
        guard let rootLayer = layer else {
            return
        }

        switch request.kind {
        case "click":
            guard let point = request.point else {
                return
            }
            clickLayers(at: localPoint(from: point), mode: request.mode ?? "left")
                .forEach(rootLayer.addSublayer)
        case "move":
            guard let point = request.point else {
                return
            }
            moveLayers(at: localPoint(from: point)).forEach(rootLayer.addSublayer)
        case "drag":
            guard let from = request.from, let to = request.to else {
                return
            }
            dragLayers(from: localPoint(from: from), to: localPoint(from: to))
                .forEach(rootLayer.addSublayer)
        case "scroll":
            guard let point = request.point else {
                return
            }
            scrollLayers(
                at: localPoint(from: point),
                dx: request.dx ?? 0,
                dy: request.dy ?? 0
            )
            .forEach(rootLayer.addSublayer)
        case "keyboard":
            guard let label = request.label, !label.isEmpty else {
                return
            }
            keyboardLayers(label: label).forEach(rootLayer.addSublayer)
        default:
            return
        }
    }

    private func localPoint(from point: EffectPoint) -> CGPoint {
        let bottomLeft = CGPoint(x: point.x, y: unionFrame.maxY - point.y)
        return CGPoint(x: bottomLeft.x - unionFrame.minX, y: bottomLeft.y - unionFrame.minY)
    }

    private func clickLayers(at point: CGPoint, mode: String) -> [CALayer] {
        let color = clickColor(mode)
        var layers = [
            circleLayer(center: point, radius: 22, fillColor: color.withAlphaComponent(0.16)),
            circleLayer(
                center: point,
                radius: 18,
                strokeColor: color.withAlphaComponent(0.98),
                lineWidth: 4
            ),
            circleLayer(
                center: point,
                radius: 34,
                strokeColor: color.withAlphaComponent(0.55),
                lineWidth: 3
            ),
            circleLayer(center: point, radius: 6, fillColor: color)
        ]

        if mode == "double" {
            layers.append(
                circleLayer(
                    center: point,
                    radius: 48,
                    strokeColor: color.withAlphaComponent(0.42),
                    lineWidth: 2,
                    dashPattern: [8, 6]
                )
            )
        }

        return layers
    }

    private func moveLayers(at point: CGPoint) -> [CALayer] {
        let trailColor = NSColor(
            calibratedRed: 0.12,
            green: 0.92,
            blue: 1.0,
            alpha: 0.98
        )
        let start = CGPoint(x: point.x - 46, y: point.y + 24)
        var layers = clickLayers(at: point, mode: "left")
        layers.insert(contentsOf: [
            lineLayer(
                from: start,
                to: point,
                lineWidth: 14,
                color: NSColor.white.withAlphaComponent(0.16)
            ),
            lineLayer(
                from: start,
                to: point,
                lineWidth: 10,
                color: trailColor.withAlphaComponent(0.60)
            ),
            arrowHeadLayer(from: start, to: point, color: trailColor)
        ], at: 0)
        return layers
    }

    private func dragLayers(from: CGPoint, to: CGPoint) -> [CALayer] {
        let lineColor = NSColor(
            calibratedRed: 0.18,
            green: 0.85,
            blue: 0.58,
            alpha: 0.9
        )
        return [
            lineLayer(
                from: from,
                to: to,
                lineWidth: 10,
                color: lineColor.withAlphaComponent(0.18)
            ),
            lineLayer(
                from: from,
                to: to,
                lineWidth: 6,
                color: lineColor,
                dashPattern: [12, 8]
            ),
            circleLayer(center: from, radius: 10, fillColor: lineColor.withAlphaComponent(0.85)),
            circleLayer(center: to, radius: 28, fillColor: lineColor.withAlphaComponent(0.18)),
            circleLayer(center: to, radius: 22, strokeColor: lineColor, lineWidth: 5),
            circleLayer(center: to, radius: 6, fillColor: lineColor),
            arrowHeadLayer(from: from, to: to, color: lineColor)
        ]
    }

    private func scrollLayers(at point: CGPoint, dx: Double, dy: Double) -> [CALayer] {
        let color = NSColor(
            calibratedRed: 0.98,
            green: 0.70,
            blue: 0.24,
            alpha: 0.92
        )
        let direction = normalizedVector(dx: dx, dy: dy)
        let end = CGPoint(x: point.x + (direction.dx * 40), y: point.y + (direction.dy * 40))

        return [
            circleLayer(center: point, radius: 32, fillColor: color.withAlphaComponent(0.20)),
            circleLayer(
                center: point,
                radius: 26,
                strokeColor: color.withAlphaComponent(0.82),
                lineWidth: 5
            ),
            lineLayer(
                from: point,
                to: end,
                lineWidth: 11,
                color: NSColor.white.withAlphaComponent(0.18)
            ),
            lineLayer(from: point, to: end, lineWidth: 7, color: color),
            arrowHeadLayer(from: point, to: end, color: color)
        ]
    }

    private func keyboardLayers(label: String) -> [CALayer] {
        let title = "KEYBOARD"
        let titleFont = NSFont.monospacedSystemFont(ofSize: 12, weight: .medium)
        let bodyFont = NSFont.monospacedSystemFont(ofSize: 24, weight: .semibold)
        let titleSize = textSize(title, font: titleFont)
        let bodySize = textSize(label, font: bodyFont)
        let maxPanelWidth = max(min(bounds.width - 48, 720), 220)
        let panelWidth = min(max(max(titleSize.width, bodySize.width) + 56, 220), maxPanelWidth)
        let panelHeight: CGFloat = 96
        let panelRect = CGRect(
            x: (bounds.width - panelWidth) / 2,
            y: max(36, min(72, bounds.height - panelHeight - 24)),
            width: panelWidth,
            height: panelHeight
        )

        let panel = CALayer()
        panel.frame = panelRect
        panel.cornerRadius = 18
        panel.backgroundColor = NSColor(
            calibratedWhite: 0.06,
            alpha: 0.88
        ).cgColor
        panel.borderWidth = 1
        panel.borderColor = NSColor.white.withAlphaComponent(0.10).cgColor
        panel.shadowColor = NSColor.black.cgColor
        panel.shadowOpacity = 0.28
        panel.shadowRadius = 22
        panel.shadowOffset = CGSize(width: 0, height: 10)

        let accent = CALayer()
        accent.frame = CGRect(
            x: panelRect.minX + 20,
            y: panelRect.maxY - 16,
            width: panelRect.width - 40,
            height: 4
        )
        accent.cornerRadius = 2
        accent.backgroundColor = NSColor(
            calibratedRed: 0.29,
            green: 0.76,
            blue: 0.99,
            alpha: 0.98
        ).cgColor

        let titleLayer = textLayer(
            text: title,
            font: titleFont,
            color: NSColor.white.withAlphaComponent(0.72),
            frame: CGRect(
                x: panelRect.minX + 22,
                y: panelRect.maxY - 38,
                width: panelRect.width - 44,
                height: 16
            )
        )

        let bodyLayer = textLayer(
            text: label,
            font: bodyFont,
            color: NSColor.white,
            frame: CGRect(
                x: panelRect.minX + 22,
                y: panelRect.minY + 24,
                width: panelRect.width - 44,
                height: 34
            )
        )

        return [panel, accent, titleLayer, bodyLayer]
    }

    private func clickColor(_ mode: String) -> NSColor {
        switch mode {
        case "right":
            return NSColor(calibratedRed: 0.98, green: 0.54, blue: 0.24, alpha: 0.92)
        case "middle":
            return NSColor(calibratedRed: 0.88, green: 0.34, blue: 0.82, alpha: 0.92)
        case "double":
            return NSColor(calibratedRed: 0.16, green: 0.84, blue: 0.57, alpha: 0.92)
        default:
            return NSColor(calibratedRed: 0.21, green: 0.62, blue: 0.98, alpha: 0.92)
        }
    }

    private func normalizedVector(dx: Double, dy: Double) -> CGVector {
        let magnitude = max(sqrt((dx * dx) + (dy * dy)), 1.0)
        return CGVector(dx: dx / magnitude, dy: dy / magnitude)
    }

    private func circleLayer(
        center: CGPoint,
        radius: CGFloat,
        fillColor: NSColor? = nil,
        strokeColor: NSColor? = nil,
        lineWidth: CGFloat = 0,
        dashPattern: [NSNumber]? = nil
    ) -> CAShapeLayer {
        let path = CGMutablePath()
        path.addEllipse(in: CGRect(
            x: center.x - radius,
            y: center.y - radius,
            width: radius * 2,
            height: radius * 2
        ))
        let layer = CAShapeLayer()
        layer.path = path
        layer.fillColor = fillColor?.cgColor ?? NSColor.clear.cgColor
        layer.strokeColor = strokeColor?.cgColor
        layer.lineWidth = lineWidth
        layer.lineDashPattern = dashPattern
        return layer
    }

    private func lineLayer(
        from: CGPoint,
        to: CGPoint,
        lineWidth: CGFloat,
        color: NSColor,
        dashPattern: [NSNumber]? = nil
    ) -> CAShapeLayer {
        let path = CGMutablePath()
        path.move(to: from)
        path.addLine(to: to)
        let layer = CAShapeLayer()
        layer.path = path
        layer.strokeColor = color.cgColor
        layer.fillColor = NSColor.clear.cgColor
        layer.lineWidth = lineWidth
        layer.lineCap = .round
        layer.lineJoin = .round
        layer.lineDashPattern = dashPattern
        return layer
    }

    private func arrowHeadLayer(from: CGPoint, to: CGPoint, color: NSColor) -> CAShapeLayer {
        let dx = to.x - from.x
        let dy = to.y - from.y
        let magnitude = max(sqrt((dx * dx) + (dy * dy)), 1)
        let unitX = dx / magnitude
        let unitY = dy / magnitude
        let arrowLength: CGFloat = 16
        let arrowWidth: CGFloat = 10
        let left = CGPoint(
            x: to.x - (unitX * arrowLength) + (unitY * arrowWidth),
            y: to.y - (unitY * arrowLength) - (unitX * arrowWidth)
        )
        let right = CGPoint(
            x: to.x - (unitX * arrowLength) - (unitY * arrowWidth),
            y: to.y - (unitY * arrowLength) + (unitX * arrowWidth)
        )

        let path = CGMutablePath()
        path.move(to: to)
        path.addLine(to: left)
        path.addLine(to: right)
        path.closeSubpath()

        let layer = CAShapeLayer()
        layer.path = path
        layer.fillColor = color.cgColor
        layer.strokeColor = color.cgColor
        return layer
    }

    private func textLayer(
        text: String,
        font: NSFont,
        color: NSColor,
        frame: CGRect
    ) -> CATextLayer {
        let layer = CATextLayer()
        layer.frame = frame
        layer.alignmentMode = .center
        layer.isWrapped = false
        layer.truncationMode = .end
        layer.contentsScale = NSScreen.main?.backingScaleFactor ?? 2.0
        layer.string = NSAttributedString(
            string: text,
            attributes: [
                .font: font,
                .foregroundColor: color,
            ]
        )
        return layer
    }

    private func textSize(_ text: String, font: NSFont) -> CGSize {
        (text as NSString).size(withAttributes: [.font: font])
    }
}

guard CommandLine.arguments.count == 2 else {
    exit(0)
}

guard let requestData = CommandLine.arguments[1].data(using: .utf8),
      let request = try? JSONDecoder().decode(EffectRequest.self, from: requestData),
      let firstScreen = NSScreen.screens.first
else {
    exit(0)
}

let unionFrame = NSScreen.screens.dropFirst().reduce(firstScreen.frame) { partial, screen in
    partial.union(screen.frame)
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)

let window = NSWindow(
    contentRect: unionFrame,
    styleMask: .borderless,
    backing: .buffered,
    defer: false
)
window.isOpaque = false
window.backgroundColor = .clear
window.hasShadow = false
window.ignoresMouseEvents = true
window.level = .screenSaver
window.collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary]
window.contentView = OverlayView(
    frame: CGRect(origin: .zero, size: unionFrame.size),
    unionFrame: unionFrame,
    request: request
)
window.alphaValue = 1.0
window.orderFrontRegardless()

DispatchQueue.main.asyncAfter(deadline: .now() + 0.18) {
    NSAnimationContext.runAnimationGroup { context in
        context.duration = 0.12
        window.animator().alphaValue = 0.0
    } completionHandler: {
        app.terminate(nil)
    }
}

DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) {
    app.terminate(nil)
}

app.run()
