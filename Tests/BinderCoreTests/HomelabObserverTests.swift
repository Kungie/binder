import Foundation
import Testing
@testable import BinderCore

private final class SpyProbe: LANProbe, @unchecked Sendable {
    var calls = 0
    let answers: [LANHost]

    init(_ answers: [LANHost]) { self.answers = answers }

    func scan(ports: [Int]) -> [LANHost] {
        calls += 1
        return answers
    }
}

private func homeWithSSHConfig() throws -> URL {
    let home = makeTempDir("home")
    let ssh = home.appendingPathComponent(".ssh")
    try FileManager.default.createDirectory(at: ssh.appendingPathComponent("config.d"), withIntermediateDirectories: true)
    try FileManager.default.copyItem(at: fixture("ssh/config"), to: ssh.appendingPathComponent("config"))
    try FileManager.default.copyItem(at: fixture("ssh/config.d/homelab"), to: ssh.appendingPathComponent("config.d/homelab"))
    return home
}

@Suite struct HomelabObserverTests {
    @Test func homelabHostsBecomeProposalsNotNotes() throws {
        let home = try homeWithSSHConfig()
        let obs = HomelabObserver(probe: SpyProbe([])).observe(ObserverContext(home: home, projectRoots: [], runner: FakeRunner()))
        #expect(obs.notes.isEmpty)
        #expect(obs.proposals.map(\.text) == ["proxmox at 192.168.1.20 is part of your homelab."])
        let p = try #require(obs.proposals.first)
        #expect(p.kind == .fact && p.source == .observed && p.target == "servers/proxmox.md" && p.agent == "binder")
    }

    @Test func lanProbeOnlyWhenAllowed() throws {
        let home = try homeWithSSHConfig()
        let probe = SpyProbe([LANHost(ip: "192.168.1.20", ports: [22, 8006]), LANHost(ip: "192.168.1.50", ports: [22]),
                              LANHost(ip: "192.168.1.60", ports: [8123, 22])])
        _ = HomelabObserver(probe: probe).observe(ObserverContext(home: home, projectRoots: [], runner: FakeRunner(), allowLAN: false))
        #expect(probe.calls == 0)
        let obs = HomelabObserver(probe: probe).observe(ObserverContext(home: home, projectRoots: [], runner: FakeRunner(), allowLAN: true))
        #expect(probe.calls == 1)
        let texts = obs.proposals.map(\.text)
        #expect(texts.contains("Something answers SSH on 192.168.1.50."))
        #expect(texts.contains("Something answers on 192.168.1.60: SSH, Home Assistant."))
        #expect(!texts.contains { $0.contains("on 192.168.1.20") })
    }

    @Test func privateAddressRanges() {
        for yes in ["10.0.0.4", "172.16.3.1", "172.31.255.1", "192.168.1.20", "nas.local"] { #expect(HomelabObserver.isPrivate(yes), "\(yes)") }
        for no in ["116.203.10.20", "172.32.0.1", "github.com", "8.8.8.8"] { #expect(!HomelabObserver.isPrivate(no), "\(no)") }
    }
}
