package dev.sonata.android.discord

/**
 * Shared Discord application constants, activity types, and identifier helpers
 * for Sonata's mobile Discord Rich Presence integration.
 */
object DiscordApplication {
    /**
     * Immutable Sonata Discord Application ID (cannot be overridden by user preferences).
     */
    const val DEFAULT_APP_ID = "1520387624861438002"
    const val DEFAULT_APP_NAME = "Sonata"
    const val DEFAULT_BUTTON_LABEL = "Listen on Sonata"
    const val PROJECT_URL = "https://github.com/MehmetCanWT/sonata"
    const val GATEWAY_URL = "wss://gateway.discord.gg/?v=10&encoding=json"
    const val API_BASE_URL = "https://discord.com/api/v9"
    const val CURRENT_USER_URL = "$API_BASE_URL/users/@me"
    const val GATEWAY_USER_AGENT =
        "Mozilla/5.0 (Linux; Android 14; Mobile) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36"

    private const val MIN_SNOWFLAKE_LEN = 15
    private const val MAX_SNOWFLAKE_LEN = 22
    private const val MIN_TOKEN_LEN = 20

    /**
     * Validates whether [raw] looks like a Discord numeric snowflake Application ID.
     */
    fun isValidSnowflakeAppId(raw: String): Boolean {
        val trimmed = raw.trim()
        return trimmed.length in MIN_SNOWFLAKE_LEN..MAX_SNOWFLAKE_LEN && trimmed.all { it.isDigit() }
    }

    /**
     * Always returns Sonata's immutable [DEFAULT_APP_ID], ignoring any custom override.
     */
    @Suppress("UNUSED_PARAMETER")
    fun resolveAppId(overrideAppId: String? = null): String = DEFAULT_APP_ID

    /**
     * Returns the Discord REST endpoint used to proxy external track artwork URLs.
     */
    @Suppress("UNUSED_PARAMETER")
    fun externalAssetsUrl(appId: String = DEFAULT_APP_ID): String =
        "$API_BASE_URL/applications/$DEFAULT_APP_ID/external-assets"

    /**
     * Normalizes a Discord user token by trimming whitespace, unwrapping JSON/string quotes,
     * and rejecting empty, placeholder, bot/bearer-prefixed, or whitespace-containing values.
     */
    fun sanitizeToken(raw: String?): String? {
        if (raw.isNullOrBlank()) return null
        var candidate = raw.trim()
        while (candidate.length >= 2 &&
            ((candidate.startsWith("\"") && candidate.endsWith("\"")) ||
                (candidate.startsWith("'") && candidate.endsWith("'")))
        ) {
            candidate = candidate.substring(1, candidate.length - 1).trim()
        }
        if (candidate.isBlank() ||
            candidate.equals("null", ignoreCase = true) ||
            candidate.equals("undefined", ignoreCase = true) ||
            candidate.startsWith("Bot ", ignoreCase = true) ||
            candidate.startsWith("Bearer ", ignoreCase = true) ||
            candidate.length < MIN_TOKEN_LEN ||
            candidate.any { it.isWhitespace() || it.isISOControl() }
        ) {
            return null
        }
        return candidate
    }

    /**
     * Normalizes an `external_asset_path` returned by `/external-assets` (or an existing
     * `media.discordapp.net` / `mp:` reference) for Gateway `large_image`.
     */
    fun normalizeExternalAssetImage(externalAssetPath: String): String {
        val trimmed = externalAssetPath.trim()
        if (trimmed.isEmpty()) return ""
        return when {
            trimmed.startsWith("mp:") -> trimmed
            trimmed.startsWith("https://media.discordapp.net/") ->
                "mp:${trimmed.removePrefix("https://media.discordapp.net/")}"
            trimmed.startsWith("http://media.discordapp.net/") ->
                "mp:${trimmed.removePrefix("http://media.discordapp.net/")}"
            else -> "mp:$trimmed"
        }
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
