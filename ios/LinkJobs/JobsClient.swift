import Foundation

/// Mirrors `linkjobs_core::db::JobView`.
struct Job: Decodable, Identifiable, Equatable {
    struct PageInfo: Decodable, Equatable {
        let url: String
        let final_url: String
        let http_status: Int
        let title: String?
        let bytes_read: Int
    }

    let id: String
    let url: String
    let status: String
    let attempts: Int
    let created_at: Int
    let done_at: Int?
    let result: PageInfo?
    let error: String?

    var isFinished: Bool { ["Done", "Failed", "Killed"].contains(status) }
    var displayTitle: String { result?.title ?? URL(string: url)?.host() ?? url }
}

struct Stats: Decodable, Equatable {
    var pending = 0, running = 0, done = 0, failed = 0
}

/// Talks to the Rust job API on 127.0.0.1.
enum JobsClient {
    struct APIError: LocalizedError {
        let message: String
        var errorDescription: String? { message }
    }

    private struct ErrorBody: Decodable { let description: String }

    static func enqueue(_ text: String) async throws -> String {
        var request = URLRequest(url: RustCore.baseURL.appendingPathComponent("jobs"))
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")
        request.httpBody = try JSONEncoder().encode(["text": text])
        let (data, response) = try await URLSession.shared.data(for: request)
        try check(data, response)
        struct Queued: Decodable { let url: String }
        return try JSONDecoder().decode(Queued.self, from: data).url
    }

    static func jobs() async throws -> [Job] {
        try await get("jobs")
    }

    static func stats() async throws -> Stats {
        try await get("stats")
    }

    static func clearFinished() async throws {
        var request = URLRequest(url: RustCore.baseURL.appendingPathComponent("jobs"))
        request.httpMethod = "DELETE"
        let (data, response) = try await URLSession.shared.data(for: request)
        try check(data, response)
    }

    private static func get<T: Decodable>(_ path: String) async throws -> T {
        let (data, response) = try await URLSession.shared.data(from: RustCore.baseURL.appendingPathComponent(path))
        try check(data, response)
        return try JSONDecoder().decode(T.self, from: data)
    }

    private static func check(_ data: Data, _ response: URLResponse) throws {
        guard let http = response as? HTTPURLResponse, !(200..<300).contains(http.statusCode) else { return }
        let message = (try? JSONDecoder().decode(ErrorBody.self, from: data))?.description
        throw APIError(message: message ?? "HTTP \(http.statusCode)")
    }
}
