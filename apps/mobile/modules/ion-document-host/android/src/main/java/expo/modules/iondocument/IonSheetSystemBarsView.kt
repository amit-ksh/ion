package expo.modules.iondocument

import android.content.Context
import android.os.Build
import android.view.View
import android.view.WindowInsetsController
import expo.modules.kotlin.AppContext
import expo.modules.kotlin.views.ExpoView

/** Applies contrast to the containing Material dialog, rather than the activity behind it. */
class IonSheetSystemBarsView(context: Context, appContext: AppContext) : ExpoView(context, appContext) {
  private var dark = false

  fun setDark(value: Boolean) {
    dark = value
    refreshBars()
  }

  override fun onAttachedToWindow() {
    super.onAttachedToWindow()
    refreshBars()
  }

  override fun onWindowFocusChanged(hasWindowFocus: Boolean) {
    super.onWindowFocusChanged(hasWindowFocus)
    if (hasWindowFocus) refreshBars()
  }

  private fun refreshBars() {
    post {
      if (isAttachedToWindow) {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
          windowInsetsController?.setSystemBarsAppearance(
            if (dark) 0 else WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS,
            WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
          )
        } else {
          @Suppress("DEPRECATION")
          rootView.systemUiVisibility = if (dark) {
            rootView.systemUiVisibility and View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR.inv()
          } else {
            rootView.systemUiVisibility or View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR
          }
        }
      }
    }
  }
}
