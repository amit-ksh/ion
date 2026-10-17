import ExpoModulesCore
import Foundation

struct IonDocumentFile: Record {
  @Field var path: String = ""
  @Field var mediaType: String = ""
  @Field var data: String = ""
}

public class IonDocumentHostModule: Module {
  private let guardLock = NSLock()
  private let work = DispatchQueue(label: "ion.document.module", qos: .userInitiated)
  private var hosts: [String: IonDocumentOrigin] = [:]
  private var foreground = true
  private var epoch = 0

  private func shutdown() {
    guardLock.lock()
    foreground = false; epoch += 1
    let active = Array(hosts.values); hosts.removeAll()
    guardLock.unlock()
    for host in active { host.close() }
  }
  public func definition() -> ModuleDefinition {
    Name("IonDocumentHost")
    AsyncFunction("open") { (token: String, entry: String, files: [IonDocumentFile]) throws -> [String: String] in
      self.guardLock.lock()
      let expired = self.hosts.filter { !$0.value.alive() }
      for (id, host) in expired { host.close(); self.hosts.removeValue(forKey: id) }
      let ticket = self.epoch
      let allowed = self.foreground && self.hosts.count < 2 && self.hosts[token] == nil
      self.guardLock.unlock()
      guard allowed, (1...33).contains(files.count) else { throw DocumentHostError.unavailable }
      var resources: [String: DocumentResource] = [:], encoded = 0
      for file in files {
        encoded += file.data.utf8.count
        guard encoded <= 56 * 1024 * 1024, resources[file.path] == nil,
              let bytes = Data(base64Encoded: file.data) else { throw DocumentHostError.invalid }
        resources[file.path] = DocumentResource(mediaType: file.mediaType, bytes: bytes)
      }
      let host = try IonDocumentOrigin(token: token, entry: entry, files: resources)
      let url = try host.start()
      self.guardLock.lock()
      guard self.foreground, self.epoch == ticket, self.hosts.count < 2, self.hosts[token] == nil else {
        self.guardLock.unlock(); host.close(); throw DocumentHostError.unavailable
      }
      self.hosts[token] = host
      self.guardLock.unlock()
      return ["token": token, "url": url]
    }.runOnQueue(work)
    AsyncFunction("renew") { (token: String) -> Bool in
      self.guardLock.lock()
      let host = self.foreground ? self.hosts[token] : nil
      self.guardLock.unlock()
      let alive = host?.renew() ?? false
      if !alive {
        self.guardLock.lock()
        self.hosts.removeValue(forKey: token)
        self.guardLock.unlock()
        host?.close()
      }
      return alive
    }.runOnQueue(work)
    AsyncFunction("close") { (token: String) in
      self.guardLock.lock()
      let host = self.hosts.removeValue(forKey: token)
      self.guardLock.unlock()
      host?.close()
    }.runOnQueue(work)
    OnAppEntersForeground {
      self.guardLock.lock(); self.foreground = true; self.guardLock.unlock()
    }
    OnAppEntersBackground { self.shutdown() }
    OnDestroy { self.shutdown() }
  }
}
