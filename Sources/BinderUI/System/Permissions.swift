import AppKit
import BinderCore
import Foundation
import ServiceManagement
import UserNotifications

public enum Permission: String, CaseIterable, Codable, Sendable {
    case folders, localNetwork, notifications, terminal, login
}

public enum PermissionState: String, Codable, Sendable {
    case unknown
    /// macOS doesn't say whether it was granted (local network): we asked, and that's all we know.
    case asked
    case granted
    case denied
}

public protocol PermissionRequesting: Sendable {
    func status(_ p: Permission) -> PermissionState
    func request(_ p: Permission) async -> PermissionState
}

/// The real requests. Each one triggers macOS's own prompt the first time.
public struct SystemPermissions: PermissionRequesting {
    public init() {}

    public func status(_ p: Permission) -> PermissionState {
        switch p {
        case .login:
            return SMAppService.mainApp.status == .enabled ? .granted : .unknown
        default:
            return .unknown
        }
    }

    public func request(_ p: Permission) async -> PermissionState {
        switch p {
        case .folders:
            // Listing the folders is what makes macOS ask.
            let home = FileManager.default.homeDirectoryForCurrentUser
            let ok = ["Desktop", "Documents"].allSatisfy {
                (try? FileManager.default.contentsOfDirectory(atPath: home.appendingPathComponent($0).path)) != nil
            }
            return ok ? .granted : .denied
        case .localNetwork:
            // A connection attempt to the router makes macOS ask. There's no API to read the answer.
            if let (prefix, _) = SystemLANProbe.localSubnet() {
                _ = SystemLANProbe.isOpen("\(prefix).1", 9, timeoutMs: 300)
            }
            return .asked
        case .notifications:
            let granted = (try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound])) ?? false
            return granted ? .granted : .denied
        case .terminal:
            return await MainActor.run {
                var error: NSDictionary?
                NSAppleScript(source: "tell application \"Terminal\" to get name")?.executeAndReturnError(&error)
                return error == nil ? .granted : .denied
            }
        case .login:
            do {
                try SMAppService.mainApp.register()
                return .granted
            } catch {
                return .denied
            }
        }
    }
}
