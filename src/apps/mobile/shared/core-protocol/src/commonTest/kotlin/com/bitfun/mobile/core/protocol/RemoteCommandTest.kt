package com.bitfun.mobile.core.protocol

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNull
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.put
import kotlin.test.assertTrue

class RemoteCommandTest {
    @Test
    fun omitsAbsentFieldsInsteadOfSendingNull() {
        // Some desktop handlers branch on key presence, so `"title": null` and a
        // missing `title` are not interchangeable.
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "poll_session", sessionId = "s1", sinceVersion = 7),
        )
        assertEquals("""{"cmd":"poll_session","session_id":"s1","since_version":7}""", encoded)
    }

    @Test
    fun permissionModeUsesWireSpelling() {
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "set_permission_mode", mode = RemotePermissionMode.FullAccess),
        )
        assertEquals("""{"cmd":"set_permission_mode","mode":"full_access"}""", encoded)
    }

    @Test
    fun permissionModeToleratesFutureWireValues() {
        val future = RelayJson.decodeFromString(
            PermissionModeResponse.serializer(),
            """{"resp":"ok","mode":"future_mode"}""",
        )
        val ask = RelayJson.decodeFromString(
            PermissionModeResponse.serializer(),
            """{"resp":"ok","mode":"ask"}""",
        )
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "set_permission_mode", mode = RemotePermissionMode.Unknown),
        )

        assertEquals(RemotePermissionMode.Unknown, future.mode)
        assertEquals(RemotePermissionMode.Ask, ask.mode)
        assertEquals("""{"cmd":"set_permission_mode","mode":"unknown"}""", encoded)
    }

    @Test
    fun messageImagesUseDesktopImageContextsField() {
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(
                cmd = "send_message",
                sessionId = "s1",
                content = "look",
                imageContexts = listOf(
                    RemoteImageContext("image-1", null, "data:image/png;base64,abc", "image/png", null),
                ),
            ),
        )
        assertEquals(
            """{"cmd":"send_message","session_id":"s1","content":"look","image_contexts":[{"id":"image-1","data_url":"data:image/png;base64,abc","mime_type":"image/png"}]}""",
            encoded,
        )
    }

    @Test
    fun updatedInputRoundTripsAndOldPeerPayloadDefaultsToNull() {
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(
                cmd = "confirm_tool",
                toolId = "t",
                updatedInput = buildJsonObject { put("command", "ls") },
            ),
        )
        val decoded = RelayJson.decodeFromString(
            RemoteCommand.serializer(),
            """{"cmd":"confirm_tool","tool_id":"t"}""",
        )

        assertTrue("\"updated_input\":{" in encoded, encoded)
        assertEquals("t", decoded.toolId)
        assertNull(decoded.updatedInput)
    }

    @Test
    fun sessionMessageCursorAndMoreFieldsKeepWireContract() {
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "get_session_messages", sessionId = "s", limit = 100, beforeMessageId = "m9"),
        )
        val trueResponse = RelayJson.decodeFromString(
            SessionMessagesResponse.serializer(),
            """{"resp":"ok","has_more":true}""",
        )
        val falseResponse = RelayJson.decodeFromString(
            SessionMessagesResponse.serializer(),
            """{"resp":"ok","has_more":false}""",
        )
        val absentResponse = RelayJson.decodeFromString(
            SessionMessagesResponse.serializer(),
            """{"resp":"ok"}""",
        )

        assertTrue("\"before_message_id\":\"m9\"" in encoded, encoded)
        assertTrue(trueResponse.hasMore)
        assertFalse(falseResponse.hasMore)
        assertFalse(absentResponse.hasMore)
        assertTrue(absentResponse.messages.isEmpty())
    }

    @Test
    fun sendMessageImagesKeepMimeAndLegacyImageFields() {
        val encoded = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(
                cmd = "send_message",
                images = listOf(ImageAttachment("n", "data:image/png;base64,abc")),
                imageContexts = listOf(RemoteImageContext("i", null, "data:image/png;base64,abc", "image/png", null)),
            ),
        )

        assertTrue("\"mime_type\":\"image/png\"" in encoded, encoded)
        assertTrue("\"images\":[{\"name\":\"n\",\"data_url\":\"data:image/png;base64,abc\"}]" in encoded, encoded)
    }

    @Test
    fun provisionPeerDeviceIsAnExplicitlyUnsupportedCommand() {
        val provision = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "provision_peer_device"),
        )
        val request = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "x", requestId = "r1"),
        )

        assertEquals("""{"cmd":"provision_peer_device"}""", provision)
        assertFalse("device_id" in provision)
        assertFalse("device_name" in provision)
        assertFalse("request_id" in provision)
        assertTrue("\"_request_id\":\"r1\"" in request, request)
        assertFalse("\"request_id\"" in request, request)
        assertEquals(ProvisionPeerDeviceSupport.UNSUPPORTED, ProvisionPeerDeviceContract.support)
    }

    @Test
    fun unchangedPollDecodesWithoutOptionalPayloads() {
        val decoded = RelayJson.decodeFromString(
            PollSessionResponse.serializer(),
            """{"resp":"ok","version":42,"changed":false}""",
        )
        assertEquals(42, decoded.version)
        assertFalse(decoded.changed)
        assertTrue(decoded.newMessages.isEmpty())
        assertNull(decoded.activeTurn)
        assertNull(decoded.modelCatalog)
    }

    @Test
    fun activeTurnAndMessagesDecodeTogether() {
        val decoded = RelayJson.decodeFromString(
            PollSessionResponse.serializer(),
            """
            {"resp":"ok","version":43,"changed":true,
             "new_messages":[{"message_id":"m1","role":"user","content":"hi"}],
             "active_turn":{"turn_id":"t1","status":"running","text":"thinking",
                            "tools":[{"id":"tool1","name":"bash","status":"running"}]},
             "total_msg_count":9}
            """.trimIndent(),
        )
        assertEquals("m1", decoded.newMessages.single().resolvedId)
        assertEquals("t1", decoded.activeTurn?.turnId)
        assertEquals("bash", decoded.activeTurn?.tools?.single()?.name)
        assertEquals(9, decoded.totalMessageCount)
    }

    /**
     * The desktop builds this number from the catalog's last-modified time in
     * milliseconds and masks it to 53 bits, so every real one is far past 2^31.
     * It rode inside `poll_session`, which meant an [Int] here failed the whole
     * reply — the session opened and then never updated again.
     */
    @Test
    fun catalogVersionSurvivesTheDesktopsFullRange() {
        val version = 9007199254740991L
        val decoded = RelayJson.decodeFromString(
            PollSessionResponse.serializer(),
            """{"resp":"ok","version":7,"changed":true,"model_catalog":{"version":$version,"models":[]}}""",
        )

        assertEquals(version, decoded.modelCatalog?.version)

        // And it goes back out unchanged, or the desktop resends the catalog on
        // every poll because the echo never matches what it holds.
        val echoed = RelayJson.encodeToString(
            RemoteCommand.serializer(),
            RemoteCommand(cmd = "poll_session", sessionId = "s1", knownModelCatalogVersion = version),
        )
        assertTrue("\"known_model_catalog_version\":$version" in echoed, echoed)
    }

    @Test
    fun credentialsStayOutOfToString() {
        val challenge = ChallengeCommand(
            challengeEcho = "echo",
            deviceId = "dev",
            deviceName = "Pixel",
            mobileInstallId = "install",
            userId = "user",
            password = "hunter2",
        )
        assertFalse("hunter2" in challenge.toString())

        val identity = DelegatedIdentityResponse(
            resp = "ok",
            token = "tok_secret",
            userId = "user",
            masterKey = "key_secret",
        )
        val rendered = identity.toString()
        assertFalse("tok_secret" in rendered)
        assertFalse("key_secret" in rendered)
    }

    @Test
    fun workspaceAliasesResolveInMapperOrder() {
        val decoded = RelayJson.decodeFromString(
            WorkspaceInfoResponse.serializer(),
            """{"resp":"ok","path":"/a","workspace_path":"/b","project_name":"A","workspace_name":"B"}""",
        )
        assertEquals("/a", decoded.resolvedPath)
        assertEquals("A", decoded.resolvedName)
    }

    @Test
    fun recentWorkspaceFallsBackThroughTimestampAliases() {
        val decoded = RelayJson.decodeFromString(
            RecentWorkspaceListResponse.serializer(),
            """{"resp":"ok","workspaces":[{"path":"/a","updatedAt":1754476800}]}""",
        )
        assertEquals("1754476800", decoded.workspaces.single().lastOpened)
    }
}
