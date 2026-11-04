import CoreGraphics
import Testing
@testable import BinderUI

@Suite struct FlowLayoutTests {
    @Test func wrapsToANewRowWhenFull() {
        let sizes = Array(repeating: CGSize(width: 40, height: 20), count: 5)
        let (positions, size) = FlowLayout.arrange(sizes: sizes, width: 100, spacing: 6)
        #expect(positions == [CGPoint(x: 0, y: 0), CGPoint(x: 46, y: 0), CGPoint(x: 0, y: 26), CGPoint(x: 46, y: 26), CGPoint(x: 0, y: 52)])
        #expect(size == CGSize(width: 86, height: 72))
    }

    @Test func neverSquashesAWideChild() {
        let (positions, size) = FlowLayout.arrange(sizes: [CGSize(width: 150, height: 20)], width: 100, spacing: 6)
        #expect(positions == [.zero])
        #expect(size.width == 150)
    }
}
