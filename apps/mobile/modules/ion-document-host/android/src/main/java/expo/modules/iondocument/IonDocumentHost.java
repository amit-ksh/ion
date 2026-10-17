package expo.modules.iondocument;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;
import java.util.concurrent.*;
import java.util.concurrent.atomic.AtomicBoolean;

/** An ephemeral HTTP origin containing only a verified publication's supplied
 * bytes. No filesystem, DNS, upstream proxy, cookie store or app route exists.
 * Runs off the UI thread; close/expiry closes both the listener and live sockets. */
public final class IonDocumentHost implements AutoCloseable {
  public static final String CSP = "default-src 'none'; script-src 'none'; style-src 'self' 'unsafe-inline' data:; img-src 'self' data:; font-src 'self' data:; connect-src 'none'; frame-src 'none'; child-src 'none'; worker-src 'none'; object-src 'none'; media-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
  public static final class Resource {
    final String mediaType;
    final byte[] bytes;
    public Resource(String mediaType, byte[] bytes) { this.mediaType = mediaType; this.bytes = bytes; }
  }
  private volatile Map<String, Resource> resources;
  private final String token, entry;
  private final ServerSocket server;
  private final ExecutorService acceptor = Executors.newSingleThreadExecutor();
  private final ThreadPoolExecutor workers = new ThreadPoolExecutor(4, 4, 0, TimeUnit.SECONDS, new ArrayBlockingQueue<Runnable>(16));
  private final ScheduledExecutorService clock = Executors.newSingleThreadScheduledExecutor();
  private final Set<Socket> sockets = ConcurrentHashMap.newKeySet();
  private final AtomicBoolean closed = new AtomicBoolean(false);
  private final long leaseNanos;
  private volatile long deadline;
  public IonDocumentHost(String token, String entry, Map<String, Resource> resources) throws IOException {
    this(token, entry, resources, 30_000);
  }
  IonDocumentHost(String token, String entry, Map<String, Resource> resources, long leaseMillis) throws IOException {
    if (!token.matches("[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}")) throw new IllegalArgumentException("Invalid host identity");
    validate(entry, resources);
    this.token = token; this.entry = entry;
    this.resources = Collections.unmodifiableMap(new HashMap<>(resources));
    this.leaseNanos = TimeUnit.MILLISECONDS.toNanos(leaseMillis);
    this.deadline = System.nanoTime() + leaseNanos;
    this.server = new ServerSocket(0, 16, InetAddress.getByName("127.0.0.1"));
    acceptor.execute(this::accept);
    clock.scheduleAtFixedRate(() -> { if (System.nanoTime() >= deadline) close(); }, Math.min(leaseMillis, 1000), Math.min(leaseMillis, 1000), TimeUnit.MILLISECONDS);
  }
  public static boolean path(String path) {
    if (path == null || path.isEmpty() || path.getBytes(StandardCharsets.UTF_8).length > 512) return false;
    for (int i = 0; i < path.length(); i++) {
      char c = path.charAt(i);
      if (c < 32 || (c >= 127 && c <= 159) || "\\:?#".indexOf(c) >= 0) return false;
      if (Character.isHighSurrogate(c)) { if (i + 1 >= path.length() || !Character.isLowSurrogate(path.charAt(++i))) return false; }
      else if (Character.isLowSurrogate(c)) return false;
    }
    for (String part : path.split("/", -1)) if (part.isEmpty() || part.equals(".") || part.equals("..")) return false;
    return true;
  }
  private static void validate(String entry, Map<String, Resource> files) {
    if (!path(entry) || files.isEmpty() || files.size() > 33) throw new IllegalArgumentException("Invalid document file set");
    long extra = 0;
    Set<String> types = new HashSet<>(Arrays.asList("text/css", "image/svg+xml", "image/png", "image/jpeg", "image/gif", "image/webp", "font/woff", "font/woff2", "font/ttf", "font/otf"));
    for (Map.Entry<String, Resource> file : files.entrySet()) {
      Resource r = file.getValue();
      boolean html = file.getKey().equals(entry);
      if (!path(file.getKey()) || r == null || r.bytes == null ||
          (html ? !"text/html".equals(r.mediaType) || r.bytes.length == 0 || r.bytes.length > 32 * 1024 * 1024 :
          !types.contains(r.mediaType) || r.bytes.length > 2 * 1024 * 1024 || (r.bytes.length == 0 && !"text/css".equals(r.mediaType)))) throw new IllegalArgumentException("Invalid document resource");
      if (!html) extra += r.bytes.length;
    }
    if (!files.containsKey(entry) || extra > 8 * 1024 * 1024) throw new IllegalArgumentException("Document resources exceed bounds");
  }
  public String url() {
    StringBuilder path = new StringBuilder();
    for (String part : entry.split("/")) {
      try { path.append('/').append(URLEncoder.encode(part, "UTF-8").replace("+", "%20")); }
      catch (UnsupportedEncodingException impossible) { throw new AssertionError(impossible); }
    }
    return "http://127.0.0.1:" + server.getLocalPort() + "/" + token + path;
  }
  public boolean alive() { return !closed.get() && System.nanoTime() < deadline; }
  public boolean renew() {
    if (closed.get() || System.nanoTime() >= deadline) { close(); return false; }
    deadline = System.nanoTime() + leaseNanos;
    return true;
  }
  private void accept() {
    while (!closed.get()) {
      Socket socket = null;
      try {
        socket = server.accept();
        if (closed.get() || sockets.size() >= 16) { socket.close(); continue; }
        socket.setSoTimeout(3000); socket.setTcpNoDelay(true); sockets.add(socket);
        final Socket owned = socket;
        try { workers.execute(() -> serve(owned)); }
        catch (RejectedExecutionException full) { sockets.remove(owned); owned.close(); }
      } catch (IOException failure) {
        if (socket != null) { sockets.remove(socket); try { socket.close(); } catch (IOException ignored) {} }
        if (!closed.get()) close();
      }
    }
  }
  private void serve(Socket socket) {
    try (Socket owned = socket) {
      ByteArrayOutputStream input = new ByteArrayOutputStream();
      InputStream reader = new BufferedInputStream(owned.getInputStream(), 8192);
      int state = 0;
      while (input.size() < 8192 && state != 4) {
        int b = reader.read();
        if (b < 0 || b > 127) return;
        input.write(b);
        state = b == (state == 0 || state == 2 ? 13 : 10) ? state + 1 : (b == 13 ? 1 : 0);
      }
      if (state != 4 || closed.get() || System.nanoTime() >= deadline) return;
      String[] lines = input.toString("US-ASCII").split("\r\n");
      String[] first = lines[0].split(" ", -1);
      if (first.length != 3 || !(first[2].equals("HTTP/1.1") || first[2].equals("HTTP/1.0"))) { reply(owned, 400, null, false); return; }
      Map<String, String> headers = new HashMap<>();
      for (int i = 1; i < lines.length; i++) {
        int at = lines[i].indexOf(':');
        if (at <= 0 || i > 80) { reply(owned, 400, null, false); return; }
        String name = lines[i].substring(0, at).toLowerCase(Locale.ROOT), value = lines[i].substring(at + 1).trim();
        if (!name.matches("[a-z0-9-]+") || headers.put(name, value) != null) { reply(owned, 400, null, false); return; }
      }
      if (!(first[0].equals("GET") || first[0].equals("HEAD")) ||
          !("127.0.0.1:" + server.getLocalPort()).equals(headers.get("host")) ||
          headers.containsKey("transfer-encoding") ||
          (headers.containsKey("content-length") && !headers.get("content-length").equals("0")) ||
          (headers.containsKey("origin") && !headers.get("origin").equals("http://127.0.0.1:" + server.getLocalPort())) ||
          (headers.containsKey("sec-fetch-site") && !Arrays.asList("none", "same-origin").contains(headers.get("sec-fetch-site")))) { reply(owned, 403, null, false); return; }
      String target = first[1], prefix = "/" + token + "/";
      if (!target.startsWith(prefix) || target.contains("?") || target.contains("#") || target.toLowerCase(Locale.ROOT).matches(".*%(2f|5c|00).*")) { reply(owned, 404, null, false); return; }
      String path;
      try { path = URLDecoder.decode(target.substring(prefix.length()).replace("+", "%2B"), "UTF-8"); }
      catch (IllegalArgumentException malformed) { reply(owned, 404, null, false); return; }
      Resource resource = path(path) ? resources.get(path) : null;
      reply(owned, resource == null ? 404 : 200, resource, first[0].equals("HEAD"));
    } catch (IOException ignored) {
      // The owner, lease, background lifecycle or renderer may cancel a socket.
    } finally { sockets.remove(socket); }
  }
  private void reply(Socket socket, int status, Resource resource, boolean head) throws IOException {
    if (closed.get() || System.nanoTime() >= deadline) return;
    byte[] bytes = resource == null ? new byte[0] : resource.bytes;
    String type = resource == null ? "text/plain" : resource.mediaType;
    String headers = "HTTP/1.1 " + status + (status == 200 ? " OK" : " Refused") +
      "\r\nContent-Type: " + type + ((type.equals("text/html") || type.equals("text/css") || type.equals("image/svg+xml")) ? "; charset=utf-8" : "") +
      "\r\nContent-Length: " + bytes.length +
      "\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: " + CSP + "\r\n\r\n";
    OutputStream out = new BufferedOutputStream(socket.getOutputStream(), 8192);
    out.write(headers.getBytes(StandardCharsets.US_ASCII));
    if (!head) out.write(bytes);
    out.flush();
  }
  @Override public void close() {
    if (!closed.compareAndSet(false, true)) return;
    resources = Collections.emptyMap();
    try { server.close(); } catch (IOException ignored) {}
    for (Socket socket : sockets) try { socket.close(); } catch (IOException ignored) {}
    sockets.clear(); acceptor.shutdownNow(); workers.shutdownNow(); clock.shutdownNow();
  }
}
