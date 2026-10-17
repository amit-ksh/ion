import Foundation
import Network

enum DocumentHostError: Error { case invalid, unavailable, request }
struct DocumentResource {
  let mediaType: String
  let bytes: Data
}

/// A single publication, a random loopback port and an unguessable path.
/// Never opens a file or proxies a request. All work runs on a private queue.
final class IonDocumentOrigin {
  static let csp = "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline' data:; img-src 'self' data:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"
  private let queue = DispatchQueue(label: "ion.document.origin", qos: .userInitiated)
  private let listener: NWListener
  private let token: String
  private let entry: String
  private var files: [String: DocumentResource]
  private var connections: [ObjectIdentifier: NWConnection] = [:]
  private var timer: DispatchSourceTimer?
  private var stopped = false
  private var deadline = DispatchTime.now().uptimeNanoseconds + 30_000_000_000
  private var port: UInt16 = 0

  static func path(_ path: String) -> Bool {
    !path.isEmpty && path.utf8.count <= 512 &&
      !path.unicodeScalars.contains { $0.value < 32 || (127...159).contains($0.value) || "\\:?#".unicodeScalars.contains($0) } &&
      !path.split(separator: "/", omittingEmptySubsequences: false).contains { $0.isEmpty || $0 == "." || $0 == ".." }
  }

  init(token: String, entry: String, files: [String: DocumentResource]) throws {
    guard UUID(uuidString: token)?.uuidString.lowercased() == token,
          Self.path(entry), (1...33).contains(files.count), files[entry]?.mediaType == "text/html" else { throw DocumentHostError.invalid }
    let allowed = Set(["text/css", "image/svg+xml", "image/png", "image/jpeg", "image/gif", "image/webp", "font/woff", "font/woff2", "font/ttf", "font/otf"])
    var extra = 0
    for (path, file) in files {
      let html = path == entry
      guard Self.path(path),
        html ? file.mediaType == "text/html" && !file.bytes.isEmpty && file.bytes.count <= 32 * 1024 * 1024 :
          allowed.contains(file.mediaType) && file.bytes.count <= 2 * 1024 * 1024 && (!file.bytes.isEmpty || file.mediaType == "text/css")
      else { throw DocumentHostError.invalid }
      if !html { extra += file.bytes.count }
    }
    guard extra <= 8 * 1024 * 1024 else { throw DocumentHostError.invalid }
    self.token = token; self.entry = entry; self.files = files
    let parameters = NWParameters.tcp
    parameters.requiredLocalEndpoint = .hostPort(host: .ipv4(.loopback), port: .any)
    self.listener = try NWListener(using: parameters)
  }

  /// The module calls this on its background work queue, not the origin queue.
  func start() throws -> String {
    let ready = DispatchSemaphore(value: 0)
    listener.stateUpdateHandler = { [weak self] state in
      guard let self else { return }
      switch state {
      case .ready:
        self.port = self.listener.port?.rawValue ?? 0
        ready.signal()
      case .failed, .cancelled:
        ready.signal()
      default: break
      }
    }
    listener.newConnectionHandler = { [weak self] connection in
      guard let self, !self.stopped, self.connections.count < 16 else { connection.cancel(); return }
      let key = ObjectIdentifier(connection)
      self.connections[key] = connection
      connection.start(queue: self.queue)
      self.receive(connection, input: Data())
      self.queue.asyncAfter(deadline: .now() + 3) { [weak self, weak connection] in
        guard let self, let connection else { return }
        self.finish(connection)
      }
    }
    listener.start(queue: queue)
    guard ready.wait(timeout: .now() + 3) == .success, port != 0 else { close(); throw DocumentHostError.unavailable }
    queue.async { [weak self] in
      guard let self, !self.stopped else { return }
      let timer = DispatchSource.makeTimerSource(queue: self.queue)
      timer.schedule(deadline: .now() + 1, repeating: 1)
      timer.setEventHandler { [weak self] in
        guard let self else { return }
        if DispatchTime.now().uptimeNanoseconds >= self.deadline { self.stop() }
      }
      self.timer = timer
      timer.resume()
    }
    let chars = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~!$&'()*+,;=@")
    let encoded = entry.split(separator: "/").map { String($0).addingPercentEncoding(withAllowedCharacters: chars)! }.joined(separator: "/")
    return "http://127.0.0.1:\(port)/\(token)/\(encoded)"
  }

  func alive() -> Bool { queue.sync { !stopped && DispatchTime.now().uptimeNanoseconds < deadline } }
  func renew() -> Bool {
    queue.sync {
      guard !stopped, DispatchTime.now().uptimeNanoseconds < deadline else { stop(); return false }
      deadline = DispatchTime.now().uptimeNanoseconds + 30_000_000_000
      return true
    }
  }
  func close() { queue.sync { stop() } }
  private func stop() {
    guard !stopped else { return }
    stopped = true
    listener.cancel()
    timer?.cancel(); timer = nil
    for connection in connections.values { connection.cancel() }
    connections.removeAll(); files.removeAll()
  }
  private func finish(_ connection: NWConnection) {
    connections.removeValue(forKey: ObjectIdentifier(connection))
    connection.cancel()
  }
  private func receive(_ connection: NWConnection, input: Data) {
    connection.receive(minimumIncompleteLength: 1, maximumLength: 4096) { [weak self] data, _, complete, error in
      guard let self, !self.stopped else { connection.cancel(); return }
      var bytes = input
      if let data { bytes.append(data) }
      guard bytes.count <= 8192 else { self.finish(connection); return }
      if bytes.range(of: Data([13, 10, 13, 10])) != nil {
        self.respond(connection, input: bytes)
      } else if complete || error != nil {
        self.finish(connection)
      } else {
        self.receive(connection, input: bytes)
      }
    }
  }
  private func respond(_ connection: NWConnection, input: Data) {
    guard !stopped, DispatchTime.now().uptimeNanoseconds < deadline,
          let text = String(data: input, encoding: .ascii),
          let end = text.range(of: "\r\n\r\n") else { finish(connection); return }
    let lines = text[..<end.lowerBound].components(separatedBy: "\r\n")
    let first = lines[0].components(separatedBy: " ")
    guard first.count == 3, ["HTTP/1.0", "HTTP/1.1"].contains(first[2]), lines.count <= 81 else { send(connection, status: 400); return }
    var headers: [String: String] = [:]
    for line in lines.dropFirst() {
      guard let colon = line.firstIndex(of: ":") else { send(connection, status: 400); return }
      let name = String(line[..<colon]).lowercased()
      guard !name.isEmpty, name.allSatisfy({ $0.isASCII && ($0.isLetter || $0.isNumber || $0 == "-") }), headers[name] == nil else { send(connection, status: 400); return }
      headers[name] = String(line[line.index(after: colon)...]).trimmingCharacters(in: .whitespaces)
    }
    let authority = "127.0.0.1:\(port)"
    guard ["GET", "HEAD"].contains(first[0]), headers["host"] == authority,
          headers["transfer-encoding"] == nil,
          headers["content-length"] == nil || headers["content-length"] == "0",
          headers["origin"] == nil || headers["origin"] == "http://" + authority,
          headers["sec-fetch-site"] == nil || ["none", "same-origin"].contains(headers["sec-fetch-site"]!)
    else { send(connection, status: 403); return }
    let target = first[1], prefix = "/" + token + "/"
    guard target.hasPrefix(prefix), !target.contains("?"), !target.contains("#"),
          target.range(of: "%(2f|5c|00)", options: [.regularExpression, .caseInsensitive]) == nil,
          let path = String(target.dropFirst(prefix.count)).removingPercentEncoding, Self.path(path),
          let file = files[path] else { send(connection, status: 404); return }
    send(connection, status: 200, file: file, head: first[0] == "HEAD")
  }
  private func send(_ connection: NWConnection, status: Int, file: DocumentResource? = nil, head: Bool = false) {
    guard !stopped else { finish(connection); return }
    let mime = file?.mediaType ?? "text/plain", bytes = file?.bytes ?? Data()
    let charset = ["text/html", "text/css", "image/svg+xml"].contains(mime) ? "; charset=utf-8" : ""
    let header = "HTTP/1.1 \(status) \(status == 200 ? "OK" : "Refused")\r\nContent-Type: \(mime)\(charset)\r\nContent-Length: \(bytes.count)\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: \(Self.csp)\r\n\r\n"
    connection.send(content: Data(header.utf8), completion: .contentProcessed { [weak self] error in
      guard let self, error == nil, !self.stopped else { connection.cancel(); return }
      if head || bytes.isEmpty { self.finish(connection); return }
      connection.send(content: bytes, completion: .contentProcessed { [weak self] _ in self?.finish(connection) })
    })
  }
}
