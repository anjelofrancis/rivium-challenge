import SwiftUI

struct ContentView: View {
    @EnvironmentObject private var store: JobsStore
    @State private var text = ""

    var body: some View {
        NavigationStack {
            List {
                Section {
                    HStack {
                        TextField("Paste or type a link", text: $text)
                            .keyboardType(.URL)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .onSubmit(submit)
                        PasteButton(payloadType: String.self) { strings in
                            if let first = strings.first {
                                Task { await store.enqueue(first) }
                            }
                        }
                        .labelStyle(.iconOnly)
                        .buttonBorderShape(.capsule)
                    }
                    Button("Fetch title", systemImage: "arrow.down.doc", action: submit)
                        .disabled(text.trimmingCharacters(in: .whitespaces).isEmpty)
                } footer: {
                    if let message = store.message { Text(message) }
                }

                Section {
                    StatsRow(stats: store.stats)
                } header: {
                    Text("Rust core: \(store.coreStatus)")
                }

                Section("Jobs") {
                    if store.jobs.isEmpty {
                        Text("No jobs yet. Share a link from Safari via the LinkJobs shortcut, or paste one above.")
                            .foregroundStyle(.secondary)
                    }
                    ForEach(store.jobs) { job in
                        JobRow(job: job)
                    }
                }
            }
            .navigationTitle("LinkJobs")
            .toolbar {
                Button("Clear finished", systemImage: "trash") {
                    Task { await store.clearFinished() }
                }
            }
            .refreshable { await store.refresh() }
        }
    }

    private func submit() {
        let value = text
        text = ""
        Task { await store.enqueue(value) }
    }
}

private struct StatsRow: View {
    let stats: Stats

    var body: some View {
        HStack {
            stat("Waiting", stats.pending, .orange)
            stat("Running", stats.running, .blue)
            stat("Done", stats.done, .green)
            stat("Failed", stats.failed, .red)
        }
    }

    private func stat(_ label: String, _ value: Int, _ color: Color) -> some View {
        VStack {
            Text("\(value)").font(.title2.monospacedDigit().bold()).foregroundStyle(color)
            Text(label).font(.caption).foregroundStyle(.secondary)
        }
        .frame(maxWidth: .infinity)
    }
}

private struct JobRow: View {
    let job: Job

    var body: some View {
        HStack(alignment: .top, spacing: 12) {
            icon.frame(width: 22)
            VStack(alignment: .leading, spacing: 2) {
                Text(job.displayTitle).font(.headline).lineLimit(2)
                Text(job.url).font(.caption).foregroundStyle(.secondary).lineLimit(1)
                if let error = job.error, job.status != "Done" {
                    Text(error).font(.caption2).foregroundStyle(.red).lineLimit(2)
                }
            }
        }
        .contextMenu {
            if let url = URL(string: job.result?.final_url ?? job.url) {
                Link("Open in Safari", destination: url)
            }
        }
    }

    @ViewBuilder private var icon: some View {
        switch job.status {
        case "Done": Image(systemName: "checkmark.circle.fill").foregroundStyle(.green)
        case "Failed", "Killed": Image(systemName: "xmark.octagon.fill").foregroundStyle(.red)
        case "Running", "Queued": ProgressView()
        default: Image(systemName: "clock").foregroundStyle(.orange)
        }
    }
}
