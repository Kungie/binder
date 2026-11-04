import CoreGraphics
import Foundation
import ImageIO
import Testing
import UniformTypeIdentifiers
@testable import BinderCore

/// A PNG of the given size, filled with a shade that depends on `seed`, so every seed hashes differently.
func pngData(_ width: Int, _ height: Int, seed: Int = 0) -> Data {
    let ctx = CGContext(data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: 0,
                        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    ctx.setFillColor(red: CGFloat(seed % 255) / 255, green: 0.4, blue: 0.1, alpha: 1)
    ctx.fill(CGRect(x: 0, y: 0, width: width, height: height))
    let out = NSMutableData()
    let dest = CGImageDestinationCreateWithData(out, UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(dest, ctx.makeImage()!, nil)
    CGImageDestinationFinalize(dest)
    return out as Data
}

@Suite struct FileCatchTests {
    let vault: Vault = {
        let v = Vault(root: makeTempDir("files"))
        try? v.ensureLayout()
        return v
    }()

    func line(_ content: [[String: Any]], type: String = "user") -> String {
        let object: [String: Any] = ["type": type, "timestamp": "2026-11-04T10:00:00.000Z", "message": ["role": type, "content": content]]
        return String(data: try! JSONSerialization.data(withJSONObject: object), encoding: .utf8)!
    }

    func image(_ data: Data, media: String = "image/png") -> [String: Any] {
        ["type": "image", "source": ["type": "base64", "media_type": media, "data": data.base64EncodedString()]]
    }

    func entries(_ lines: [String]) throws -> [TranscriptEntry] {
        let url = makeTempDir("t").appendingPathComponent("s.jsonl")
        try Data((lines.joined(separator: "\n") + "\n").utf8).write(to: url)
        return try Transcript.read(url, from: 0).entries
    }

    func stage(_ entries: [TranscriptEntry], cwd: String = "/nowhere", since: Date? = nil, offered: Int = 0) throws -> [Proposal] {
        try FileCatch.stage(FileCatch.candidates(entries: entries, cwd: cwd, since: since), vault: vault, project: "dawn", session: "s", offeredSoFar: offered)
    }

    @Test func attachedImageBecomesWaitingFile() throws {
        let staged = try stage(entries([line([["type": "text", "text": "Here is the diagram"], image(pngData(100, 100))])]))
        let p = try #require(staged.first)
        #expect(staged.count == 1 && p.kind == .file && p.target == "projects/dawn.md")
        #expect(p.file?.hasSuffix(".png") == true && p.fileHash?.count == 64)
        #expect(FileManager.default.fileExists(atPath: vault.inboxFiles.appendingPathComponent(p.file!).path))
        #expect(p.context == "Here is the diagram")
        #expect(p.text == "Image from the session" && p.evidence == "attached in the session")
    }

    @Test func pdfDocumentBlockIsCaught() throws {
        let pdf = Data("%PDF-1.4\n1 0 obj << >> endobj\ntrailer << >>\n%%EOF\n".utf8)
        let block: [String: Any] = ["type": "document", "source": ["type": "base64", "media_type": "application/pdf", "data": pdf.base64EncodedString()]]
        let p = try #require(try stage(entries([line([block])])).first)
        #expect(p.file?.hasSuffix(".pdf") == true && p.text == "PDF from the session")
    }

    @Test func toolResultImagesAreIgnored() throws {
        let result: [String: Any] = ["type": "tool_result", "tool_use_id": "t1", "content": [image(pngData(200, 200))]]
        #expect(try stage(entries([line([result])])).isEmpty)
    }

    @Test func madeFileNeedsToBeNewAndExist() throws {
        let cwd = makeTempDir("work")
        let start = Date().addingTimeInterval(-60)
        try pngData(120, 120, seed: 1).write(to: cwd.appendingPathComponent("new-chart.png"))
        try pngData(120, 120, seed: 2).write(to: cwd.appendingPathComponent("old.png"))
        try FileManager.default.setAttributes([.modificationDate: start.addingTimeInterval(-3600)], ofItemAtPath: cwd.appendingPathComponent("old.png").path)
        let staged = try stage([.command("python plot.py --out new-chart.png && open old.png missing.png")], cwd: cwd.path, since: start)
        #expect(staged.map(\.text) == ["New chart"])
        #expect(staged.first?.evidence == "made in the session: new-chart.png")
        #expect(try stage([.command("open new-chart.png")], cwd: cwd.path, since: nil).isEmpty)
    }

    @Test func perSessionCapAndNewestFirst() throws {
        let lines = (0..<8).map { line([image(pngData(80 + $0, 80, seed: $0))]) }
        let staged = try stage(entries(lines))
        #expect(staged.count == FileCatch.perSession)
        #expect(staged.first?.fileHash == FileCatch.sha256(pngData(87, 80, seed: 7)))
        #expect(try stage(entries(lines), offered: 6).isEmpty)
    }

    @Test func tinyImageSkipped() throws {
        #expect(try stage(entries([line([image(pngData(10, 10))])])).isEmpty)
    }

    @Test func oversizedAttachmentSkippedWithoutDecoding() throws {
        let huge = TranscriptEntry.attachment(mediaType: "image/png", base64: String(repeating: "A", count: 30_000_000), near: "")
        #expect(try stage([huge]).isEmpty)
    }

    @Test func sameFileTwiceIsOfferedOnce() throws {
        let entry = try entries([line([image(pngData(100, 100, seed: 9))])])
        let first = try stage(entry)
        for p in first { #expect(try ProposalStore(vault: vault).add(p)) }
        #expect(try stage(entry).isEmpty)
    }
}
