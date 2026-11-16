import AppIntents

/// "Fetch Page Title" action for Shortcuts, Siri and Spotlight.
///
/// In the Shortcuts app, turn on "Show in Share Sheet" for a shortcut that runs this
/// action and you can share any link from Safari straight into the Rust job queue.
struct FetchTitleIntent: AppIntent {
    static let title: LocalizedStringResource = "Fetch Page Title"
    static let description = IntentDescription("Queue a background job that fetches the page title of a link.")

    @Parameter(title: "Link")
    var link: URL

    func perform() async throws -> some IntentResult & ProvidesDialog {
        if let error = await RustCore.shared.start() {
            throw JobsClient.APIError(message: error)
        }
        let url = try await JobsClient.enqueue(link.absoluteString)
        return .result(dialog: "Queued \(url). You'll get a notification with the title.")
    }
}

struct LinkJobsShortcuts: AppShortcutsProvider {
    static var appShortcuts: [AppShortcut] {
        AppShortcut(
            intent: FetchTitleIntent(),
            phrases: ["Fetch a page title with \(.applicationName)"],
            shortTitle: "Fetch Page Title",
            systemImageName: "link"
        )
    }
}
