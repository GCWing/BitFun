package com.bitfun.mobile.core.protocol

import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable

@Serializable
public data class FileInfoResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("name") val name: String? = null,
    @SerialName("size") val size: Long? = null,
    @SerialName("mime_type") val mimeType: String? = null,
) : CommandStatus

/**
 * One chunk of a file transfer. The peer streams large files as a sequence of
 * these; [offset] and [totalSize] are what let the client reassemble and show
 * progress without a separate size call.
 */
@Serializable
public data class ReadFileChunkResponse(
    @SerialName("resp") override val resp: String? = null,
    @SerialName("message") override val message: String? = null,
    @SerialName("name") val name: String? = null,
    @SerialName("chunk_base64") val chunkBase64: String? = null,
    @SerialName("offset") val offset: Long? = null,
    @SerialName("chunk_size") val chunkSize: Long? = null,
    @SerialName("total_size") val totalSize: Long? = null,
    @SerialName("mime_type") val mimeType: String? = null,
) : CommandStatus
