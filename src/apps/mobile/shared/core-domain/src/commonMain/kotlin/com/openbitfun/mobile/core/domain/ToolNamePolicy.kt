package com.openbitfun.mobile.core.domain

import com.openbitfun.mobile.core.protocol.RemoteToolStatusResponse

/**
 * Which family a tool belongs to, judged from its name.
 *
 * Ported from the `is*Tool` predicates in `pages/components/ToolStatusList.ets`.
 * The relay forwards whatever the agent calls its tools, and the same operation
 * arrives as `Read`, `read_file` or `read-file` depending on which agent ran it,
 * so every name is folded to one spelling before it is matched. That is also why
 * the raw-name checks the source keeps alongside the normalized ones are absent:
 * lowercasing subsumes them.
 */
public object ToolNamePolicy {
    /** The tool's name folded to `snake_case`, or `tool` when it sent none. */
    public fun normalized(tool: RemoteToolStatusResponse): String =
        tool.name.orEmpty().ifEmpty { "Tool" }
            .replace(' ', '_')
            .replace('-', '_')
            .lowercase()

    public fun isTodo(tool: RemoteToolStatusResponse): Boolean = normalized(tool).contains("todo")

    public fun isTask(tool: RemoteToolStatusResponse): Boolean = normalized(tool) == "task"

    public fun isDirectoryList(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in DIRECTORY

    public fun isFileRead(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in READ

    public fun isFileCreate(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in CREATE

    public fun isFileMutation(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in MUTATION

    public fun isDelete(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in DELETE

    public fun isDiff(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in DIFF

    public fun isPatch(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in PATCH

    public fun isCommand(tool: RemoteToolStatusResponse): Boolean {
        val name = normalized(tool)
        return name in COMMAND || name.contains("exec_command")
    }

    public fun isGit(tool: RemoteToolStatusResponse): Boolean {
        val name = normalized(tool)
        return name == "git" || name.startsWith("git_")
    }

    /** Fetching one page, as opposed to searching the web for pages. */
    public fun isWebFetch(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in WEB_FETCH

    public fun isWebSearch(tool: RemoteToolStatusResponse): Boolean = normalized(tool) in WEB_SEARCH

    public fun isSearch(tool: RemoteToolStatusResponse): Boolean {
        val name = normalized(tool)
        return name in SEARCH || name.contains("search")
    }

    /**
     * Whether the tool puts a question to the user, either because it is one now
     * or because it is the question tool in a state that has already been
     * answered — the icon should stay a question mark once it is over.
     */
    public fun isQuestionLike(tool: RemoteToolStatusResponse): Boolean =
        ToolStatusPolicy.isQuestion(tool) || normalized(tool) in QUESTION

    private val DIRECTORY = setOf("ls", "list_directory")
    private val READ = setOf("read", "read_file") + DIRECTORY
    private val CREATE = setOf("write", "write_file", "create", "create_file")
    private val DELETE = setOf("delete", "delete_file", "remove_file")
    private val DIFF = setOf("get_file_diff", "getfilediff")
    private val PATCH = setOf("apply_patch", "applypatch", "patch")
    private val MUTATION = CREATE + DELETE + DIFF + PATCH + setOf(
        "edit",
        "edit_file",
        "file_edit",
        "multi_edit",
        "multiedit",
        "strreplace",
        "str_replace",
        "str_replace_editor",
        "replace",
        "replace_file",
        "update_file",
    )
    private val COMMAND = setOf(
        "bash",
        "shell",
        "git",
        "exec_command",
        "run_command",
        "terminal",
        "terminal_command",
        "powershell",
        "sh",
        "write_stdin",
        "writestdin",
        "exec_control",
        "execcontrol",
    )
    private val WEB_FETCH = setOf("webfetch", "web_fetch")
    private val WEB_SEARCH = setOf("websearch", "web_search")
    private val SEARCH = setOf("grep", "glob", "semanticsearch", "semantic_search") + WEB_SEARCH
    private val QUESTION = setOf("askuserquestion", "ask_user_question")
}
