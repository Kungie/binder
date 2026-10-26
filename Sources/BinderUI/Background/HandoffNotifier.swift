import BinderCore
import Foundation
import UserNotifications

/// Which sessions were already nudged, so each one is nudged once.
public protocol NotifiedStore: AnyObject {
    func contains(_ session: String) -> Bool
    func insert(_ session: String)
}

public final class InMemoryNotified: NotifiedStore {
    private var ids = Set<String>()
    public init() {}
    public func contains(_ session: String) -> Bool { ids.contains(session) }
    public func insert(_ session: String) { ids.insert(session) }
}

/// Kept in UserDefaults, the last 200 ids.
public final class DefaultsNotified: NotifiedStore {
    let key = "notifiedSessions"
    public init() {}
    public func contains(_ session: String) -> Bool { (UserDefaults.standard.stringArray(forKey: key) ?? []).contains(session) }
    public func insert(_ session: String) {
        var ids = UserDefaults.standard.stringArray(forKey: key) ?? []
        ids.append(session)
        UserDefaults.standard.set(Array(ids.suffix(200)), forKey: key)
    }
}

/// One nudge per session, when its context crosses the handoff threshold.
public final class HandoffNotifier {
    let store: NotifiedStore

    public init(store: NotifiedStore = DefaultsNotified()) {
        self.store = store
    }

    /// True (and remembered) the first time this session is at or over the threshold.
    public func check(_ status: SessionStatus, threshold: Double) -> Bool {
        guard let fraction = status.reading?.fraction, fraction >= threshold, !store.contains(status.session) else { return false }
        store.insert(status.session)
        return true
    }

    public func post(for status: SessionStatus) {
        let content = UNMutableNotificationContent()
        content.title = "Binder"
        content.body = "\(status.project ?? "This session") is getting long. A fresh session will be sharper."
        let request = UNNotificationRequest(identifier: "handoff-\(status.session)", content: content, trigger: nil)
        UNUserNotificationCenter.current().add(request)
    }
}
