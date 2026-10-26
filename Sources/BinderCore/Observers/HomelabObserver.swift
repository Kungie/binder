import Darwin
import Foundation

public struct LANHost: Equatable, Sendable {
    public var ip: String
    public var ports: [Int]

    public init(ip: String, ports: [Int]) {
        self.ip = ip
        self.ports = ports
    }
}

/// Finds machines on the local /24 that answer on the given TCP ports.
public protocol LANProbe: Sendable {
    func scan(ports: [Int]) -> [LANHost]
}

/// The homelab. Everything here becomes a proposal, never a note: Binder guesses, the user confirms.
public struct HomelabObserver: Observer {
    public let name = "homelab"
    let probe: LANProbe

    public init(probe: LANProbe = SystemLANProbe()) {
        self.probe = probe
    }

    static let ports = [22, 80, 443, 8006, 8123]
    static let portNames: [(port: Int, name: String)] = [(22, "SSH"), (8006, "Proxmox"), (8123, "Home Assistant"), (80, "HTTP"), (443, "HTTPS")]

    public func observe(_ ctx: ObserverContext) -> Observation {
        let hosts = SSHConfig.load(ctx.home.appendingPathComponent(".ssh/config")).hosts
        var obs = Observation()
        var known = Set<String>()
        for host in hosts {
            let address = host.hostName ?? host.alias
            known.insert(address)
            guard Self.isPrivate(address) else { continue }
            obs.proposals.append(proposal("\(host.alias) at \(address) is part of your homelab.", target: "servers/\(host.alias).md", evidence: "~/.ssh/config"))
        }
        guard ctx.allowLAN else { return obs }
        for found in probe.scan(ports: Self.ports) where !known.contains(found.ip) {
            let names = Self.portNames.filter { found.ports.contains($0.port) }.map(\.name)
            guard !names.isEmpty else { continue }
            let text = names == ["SSH"] ? "Something answers SSH on \(found.ip)." : "Something answers on \(found.ip): \(names.joined(separator: ", "))."
            obs.proposals.append(proposal(text, target: "servers/homelab.md", evidence: "local network, ports \(found.ports.map(String.init).joined(separator: ", "))"))
        }
        return obs
    }

    private func proposal(_ text: String, target: String, evidence: String) -> Proposal {
        Proposal.make(kind: .fact, source: .observed, text: text, evidence: evidence, target: target,
                      project: nil, session: nil, confidence: .medium, agent: "binder")
    }

    /// 10/8, 172.16/12, 192.168/16, or a `.local` name.
    public static func isPrivate(_ address: String) -> Bool {
        if address.hasSuffix(".local") { return true }
        let parts = address.split(separator: ".").compactMap { Int($0) }
        guard parts.count == 4 else { return false }
        return parts[0] == 10 || (parts[0] == 172 && (16...31).contains(parts[1])) || (parts[0] == 192 && parts[1] == 168)
    }
}

/// Tries a TCP connection to each port of each address in this Mac's /24, 200 ms per try, 64 at a time.
public struct SystemLANProbe: LANProbe {
    public init() {}

    public func scan(ports: [Int]) -> [LANHost] {
        guard let (prefix, own) = Self.localSubnet() else { return [] }
        let queue = OperationQueue()
        queue.maxConcurrentOperationCount = 64
        let lock = NSLock()
        var found: [String: [Int]] = [:]
        for last in 1...254 where last != own {
            for port in ports {
                queue.addOperation {
                    let ip = "\(prefix).\(last)"
                    guard Self.isOpen(ip, port) else { return }
                    lock.lock()
                    found[ip, default: []].append(port)
                    lock.unlock()
                }
            }
        }
        queue.waitUntilAllOperationsAreFinished()
        return found.map { LANHost(ip: $0.key, ports: $0.value.sorted()) }.sorted { $0.ip < $1.ip }
    }

    /// The first private IPv4 address on a non-loopback interface, as `("192.168.1", 23)`.
    public static func localSubnet() -> (String, Int)? {
        var list: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&list) == 0, let first = list else { return nil }
        defer { freeifaddrs(list) }
        var cursor: UnsafeMutablePointer<ifaddrs>? = first
        while let entry = cursor {
            defer { cursor = entry.pointee.ifa_next }
            guard let addr = entry.pointee.ifa_addr, addr.pointee.sa_family == UInt8(AF_INET) else { continue }
            var host = [CChar](repeating: 0, count: Int(NI_MAXHOST))
            guard getnameinfo(addr, socklen_t(addr.pointee.sa_len), &host, socklen_t(host.count), nil, 0, NI_NUMERICHOST) == 0 else { continue }
            let ip = String(cString: host)
            guard HomelabObserver.isPrivate(ip) else { continue }
            let parts = ip.split(separator: ".")
            return (parts.prefix(3).joined(separator: "."), Int(parts[3]) ?? 0)
        }
        return nil
    }

    public static func isOpen(_ ip: String, _ port: Int, timeoutMs: Int32 = 200) -> Bool {
        let fd = socket(AF_INET, SOCK_STREAM, 0)
        guard fd >= 0 else { return false }
        defer { close(fd) }
        _ = fcntl(fd, F_SETFL, fcntl(fd, F_GETFL) | O_NONBLOCK)
        var addr = sockaddr_in()
        addr.sin_family = sa_family_t(AF_INET)
        addr.sin_port = in_port_t(UInt16(port).bigEndian)
        guard inet_pton(AF_INET, ip, &addr.sin_addr) == 1 else { return false }
        let result = withUnsafePointer(to: &addr) {
            $0.withMemoryRebound(to: sockaddr.self, capacity: 1) { connect(fd, $0, socklen_t(MemoryLayout<sockaddr_in>.size)) }
        }
        if result == 0 { return true }
        guard errno == EINPROGRESS else { return false }
        var pfd = pollfd(fd: fd, events: Int16(POLLOUT), revents: 0)
        guard poll(&pfd, 1, timeoutMs) == 1 else { return false }
        var error: Int32 = 0
        var length = socklen_t(MemoryLayout<Int32>.size)
        getsockopt(fd, SOL_SOCKET, SO_ERROR, &error, &length)
        return error == 0
    }
}
