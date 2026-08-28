package com.bitfun.mobile.core.feature.session

/**
 * Whether the desktop peer can accept edited tool approvals.
 */
public enum class ToolApprovalEditSupport {
    SUPPORTED,
    UNSUPPORTED,
}

/**
 * Today the desktop confirm_tool accepts only tool_id, so edited tool approvals
 * are UNSUPPORTED. This flips to SUPPORTED when the peer advertises confirm_tool
 * edit support. Android UI reads this fact to decide whether to offer an edit
 * affordance.
 */
public object ToolApprovalEditContract {
    public val support: ToolApprovalEditSupport = ToolApprovalEditSupport.UNSUPPORTED
}
