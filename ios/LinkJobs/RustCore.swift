import Foundation
import LinkJobsCore

/// Owns the Rust side: a Rivium embedded host running the Apalis worker + loopback API.
///
/// `linkjobs_start` / `linkjobs_stop` block while services start or drain, so they always
/// run off the main thread.
actor RustCore {
    static let shared = RustCore()

    enum Status: Int32 {
        case idle = 0, starting, running, stopping, restarting, failed
    }

    /// Jobs and logs live here, so they survive the app being closed.
    static var rootDirectory: URL {
        let base = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask)[0]
        return base.appendingPathComponent("linkjobs", isDirectory: true)
    }

    static var baseURL: URL {
        URL(string: "http://127.0.0.1:\(linkjobs_port())")!
    }

    var status: Status { Status(rawValue: linkjobs_status()) ?? .failed }

    /// Start the host if it is not already running. Returns an error message on failure.
    @discardableResult
    func start() async -> String? {
        if status == .running { return nil }
        let root = Self.rootDirectory
        try? FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
        let code: Int32 = await Task.detached(priority: .userInitiated) {
            root.path.withCString { linkjobs_start($0) }
        }.value
        // 0 = started, 1 = already running.
        if code == 0 || code == 1 { return nil }
        return "start failed (\(code)): \(Self.lastError() ?? "unknown error")"
    }

    func stop(timeout: Duration = .seconds(3)) async {
        let millis = UInt64(timeout.components.seconds * 1000)
        _ = await Task.detached { linkjobs_stop(millis) }.value
    }

    static func lastError() -> String? { take(linkjobs_last_error()) }
    static func version() -> String { take(linkjobs_version()) ?? "?" }

    private static func take(_ pointer: UnsafeMutablePointer<CChar>?) -> String? {
        guard let pointer else { return nil }
        defer { linkjobs_string_free(pointer) }
        return String(cString: pointer)
    }
}
