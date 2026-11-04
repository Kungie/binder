import Foundation
import Testing
@testable import BinderCore

@Suite struct CardSearchTests {
    @Test func headerNamesHelperPath() throws {
        let card = CardBuilder.buildIncluded(vault: try fixtureVault(), repo: dawnRepo, helper: "/Applications/Binder.app/Contents/Helpers/binder").card
        #expect(card.components(separatedBy: "\n")[1] == #"Missing something? Run "/Applications/Binder.app/Contents/Helpers/binder" search <words>."#)
    }

    @Test func noHelperNoLine() throws {
        #expect(!CardBuilder.build(vault: try fixtureVault(), repo: dawnRepo).contains("Missing something?"))
    }
}
