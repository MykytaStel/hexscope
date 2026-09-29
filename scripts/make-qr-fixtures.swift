// Writes QR codes made by Core Image, the system's own encoder, for the QR
// reader's tests: one module a pixel, at every error correction level and
// lengths from version 1 to 40; and a few as a camera sees them — tilted,
// blurred, on a busy background.
//
//   swiftc -O -o /tmp/make-qr-fixtures scripts/make-qr-fixtures.swift
//   /tmp/make-qr-fixtures apps/web/src/qr/fixtures
import CoreImage
import Foundation
import ImageIO
import UniformTypeIdentifiers

let out = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
try? FileManager.default.createDirectory(at: out, withIntermediateDirectories: true)
let context = CIContext()

func code(_ text: String, _ level: String) -> CIImage {
  let f = CIFilter(name: "CIQRCodeGenerator")!
  f.setValue(text.data(using: .utf8), forKey: "inputMessage")
  f.setValue(level, forKey: "inputCorrectionLevel")
  return f.outputImage!
}

func save(_ image: CIImage, _ name: String) {
  let gray = CGColorSpaceCreateDeviceGray()
  let cg = context.createCGImage(image, from: image.extent, format: .L8, colorSpace: gray)!
  let url = out.appendingPathComponent(name)
  let dest = CGImageDestinationCreateWithURL(url as CFURL, UTType.png.identifier as CFString, 1, nil)!
  CGImageDestinationAddImage(dest, cg, nil)
  CGImageDestinationFinalize(dest)
}

/// A text of `n` characters, different at every length.
func text(_ n: Int) -> String {
  let words = "hexscope reads the file you give it, in your browser, and nothing leaves the tab. "
  var s = "https://hexscope.pages.dev/?n=\(n)&t="
  while s.count < n { s += words }
  return String(s.prefix(n))
}

// Byte-mode capacity of each version at level L: each length fills one
// version from 1 to 40 at L, and lands somewhere at the other levels.
let lengths = [17, 32, 53, 78, 106, 134, 154, 192, 230, 271, 321, 367, 425, 458, 520, 586, 644, 718, 792, 858,
               929, 1003, 1091, 1171, 1273, 1367, 1465, 1528, 1628, 1732, 1840, 1952, 2068, 2188, 2303, 2431,
               2563, 2699, 2809, 2953]
let limits = ["L": 2953, "M": 2331, "Q": 1663, "H": 1273]
// Every version at L; every third length at the other levels.
for (level, limit) in limits {
  for (i, n) in lengths.enumerated() where n <= limit && (level == "L" || i % 3 == 0) {
    save(code(text(n), level), "\(level)-\(n).png")
  }
}

// As a camera sees one: on a photo-like background, scaled up, turned,
// tilted and softened.
func scene(_ image: CIImage, turn: CGFloat, tilt: CGFloat, blur: Double) -> CIImage {
  var img = image.transformed(by: CGAffineTransform(scaleX: 6, y: 6))
  img = img.transformed(by: CGAffineTransform(rotationAngle: turn))
  let e = img.extent
  let p = CIFilter(name: "CIPerspectiveTransform")!
  p.setValue(img, forKey: kCIInputImageKey)
  p.setValue(CIVector(x: e.minX + tilt, y: e.maxY), forKey: "inputTopLeft")
  p.setValue(CIVector(x: e.maxX - tilt, y: e.maxY - tilt / 2), forKey: "inputTopRight")
  p.setValue(CIVector(x: e.minX, y: e.minY), forKey: "inputBottomLeft")
  p.setValue(CIVector(x: e.maxX, y: e.minY + tilt / 3), forKey: "inputBottomRight")
  img = p.outputImage!
  let backdrop = CIFilter(name: "CILinearGradient")!
  backdrop.setValue(CIVector(x: img.extent.minX, y: img.extent.minY), forKey: "inputPoint0")
  backdrop.setValue(CIVector(x: img.extent.maxX, y: img.extent.maxY), forKey: "inputPoint1")
  backdrop.setValue(CIColor(red: 0.55, green: 0.6, blue: 0.5), forKey: "inputColor0")
  backdrop.setValue(CIColor(red: 0.95, green: 0.9, blue: 0.85), forKey: "inputColor1")
  let frame = img.extent.insetBy(dx: -80, dy: -60)
  img = img.composited(over: backdrop.outputImage!.cropped(to: frame))
  if blur > 0 { img = img.applyingGaussianBlur(sigma: blur).cropped(to: frame) }
  return img
}
save(scene(code("WIFI:S:Home Network;T:WPA;P:correct horse battery staple;;", "M"), turn: 0.2, tilt: 30, blur: 1.2), "scene-wifi.png")
save(scene(code("https://xn--exmple-bank-zij.com/verify?id=7Q1", "Q"), turn: -0.35, tilt: 25, blur: 0.8), "scene-link.png")
save(scene(code(text(300), "L"), turn: 0.05, tilt: 15, blur: 0.6), "scene-v10.png")

// A one-page invoice with a code to "pay" at a lookalike address, as a PDF
// draws a picture: an image on the page.
do {
  let url = out.appendingPathComponent("invoice.pdf")
  var box = CGRect(x: 0, y: 0, width: 595, height: 842)
  let pdf = CGContext(url as CFURL, mediaBox: &box, nil)!
  pdf.beginPDFPage(nil)
  let qr = code("https://xn--exmple-bank-zij.com/pay?invoice=4471", "M")
  let cg = context.createCGImage(qr, from: qr.extent, format: .L8, colorSpace: CGColorSpaceCreateDeviceGray())!
  pdf.interpolationQuality = .none
  pdf.draw(cg, in: CGRect(x: 380, y: 620, width: 150, height: 150))
  pdf.setFillColor(gray: 0.1, alpha: 1)
  pdf.fill(CGRect(x: 60, y: 760, width: 240, height: 14))
  pdf.endPDFPage()
  pdf.closePDF()
}
