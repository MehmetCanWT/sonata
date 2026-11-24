package dev.sonata.android.discord

/**
 * Shared Discord application constants, activity types, and identifier helpers
 * for Sonata's mobile Discord Rich Presence integration.
 */
object DiscordApplication {
    const val DEFAULT_APP_ID = "1520387624861438002"
    const val DEFAULT_APP_NAME = "Sonata"
    const val DEFAULT_BUTTON_LABEL = "Listen on Sonata"
    const val PROJECT_URL = "https://github.com/MehmetCanWT/sonata"
    const val GATEWAY_URL = "wss://gateway.discord.gg/?v=10&encoding=json"
    const val API_BASE_URL = "https://discord.com/api/v9"

    private const val MIN_SNOWFLAKE_LEN = 15
    private const val MAX_SNOWFLAKE_LEN = 22

    /**
     * Validates whether [raw] looks like a Discord numeric snowflake Application ID.
     */
    fun isValidSnowflakeAppId(raw: String): Boolean {
        val trimmed = raw.trim()
        return trimmed.length in MIN_SNOWFLAKE_LEN..MAX_SNOWFLAKE_LEN && trimmed.all { it.isDigit() }
    }

    /**
     * Returns [overrideAppId] when it is a valid Discord snowflake, otherwise [DEFAULT_APP_ID].
     */
    fun resolveAppId(overrideAppId: String?): String {
        val trimmed = overrideAppId?.trim().orEmpty()
        return if (isValidSnowflakeAppId(trimmed)) trimmed else DEFAULT_APP_ID
    }

    /**
     * Returns the Discord REST endpoint used to proxy external track artwork URLs.
     */
    fun externalAssetsUrl(appId: String): String =
        "$API_BASE_URL/applications/${resolveAppId(appId)}/external-assets"

    /**
     * Normalizes an `external_asset_path` returned by `/external-assets` for Gateway `large_image`.
     */
    fun normalizeExternalAssetImage(externalAssetPath: String): String {
        val trimmed = externalAssetPath.trim()
        if (trimmed.isEmpty()) return ""
        return if (trimmed.startsWith("mp:")) trimmed else "mp:$trimmed"
    }

    /**
     * Redacts any sensitive token material before emitting diagnostics or UI errors.
     */
    fun redactSensitiveText(message: String, token: String? = null): String {
        var sanitized = message
        val trimmedToken = token?.trim().orEmpty()
        if (trimmedToken.length >= 8) {
            sanitized = sanitized.replace(trimmedToken, "[REDACTED]")
        }
        return sanitized
            .replace(Regex("""(Authorization:\s*)([^\r\n]+)""", RegexOption.IGNORE_CASE), "$1[REDACTED]")
            .replace(Regex("""("token"\s*:\s*")([^"]+)(")""", RegexOption.IGNORE_CASE), "$1[REDACTED]$3")
    }
}

/**
 * Discord Gateway activity type values supported for Rich Presence.
 */
enum class DiscordActivityType(
    val gatewayType: Int,
) {
    LISTENING(2),
    PLAYING(0),
    WATCHING(3),
    COMPETING(5),
}
