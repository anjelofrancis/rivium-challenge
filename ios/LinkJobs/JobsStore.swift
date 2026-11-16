import Foundation
import UserNotifications

/// UI state: polls the Rust API and posts a local notification when a job finishes.
@MainActor
final class JobsStore: ObservableObject {
    @Published private(set) var jobs: [Job] = []
    @Published private(set) var stats = Stats()
    @Published private(set) var coreStatus = "starting…"
    @Published var message: String?

    private var pollTask: Task<Void, Never>?
    /// Jobs we already notified about, persisted so reopening the app doesn't repeat them.
    private var notified: Set<String> {
        get { Set(UserDefaults.standard.stringArray(forKey: "notifiedJobs") ?? []) }
        set { UserDefaults.standard.set(Array(newValue.suffix(200)), forKey: "notifiedJobs") }
    }

    func start() {
        guard pollTask == nil else { return }
        // Don't wait for the user to answer the permission prompt before showing jobs.
        Task { await requestNotificationPermission() }
        pollTask = Task { [weak self] in
            if let error = await RustCore.shared.start() {
                self?.coreStatus = error
            }
            while !Task.isCancelled {
                await self?.refresh()
                try? await Task.sleep(for: .seconds(1))
            }
        }
    }

    func stopPolling() {
        pollTask?.cancel()
        pollTask = nil
    }

    func enqueue(_ text: String) async {
        do {
            _ = await RustCore.shared.start()
            let url = try await JobsClient.enqueue(text)
            message = "Queued \(url)"
            await refresh()
        } catch {
            message = error.localizedDescription
        }
    }

    func clearFinished() async {
        try? await JobsClient.clearFinished()
        await refresh()
    }

    func refresh() async {
        coreStatus = "\(await RustCore.shared.status)"
        guard let jobs = try? await JobsClient.jobs(), let stats = try? await JobsClient.stats() else { return }
        self.jobs = jobs
        self.stats = stats
        notifyFinished(jobs)
    }

    private func notifyFinished(_ jobs: [Job]) {
        var seen = notified
        let fresh = jobs.filter { $0.isFinished && !seen.contains($0.id) }
        for job in fresh {
            seen.insert(job.id)
            let content = UNMutableNotificationContent()
            if job.status == "Done" {
                content.title = job.result?.title ?? "No title found"
                content.body = job.result?.final_url ?? job.url
            } else {
                content.title = "Couldn't fetch link"
                content.body = "\(job.url)\n\(job.error ?? job.status)"
            }
            content.sound = .default
            let request = UNNotificationRequest(identifier: job.id, content: content, trigger: nil)
            UNUserNotificationCenter.current().add(request)
        }
        if !fresh.isEmpty { notified = seen }
    }

    private func requestNotificationPermission() async {
        _ = try? await UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound, .badge])
    }
}

/// Show notifications while the app is in the foreground too.
final class NotificationDelegate: NSObject, UNUserNotificationCenterDelegate {
    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        [.banner, .sound, .list]
    }
}
