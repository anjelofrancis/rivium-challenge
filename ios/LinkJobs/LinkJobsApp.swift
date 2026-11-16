import SwiftUI
import UserNotifications

@main
struct LinkJobsApp: App {
    @StateObject private var store = JobsStore()
    @Environment(\.scenePhase) private var scenePhase
    private let notificationDelegate = NotificationDelegate()

    init() {
        UNUserNotificationCenter.current().delegate = notificationDelegate
    }

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environmentObject(store)
                // scenePhase's onChange does not fire for the initial launch phase.
                .task { store.start() }
                // linkjobs://add?url=https://example.com
                .onOpenURL { url in
                    guard let items = URLComponents(url: url, resolvingAgainstBaseURL: false)?.queryItems,
                          let link = items.first(where: { $0.name == "url" })?.value
                    else { return }
                    Task { await store.enqueue(link) }
                }
        }
        .onChange(of: scenePhase) { _, phase in
            switch phase {
            case .active: store.start()
            case .background: store.stopPolling()
            default: break
            }
        }
    }
}
