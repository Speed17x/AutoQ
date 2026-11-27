package com.speed17x.trivium.ui

import java.util.Locale

/**
 * Lightweight automatic EN / ES localization matching Android's active system Locale.
 */
object UiLang {
    fun isSpanish(): Boolean =
        Locale.getDefault().language.equals("es", ignoreCase = true)

    fun tr(es: String, en: String): String =
        if (isSpanish()) es else en
}
