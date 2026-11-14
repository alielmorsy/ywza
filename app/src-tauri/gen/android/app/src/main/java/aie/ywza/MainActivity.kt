package dev.ywza.app

import android.content.Intent
import android.graphics.Color
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.view.View
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.SystemBarStyle
import androidx.activity.enableEdgeToEdge
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class MainActivity : TauriActivity() {
  private var web: WebView? = null
  private var pendingShare: String? = null
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge(SystemBarStyle.dark(Color.TRANSPARENT), SystemBarStyle.dark(Color.TRANSPARENT))
    super.onCreate(savedInstanceState)
    window.decorView.setBackgroundColor(Color.BLACK) // shows behind the bars
    // The WebView reports 0 for env(safe-area-inset-*), so keep the page clear of the system bars
    // and the keyboard here instead. Hidden bars report 0, so immersive mode gets the whole screen.
    val content = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { v, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout() or WindowInsetsCompat.Type.ime())
      v.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      WindowInsetsCompat.CONSUMED
    }
    if (savedInstanceState == null) readShare(intent)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    readShare(intent)
  }

  // Share-sheet input: streams are copied to cacheDir/shared (off the UI thread) so the app can push plain paths.
  // The page takes the result with `window.ywzaShare.take()` and hears `ywza-share` when one arrives while it runs.
  private fun readShare(i: Intent?) {
    if (i == null) return
    val action = i.action
    if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return
    val text = i.getStringExtra(Intent.EXTRA_TEXT)
    val subject = i.getStringExtra(Intent.EXTRA_SUBJECT)
    @Suppress("DEPRECATION")
    val uris: List<Uri> = if (action == Intent.ACTION_SEND_MULTIPLE) i.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM) ?: emptyList()
      else listOfNotNull(i.getParcelableExtra<Uri>(Intent.EXTRA_STREAM))
    setIntent(Intent()) // a rotate or resume must not resend it
    Thread {
      val dir = File(cacheDir, "shared")
      dir.deleteRecursively()
      dir.mkdirs()
      val files = JSONArray()
      for (uri in uris) {
        try {
          val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { if (it.moveToFirst()) it.getString(0) else null }
          val base = File(name ?: uri.lastPathSegment ?: "shared").name
          var dest = File(dir, base)
          var n = 1
          while (dest.exists()) dest = File(dir, "${n++}-$base")
          contentResolver.openInputStream(uri)?.use { input -> dest.outputStream().use { input.copyTo(it) } }
          files.put(dest.absolutePath)
        } catch (_: Exception) {}
      }
      if (files.length() == 0 && text.isNullOrBlank()) return@Thread
      val share = JSONObject().put("text", text).put("subject", subject).put("files", files).toString()
      synchronized(this@MainActivity) { pendingShare = share }
      runOnUiThread { web?.evaluateJavascript("window.dispatchEvent(new Event('ywza-share'))", null) }
    }.start()
  }

  // The WebView has no element fullscreen (no custom view support), so the live view asks for
  // immersive mode through `window.ywzaWindow.immersive(on)`.
  override fun onWebViewCreate(webView: WebView) {
    web = webView
    // Back closes what the page opened (it pushes a history entry, e.g. a full-size screenshot)
    // before it leaves the app. Leaving only backgrounds the task: finishing the activity makes the
    // event loop call process::exit while the WebView is still tearing down, which crashes natively.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        if (webView.canGoBack()) webView.goBack() else moveTaskToBack(true)
      }
    })
    webView.addJavascriptInterface(object {
      @JavascriptInterface
      fun immersive(on: Boolean) = runOnUiThread {
        val c = WindowCompat.getInsetsController(window, window.decorView)
        if (on) {
          c.systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
          c.hide(WindowInsetsCompat.Type.systemBars())
        } else c.show(WindowInsetsCompat.Type.systemBars())
      }
    }, "ywzaWindow")
    webView.addJavascriptInterface(object {
      @JavascriptInterface
      fun take(): String? = synchronized(this@MainActivity) { pendingShare.also { pendingShare = null } }
    }, "ywzaShare")
  }
}
