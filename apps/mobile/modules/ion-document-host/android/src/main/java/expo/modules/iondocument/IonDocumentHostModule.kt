package expo.modules.iondocument

import android.util.Base64
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class IonDocumentHostModule : Module() {
  private val guard = Any()
  private val hosts = mutableMapOf<String, IonDocumentHost>()
  private var foreground = true
  private var epoch = 0L

  private fun shutdown() = synchronized(guard) {
    foreground = false
    epoch++
    hosts.values.forEach { it.close() }
    hosts.clear()
  }

  override fun definition() = ModuleDefinition {
    Name("IonDocumentHost")
    Constants("sheetSystemBarsSupported" to true)
    View(IonSheetSystemBarsView::class) {
      Prop("dark") { view: IonSheetSystemBarsView, dark: Boolean -> view.setDark(dark) }
    }
    AsyncFunction("open") { token: String, entry: String, files: List<Map<String, String>> ->
      val ticket = synchronized(guard) {
        hosts.entries.removeAll { if (!it.value.alive()) { it.value.close(); true } else false }
        check(foreground && hosts.size < 2 && !hosts.containsKey(token)) { "Document host unavailable" }
        epoch
      }
      require(files.size in 1..33) { "Invalid document file set" }
      val resources = mutableMapOf<String, IonDocumentHost.Resource>()
      var encoded = 0L
      files.forEach { file ->
        require(file.keys == setOf("path", "mediaType", "data")) { "Invalid resource fields" }
        val data = file.getValue("data")
        encoded += data.length
        require(encoded <= 56L * 1024 * 1024 && data.length % 4 == 0 && data.matches(Regex("[A-Za-z0-9+/]*={0,2}"))) { "Invalid resource bytes" }
        require(resources.put(file.getValue("path"), IonDocumentHost.Resource(file.getValue("mediaType"), Base64.decode(data, Base64.NO_WRAP))) == null) { "Duplicate document resource" }
      }
      val host = IonDocumentHost(token, entry, resources)
      synchronized(guard) {
        if (!foreground || epoch != ticket || hosts.size >= 2 || hosts.containsKey(token)) {
          host.close()
          error("Document host scope changed")
        }
        hosts[token] = host
      }
      mapOf("token" to token, "url" to host.url())
    }
    AsyncFunction("renew") { token: String ->
      synchronized(guard) {
        val host = hosts[token]
        val alive = foreground && host?.renew() == true
        if (!alive) hosts.remove(token)?.close()
        alive
      }
    }
    AsyncFunction("close") { token: String -> synchronized(guard) { hosts.remove(token)?.close() }; Unit }
    OnActivityEntersForeground { synchronized(guard) { foreground = true } }
    OnActivityEntersBackground { shutdown() }
    OnDestroy { shutdown() }
  }
}
