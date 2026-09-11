package com.openbitfun.mobile.core.transport

import io.ktor.http.Url

public data class AccountDeviceLink(public val relayUrl: String, public val deviceId: String)

public fun normalizeAccountRelayUrl(value: String): String? = runCatching {
    val url = Url(value.trim())
    if (url.protocol.name !in setOf("http", "https") || url.user != null || url.password != null ||
        url.parameters.names().isNotEmpty() || url.fragment.isNotEmpty()) return null
    val normalized = url.toString().trimEnd('/')
    if (normalized == DEFAULT_CLOUD_RELAY_URL) return normalized
    val host = url.host.removeSurrounding("[", "]").lowercase()
    val octets = host.split('.').mapNotNull(String::toIntOrNull)
    val local = host == "localhost" || host == "::1" ||
        Regex("^(f[cd][0-9a-f]{2}|fe[89ab][0-9a-f]):").containsMatchIn(host) ||
        (octets.size == 4 && octets.all { it in 0..255 } && (octets[0] == 10 || octets[0] == 127 ||
            (octets[0] == 192 && octets[1] == 168) || (octets[0] == 172 && octets[1] in 16..31) ||
            (octets[0] == 169 && octets[1] == 254)))
    normalized.takeIf { local && url.encodedPath in setOf("", "/") }
}.getOrNull()

/** The link chooses an endpoint and target; authenticated membership grants access. */
public fun accountDeviceLink(value: String): AccountDeviceLink? = runCatching {
    if (value.length > 8192) return null
    val url = Url(value.trim())
    if (url.parameters.names().isNotEmpty() || !url.fragment.startsWith("/pair?")) return null
    val endpoint = normalizeAccountRelayUrl(value.trim().substringBefore('#')) ?: return null
    val query = Url("https://localhost/?" + url.fragment.removePrefix("/pair?"))
    if (query.parameters.names() != setOf("did")) return null
    val id = query.parameters.getAll("did")?.singleOrNull()?.takeIf { id ->
        id.isNotEmpty() && id.length <= 128 && id !in setOf(".", "..") &&
            id.all { (it.isLetterOrDigit() && it.code < 128) || it in "-_." }
    } ?: return null
    AccountDeviceLink(endpoint, id)
}.getOrNull()
