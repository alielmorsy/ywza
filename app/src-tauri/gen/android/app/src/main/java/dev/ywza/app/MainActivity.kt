package dev.ywza.app

import android.graphics.Color
import android.os.Bundle
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

class MainActivity : TauriActivity() {
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
  }

  // The WebView has no element fullscreen (no custom view support), so the live view asks for
  // immersive mode through `window.ywzaWindow.immersive(on)`.
  override fun onWebViewCreate(webView: WebView) {
    // Back closes what the page opened (it pushes a history entry, e.g. a full-size screenshot)
    // before it leaves the app.
    onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        if (webView.canGoBack()) return webView.goBack()
        isEnabled = false
        onBackPressedDispatcher.onBackPressed()
        isEnabled = true
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
  }
}
