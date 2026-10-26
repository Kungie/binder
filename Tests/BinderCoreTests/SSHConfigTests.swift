import Foundation
import Testing
@testable import BinderCore

@Suite struct SSHConfigTests {
    func hosts() throws -> [SSHHost] {
        let text = try String(contentsOf: fixture("ssh/config"), encoding: .utf8)
        return SSHConfig.parse(text, includeBase: fixture("ssh"))
    }

    @Test func parsesHostsUsersPortsAndKeys() throws {
        let hetzner = try #require(try hosts().first { $0.alias == "hetzner-1" })
        #expect(hetzner.hostName == "116.203.10.20")
        #expect(hetzner.user == "eren")
        #expect(hetzner.port == 2222)
        #expect(hetzner.identityFiles == ["~/.ssh/id_ed25519_hetzner"])
        let aliases = try hosts().map(\.alias)
        #expect(aliases.contains("gh-work") && aliases.contains("github-alt"))
        #expect(try hosts().first { $0.alias == "github-alt" }?.hostName == "github.com")
    }

    @Test func followsIncludeGlob() throws {
        let proxmox = try #require(try hosts().first { $0.alias == "proxmox" })
        #expect(proxmox.hostName == "192.168.1.20")
        #expect(proxmox.user == "root")
        #expect(proxmox.identityFiles == ["~/.ssh/id_ed25519_homelab"])
    }

    @Test func skipsWildcardHosts() throws {
        let aliases = try hosts().map(\.alias)
        #expect(!aliases.contains("*"))
        #expect(!aliases.contains { $0.contains("*") || $0.hasPrefix("!") })
        #expect(aliases.count == 5)
    }
}
