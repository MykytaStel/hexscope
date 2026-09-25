// Writes a one-second QuickTime movie the way an iPhone's camera does:
// H.264 frames, and metadata in the QuickTime "mdta" keys — location
// (ISO 6709), make, model, software and creation date. The values are
// samples, not anyone's.
//
//   swiftc -O -o /tmp/make-sample-video scripts/make-sample-video.swift
//   /tmp/make-sample-video crates/hexscope-core/tests/fixtures/iphone.mov
import AVFoundation
import CoreVideo

let out = URL(fileURLWithPath: CommandLine.arguments[1])
try? FileManager.default.removeItem(at: out)
let writer = try! AVAssetWriter(outputURL: out, fileType: .mov)

func item(_ id: AVMetadataIdentifier, _ value: String) -> AVMetadataItem {
  let m = AVMutableMetadataItem()
  m.identifier = id
  m.value = value as NSString
  m.dataType = kCMMetadataBaseDataType_UTF8 as String
  return m
}
writer.metadata = [
  item(.quickTimeMetadataLocationISO6709, "+48.8584+002.2945+035.000/"),
  item(.quickTimeMetadataMake, "hexscope"),
  item(.quickTimeMetadataModel, "Sample Camera X1"),
  item(.quickTimeMetadataSoftware, "hexscope sample generator"),
  item(.quickTimeMetadataCreationDate, "2026-06-14T18:32:07+0200"),
]

let (w, h) = (64, 48)
let input = AVAssetWriterInput(mediaType: .video, outputSettings: [
  AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: w, AVVideoHeightKey: h,
])
input.expectsMediaDataInRealTime = false
let adaptor = AVAssetWriterInputPixelBufferAdaptor(assetWriterInput: input, sourcePixelBufferAttributes: nil)
writer.add(input)
writer.startWriting()
writer.startSession(atSourceTime: .zero)

for frame in 0..<5 {
  var buffer: CVPixelBuffer?
  CVPixelBufferCreate(nil, w, h, kCVPixelFormatType_32BGRA, nil, &buffer)
  let pb = buffer!
  CVPixelBufferLockBaseAddress(pb, [])
  let base = CVPixelBufferGetBaseAddress(pb)!.assumingMemoryBound(to: UInt8.self)
  let stride = CVPixelBufferGetBytesPerRow(pb)
  for y in 0..<h {
    for x in 0..<w {
      let p = base + y * stride + x * 4
      p[0] = UInt8((x * 4 + frame * 40) & 255)
      p[1] = UInt8(y * 5)
      p[2] = 200
      p[3] = 255
    }
  }
  CVPixelBufferUnlockBaseAddress(pb, [])
  while !input.isReadyForMoreMediaData { usleep(1000) }
  adaptor.append(pb, withPresentationTime: CMTime(value: CMTimeValue(frame), timescale: 5))
}
input.markAsFinished()
let done = DispatchSemaphore(value: 0)
writer.finishWriting { done.signal() }
done.wait()
print(writer.status == .completed ? "wrote \(out.path)" : "failed: \(String(describing: writer.error))")
