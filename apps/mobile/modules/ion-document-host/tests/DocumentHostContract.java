package expo.modules.iondocument;
import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.*;

public final class DocumentHostContract {
  private static int checks;
  private static void check(boolean value, String label) { checks++; if (!value) throw new AssertionError(label); }
  private static String request(URL url, String method, String path, String headers) throws Exception {
    try (Socket socket = new Socket("127.0.0.1", url.getPort())) {
      socket.setSoTimeout(4000);
      String wire = method + " " + path + " HTTP/1.1\r\n" + headers + "\r\n";
      socket.getOutputStream().write(wire.getBytes(StandardCharsets.US_ASCII));
      ByteArrayOutputStream bytes = new ByteArrayOutputStream();
      byte[] buffer = new byte[8192]; int n;
      while ((n = socket.getInputStream().read(buffer)) >= 0) bytes.write(buffer, 0, n);
      return new String(bytes.toByteArray(), StandardCharsets.UTF_8);
    }
  }
  private static Map<String, IonDocumentHost.Resource> resources() {
    Map<String, IonDocumentHost.Resource> map = new HashMap<>();
    map.put("docs/経緯🚀.html", new IonDocumentHost.Resource("text/html", "<link rel=stylesheet href='../theme.css'><svg><use href='../icons.svg#mark'/></svg>".getBytes(StandardCharsets.UTF_8)));
    map.put("theme.css", new IonDocumentHost.Resource("text/css", "@property --name {syntax:'*';inherits:true;initial-value:demo;} @keyframes demo{} svg{--name:demo}".getBytes(StandardCharsets.UTF_8)));
    map.put("empty.css", new IonDocumentHost.Resource("text/css", new byte[0]));
    map.put("icons.svg", new IonDocumentHost.Resource("image/svg+xml", "<svg xmlns='http://www.w3.org/2000/svg'><symbol id='mark'><path d='M0 0h10'/></symbol></svg>".getBytes(StandardCharsets.UTF_8)));
    map.put("fonts/demo.woff2", new IonDocumentHost.Resource("font/woff2", new byte[]{0, 1, 2, 3, 4}));
    map.put("literal+percent%2f.css", new IonDocumentHost.Resource("text/css", new byte[0]));
    return map;
  }
  public static void main(String[] args) throws Exception {
    String token = UUID.randomUUID().toString();
    IonDocumentHost host = new IonDocumentHost(token, "docs/経緯🚀.html", resources());
    URL url = new URL(host.url());
    String headers = "Host: 127.0.0.1:" + url.getPort() + "\r\n";
    String root = "/" + token + "/";
    try {
      check(url.getHost().equals("127.0.0.1"), "loopback only");
      String html = request(url, "GET", url.getFile(), headers);
      check(html.startsWith("HTTP/1.1 200"), "Unicode entry path");
      check(html.contains("<link rel=stylesheet href='../theme.css'>"), "authored relative CSS preserved");
      check(html.contains("Content-Security-Policy: " + IonDocumentHost.CSP), "response CSP");
      check(html.contains("Cache-Control: no-store"), "no cache");
      check(html.contains("X-Content-Type-Options: nosniff"), "no MIME sniffing");
      check(!html.contains("Set-Cookie") && !html.contains("Access-Control-Allow-Origin"), "no cookies or wildcard CORS");
      String css = request(url, "GET", root + "theme.css", headers + "Sec-Fetch-Site: same-origin\r\n");
      check(css.startsWith("HTTP/1.1 200") && css.endsWith("svg{--name:demo}"), "CSS remains native");
      check(css.contains("Content-Type: text/css; charset=utf-8"), "CSS MIME");
      String svg = request(url, "GET", root + "icons.svg", headers);
      check(svg.contains("Content-Type: image/svg+xml; charset=utf-8") && svg.endsWith("</svg>"), "separate SVG document");
      String font = request(url, "GET", root + "fonts/demo.woff2", headers + "Origin: http://127.0.0.1:" + url.getPort() + "\r\n");
      check(font.startsWith("HTTP/1.1 200") && font.endsWith(new String(new byte[]{0, 1, 2, 3, 4}, StandardCharsets.UTF_8)), "binary font");
      check(request(url, "GET", root + "empty.css", headers).contains("Content-Length: 0"), "empty CSS");
      check(request(url, "GET", root + "literal+percent%252f.css", headers).startsWith("HTTP/1.1 200"), "literal plus and percent");
      check(request(url, "HEAD", root + "theme.css", headers).endsWith("\r\n\r\n"), "HEAD has no body");
      for (String path : Arrays.asList("/" + UUID.randomUUID() + "/theme.css", root + "missing", root + "../theme.css", root + "%2e%2e/theme.css", root + "theme.css?q=x", root + "%2ftheme.css", root + "theme.css%00", root + "fonts%5cdemo.woff2", "https://outside.invalid/theme.css", "/etc/passwd")) {
        check(request(url, "GET", path, headers).startsWith("HTTP/1.1 404"), "refuse path " + path);
      }
      for (String bad : Arrays.asList("Host: outside.invalid\r\n", headers + headers, headers + "Origin: https://outside.invalid\r\n", headers + "Sec-Fetch-Site: cross-site\r\n", headers + "Transfer-Encoding: chunked\r\n", headers + "Content-Length: 1\r\n")) {
        String reply = request(url, "GET", root + "theme.css", bad);
        check(!reply.startsWith("HTTP/1.1 200"), "refuse authority/body");
      }
      check(request(url, "POST", root + "theme.css", headers).startsWith("HTTP/1.1 403"), "read-only");
      check(host.renew(), "active lease renews");
      // Teardown cancels an incomplete socket too.
      Socket pending = new Socket("127.0.0.1", url.getPort());
      pending.getOutputStream().write("GET /".getBytes(StandardCharsets.US_ASCII));
      host.close(); pending.setSoTimeout(2000);
      try { check(pending.getInputStream().read() == -1, "pending socket closes"); }
      catch (SocketException closed) { check(true, "pending socket reset"); }
      pending.close();
      check(!host.renew(), "closed lease cannot revive");
      try { new Socket("127.0.0.1", url.getPort()).close(); throw new AssertionError("listener remained open"); }
      catch (ConnectException expected) { check(true, "listener closed"); }
    } finally { host.close(); }
    IonDocumentHost expired = new IonDocumentHost(UUID.randomUUID().toString(), "docs/経緯🚀.html", resources(), 40);
    Thread.sleep(100);
    check(!expired.renew(), "expired lease cannot revive"); expired.close();
    for (String bad : Arrays.asList("../x.html", "/x.html", "a//b", "a/./b", "C:/x", "a\\b", "a?b", "a#b", "a\u0000b")) check(!IonDocumentHost.path(bad), "invalid path");
    Map<String, IonDocumentHost.Resource> bad = resources();
    bad.put("extra.html", new IonDocumentHost.Resource("text/html", new byte[]{1}));
    try { new IonDocumentHost(UUID.randomUUID().toString(), "docs/経緯🚀.html", bad); throw new AssertionError("second HTML allowed"); }
    catch (IllegalArgumentException expected) { check(true, "single entry HTML"); }
    System.out.println(checks + " real HTTP/lifecycle assertions passed");
  }
}
