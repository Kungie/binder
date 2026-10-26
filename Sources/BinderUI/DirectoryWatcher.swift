import Foundation

/// Calls `onChange` (on the main queue, debounced) when files are added, removed or renamed in a folder.
public final class DirectoryWatcher {
    private var source: DispatchSourceFileSystemObject?
    private var pending: DispatchWorkItem?
    private let onChange: () -> Void
    private let debounce: TimeInterval

    public init?(url: URL, debounce: TimeInterval = 0.2, onChange: @escaping () -> Void) {
        let fd = open(url.path, O_EVTONLY)
        guard fd >= 0 else { return nil }
        self.onChange = onChange
        self.debounce = debounce
        let source = DispatchSource.makeFileSystemObjectSource(fileDescriptor: fd, eventMask: [.write, .rename, .delete, .extend], queue: .main)
        source.setEventHandler { [weak self] in self?.fire() }
        source.setCancelHandler { close(fd) }
        source.resume()
        self.source = source
    }

    private func fire() {
        pending?.cancel()
        let work = DispatchWorkItem { [weak self] in self?.onChange() }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + debounce, execute: work)
    }

    public func stop() {
        pending?.cancel()
        source?.cancel()
        source = nil
    }

    deinit { stop() }
}
